use crate::{
    ch_client::ChClient,
    config::TableConfig,
    error::AppError,
    pg_client::PgClient,
    type_map::pg_to_ch_type,
};
use tracing::info;

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
            info!("table {} already exists in ClickHouse, skipping", dest);
            return Ok(());
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
}
