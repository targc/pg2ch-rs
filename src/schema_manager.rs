use crate::{
    ch_client::ChClient,
    config::TableConfig,
    error::AppError,
    pg_client::PgClient,
    type_map::pg_to_ch_type,
};
use std::collections::HashSet;
use tracing::{info, warn};

pub struct SchemaManager<'a> {
    pg: &'a PgClient,
    ch: &'a ChClient,
}

impl<'a> SchemaManager<'a> {
    pub fn new(pg: &'a PgClient, ch: &'a ChClient) -> Self {
        Self { pg, ch }
    }

    pub async fn sync_all(&self, tables: &[TableConfig]) -> Result<(), AppError> {
        for table in tables {
            self.sync_table(table).await?;
        }
        Ok(())
    }

    async fn sync_table(&self, table: &TableConfig) -> Result<(), AppError> {
        let dest = table.dest_name();

        if self.ch.table_exists(dest).await? {
            return self.sync_columns(table).await;
        }

        info!("creating ClickHouse table {}", dest);

        let columns = self.pg.get_columns(&table.source).await?;
        if columns.is_empty() {
            return Err(AppError::Schema(format!(
                "table {} not found or has no columns in PostgreSQL",
                table.source
            )));
        }

        let order_by_cols: std::collections::HashSet<&str> = table.ch_order_by().iter()
            .map(|s| s.as_str())
            .collect();

        let col_defs: Vec<String> = columns.iter()
            .map(|c| {
                // ORDER BY columns must be non-nullable in ClickHouse
                let nullable = c.is_nullable && !order_by_cols.contains(c.name.as_str());
                format!(
                    "    `{}` {}",
                    c.name,
                    pg_to_ch_type(&c.pg_type, nullable, c.numeric_precision, c.numeric_scale)
                )
            })
            .collect();

        let order_by = table.ch_order_by().iter()
            .map(|c| format!("`{}`", c))
            .collect::<Vec<_>>()
            .join(", ");

        let ddl = format!(
            "CREATE TABLE IF NOT EXISTS `{}`\n(\n{}\n)\nENGINE = ReplacingMergeTree()\nORDER BY ({})",
            dest,
            col_defs.join(",\n"),
            order_by,
        );

        self.ch.execute(&ddl).await
            .map_err(|e| AppError::Schema(format!("failed to create table {}: {}", dest, e)))?;

        info!("created ClickHouse table {}", dest);
        Ok(())
    }

    async fn sync_columns(&self, table: &TableConfig) -> Result<(), AppError> {
        let dest = table.dest_name();
        let pg_cols = self.pg.get_columns(&table.source).await?;
        let ch_col_names: HashSet<String> = self.ch.get_columns(dest).await?.into_iter().collect();

        let order_by_cols: HashSet<&str> = table.ch_order_by().iter()
            .map(|s| s.as_str())
            .collect();

        for col in &pg_cols {
            if ch_col_names.contains(&col.name) {
                continue;
            }

            if order_by_cols.contains(col.name.as_str()) {
                warn!("new column {} is an ORDER BY key in {}, cannot add after creation", col.name, dest);
                continue;
            }

            let ch_type = pg_to_ch_type(&col.pg_type, col.is_nullable, col.numeric_precision, col.numeric_scale);
            let ddl = format!("ALTER TABLE `{}` ADD COLUMN `{}` {}", dest, col.name, ch_type);
            self.ch.execute(&ddl).await
                .map_err(|e| AppError::Schema(format!("failed to add column {} to {}: {}", col.name, dest, e)))?;
            info!("added column {} ({}) to {}", col.name, ch_type, dest);
        }

        Ok(())
    }
}
