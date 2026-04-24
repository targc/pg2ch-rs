mod ch_client;
mod config;
mod cursor_store;
mod error;
mod pg_client;
mod schema_manager;
mod sync_engine;
mod table_worker;
mod type_map;

use ch_client::ChClient;
use cursor_store::CursorStore;
use error::AppError;
use pg_client::PgClient;
use schema_manager::SchemaManager;
use sync_engine::SyncEngine;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .init();

    let config = config::load_from_arg()?;

    info!("connecting to PostgreSQL");
    let pg = PgClient::connect(&config.source.connection_url).await?;

    info!("connecting to ClickHouse");
    let ch = ChClient::connect(&config.destination.connection_url).await?;

    info!("syncing schemas");
    SchemaManager::new(&pg, &ch).sync_all(&config.tables).await?;

    info!("initializing cursors");
    let mut cursors = CursorStore::default();
    for table in &config.tables {
        let values = ch.fetch_last_cursor(table.dest_name(), &table.cursors).await?;
        cursors.set(&table.source, values);
        info!("cursor ready for {}", table.source);
    }

    info!("starting sync engine");
    SyncEngine::new(config, pg, ch, cursors).run().await
}
