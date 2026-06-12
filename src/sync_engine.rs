use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::error;

use crate::{
    ch_client::ChClient,
    config::Config,
    cursor_store::CursorStore,
    pg_client::PgClient,
    table_worker::TableWorker,
};

pub struct SyncEngine {
    config: Config,
    pg: Arc<PgClient>,
    ch: Arc<ChClient>,
    cursors: Arc<Mutex<CursorStore>>,
}

impl SyncEngine {
    pub fn new(config: Config, pg: PgClient, ch: ChClient, cursors: CursorStore) -> Self {
        Self {
            config,
            pg: Arc::new(pg),
            ch: Arc::new(ch),
            cursors: Arc::new(Mutex::new(cursors)),
        }
    }

    pub async fn run(&self) -> Result<(), crate::error::AppError> {
        loop {
            self.sync_all_tables().await;
            sleep(Duration::from_millis(self.config.interval_ms)).await;
        }
    }

    async fn sync_all_tables(&self) {
        let mut handles = vec![];

        for table in &self.config.tables {
            let table_name = table.source.clone();
            let worker = TableWorker {
                table: table.clone(),
                pg: Arc::clone(&self.pg),
                ch: Arc::clone(&self.ch),
                cursors: Arc::clone(&self.cursors),
                batch_size: self.config.batch_size,
            };
            handles.push(tokio::spawn(async move {
                (table_name, worker.run().await)
            }));
        }

        for handle in handles {
            match handle.await {
                Ok((_, Ok(()))) => {}
                Ok((name, Err(e))) => {
                    // CursorMissing or other fatal error
                    error!("fatal error in worker for {}: {}", name, e);
                    std::process::exit(1);
                }
                Err(e) => {
                    error!("worker task panicked: {}", e);
                }
            }
        }
    }
}
