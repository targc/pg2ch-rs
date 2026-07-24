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
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Send our logs to stderr. Redirect stdout → /dev/null so any binary output
    // from the native TCP library doesn't leak into the log stream.
    #[cfg(unix)]
    unsafe {
        let devnull = libc::open(b"/dev/null\0".as_ptr() as *const libc::c_char, libc::O_WRONLY);
        if devnull >= 0 {
            libc::dup2(devnull, libc::STDOUT_FILENO);
            libc::close(devnull);
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    // Identify this process run. Restarts are otherwise only visible as a gap in
    // log timestamps; `grep "sync engine starting"` gives an exact restart history.
    // BUILD_SHA is read at runtime so it can be set on the container without a rebuild;
    // CI already tags images with the short commit SHA.
    info!(
        "sync engine starting, version: {}, build: {}, pid: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::var("BUILD_SHA").unwrap_or_else(|_| "unknown".to_string()),
        std::process::id(),
    );

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
        // A NULL here is never legitimate: every later query becomes
        // `WHERE (cursor) > (NULL, ...)`, which matches nothing, so the table stops
        // syncing and cannot recover on its own. Say so loudly at the moment it happens.
        if values.iter().any(|v| v.is_null()) {
            warn!(
                "initial cursor for {} contains NULL: {:?} — this table will not sync",
                table.source, values
            );
        }
        info!("cursor ready for {}: {:?}", table.source, values);
        cursors.set(&table.source, values);
    }

    info!("starting sync engine");
    SyncEngine::new(config, pg, ch, cursors).run().await
}
