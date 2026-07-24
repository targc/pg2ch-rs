use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::{error, warn};

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
        // Consecutive ticks each table has synced nothing. Kept here rather than in
        // TableWorker because a worker is rebuilt every tick and holds no state.
        let mut idle_ticks: HashMap<String, u64> = HashMap::new();

        loop {
            self.sync_all_tables(&mut idle_ticks).await;
            sleep(Duration::from_millis(self.config.interval_ms)).await;
        }
    }

    async fn sync_all_tables(&self, idle_ticks: &mut HashMap<String, u64>) {
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
                Ok((name, Ok(rows))) => {
                    let idle = idle_ticks.entry(name.clone()).or_insert(0);
                    if rows > 0 {
                        *idle = 0;
                        continue;
                    }

                    *idle += 1;
                    let every = self.config.stall_warn_ticks;
                    if every > 0 && *idle % every == 0 {
                        let cursor = self
                            .cursors
                            .lock()
                            .await
                            .get(&name)
                            .map(|v| v.clone())
                            .unwrap_or_default();
                        warn!(
                            "{} has synced 0 rows for {} consecutive ticks, cursor: {:?}",
                            name, idle, cursor
                        );
                    }
                }
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
