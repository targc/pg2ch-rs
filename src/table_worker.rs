use std::sync::Arc;

use serde_json::Value;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::{
    ch_client::ChClient, config::TableConfig, cursor_store::CursorStore, error::AppError,
    pg_client::PgClient,
};

pub struct TableWorker {
    pub table: TableConfig,
    pub pg: Arc<PgClient>,
    pub ch: Arc<ChClient>,
    pub cursors: Arc<Mutex<CursorStore>>,
    pub batch_size: usize,
}

impl TableWorker {
    pub async fn run(&self) -> Result<(), AppError> {
        let table = &self.table;

        let mut cursor_values = {
            let store = self.cursors.lock().await;
            store.get(&table.source)?.clone()
        };
        info!("syncing, table: {}, cursor: {:?}", table.dest_name(), cursor_values);

        loop {
            let rows = match self
                .pg
                .fetch_batch(
                    &table.source,
                    &table.cursors,
                    &cursor_values,
                    self.batch_size,
                )
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    error!("failed to fetch batch from {}: {}", table.source, e);
                    return Ok(());
                }
            };

            if rows.is_empty() {
                break;
            }

            let row_count = rows.len();

            if let Err(e) = self.ch.insert_rows(table.dest_name(), &rows).await {
                error!("failed to insert into {}: {}", table.dest_name(), e);
                return Ok(()); // cursor not advanced; retried next tick
            }

            // Advance cursor to last row's values
            let last_row = rows.last().unwrap();
            let new_cursor: Vec<Value> = table
                .cursors
                .iter()
                .map(|c| last_row.get(c).cloned().unwrap_or(Value::Null))
                .collect();

            if new_cursor.iter().any(|v| v.is_null()) {
                error!(
                    "cursor column has NULL value in {}, stopping sync to avoid silent stall. cursor: {:?}",
                    table.source, new_cursor
                );
                return Ok(());
            }
            cursor_values = new_cursor;

            {
                let mut store = self.cursors.lock().await;
                store.set(&table.source, cursor_values.clone());
            }

            info!("synced {} rows from {}", row_count, table.source);

            if row_count < self.batch_size {
                break; // last page
            }
        }

        Ok(())
    }
}
