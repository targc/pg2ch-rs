# Code Project Structure — pg2ch

## Directory Layout

```
pg2ch-rs/
├── Cargo.toml
├── Cargo.lock
├── Dockerfile
├── config.example.yaml
├── REQUIREMENTS.md
├── SYSTEM_ARCHITECTURE.md
├── SYSTEM_FLOW.md
├── CODE_PROJECT_STRUCTURE.md
└── src/
    ├── main.rs              # entry point: startup sequence
    ├── config.rs            # config struct + YAML loading
    ├── error.rs             # AppError enum
    ├── type_map.rs          # PG → CH type mapping
    ├── schema_manager.rs    # schema introspection + CH DDL
    ├── cursor_store.rs      # in-memory cursor state
    ├── sync_engine.rs       # ticker loop
    ├── table_worker.rs      # per-table ETL pipeline
    ├── pg_client.rs         # PostgreSQL query helpers
    └── ch_client.rs         # ClickHouse insert helpers
```

---

## Key Types per File

### `config.rs`
```rust
#[derive(Deserialize)]
pub struct Config {
    pub interval_ms: u64,
    pub batch_size: usize,
    pub source: DbConfig,
    pub destination: DbConfig,
    pub tables: Vec<TableConfig>,
}

#[derive(Deserialize)]
pub struct DbConfig {
    pub connection_url: String,
}

#[derive(Deserialize)]
pub struct TableConfig {
    pub source: String,
    pub dest: Option<String>,   // defaults to source name
    pub cursors: Vec<String>,   // ordered cursor columns
}

impl TableConfig {
    pub fn dest_name(&self) -> &str {
        self.dest.as_deref().unwrap_or(&self.source)
    }
}
```

### `error.rs`
```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("config error: {0}")]
    Config(String),
    #[error("postgres error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("clickhouse error: {0}")]
    Clickhouse(String),
    #[error("cursor missing for table: {0}")]
    CursorMissing(String),
    #[error("schema error: {0}")]
    Schema(String),
}
```

### `cursor_store.rs`
```rust
pub type CursorValues = Vec<serde_json::Value>;  // ordered, matches cursors[]

pub struct CursorStore {
    inner: HashMap<String, CursorValues>,  // key = table source name
}

impl CursorStore {
    pub fn get(&self, table: &str) -> Result<&CursorValues, AppError>;
    pub fn set(&mut self, table: &str, values: CursorValues);
}
```

### `schema_manager.rs`
```rust
pub struct SchemaManager<'a> {
    pg: &'a PgClient,
    ch: &'a ChClient,
}

impl SchemaManager<'_> {
    pub async fn sync_all(&self, tables: &[TableConfig]) -> Result<(), AppError>;
    // introspects PG, maps types, creates CH table if missing
}
```

### `sync_engine.rs`
```rust
pub struct SyncEngine {
    config: Config,
    pg: Arc<PgClient>,
    ch: Arc<ChClient>,
    cursors: Arc<Mutex<CursorStore>>,
}

impl SyncEngine {
    pub async fn run(&self) -> Result<(), AppError>;
    // loop: sync_all_tables → sleep(interval_ms)

    async fn sync_all_tables(&self) -> Result<(), AppError>;
    // spawns one task per table, joins all
}
```

### `table_worker.rs`
```rust
pub struct TableWorker<'a> {
    table: &'a TableConfig,
    pg: &'a PgClient,
    ch: &'a ChClient,
    cursors: Arc<Mutex<CursorStore>>,
    batch_size: usize,
}

impl TableWorker<'_> {
    pub async fn run(&self) -> Result<(), AppError>;
    // paginated loop: fetch → insert → advance cursor
}
```

---

## Dependencies (`Cargo.toml`)

```toml
[dependencies]
tokio              = { version = "1", features = ["full"] }
serde              = { version = "1", features = ["derive"] }
serde_yaml         = "0.9"
serde_json         = "1"
thiserror          = "1"
tracing            = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# PostgreSQL
tokio-postgres = { version = "0.7", features = ["with-serde_json-1", "with-chrono-0_4", "with-uuid-1"] }

# ClickHouse
clickhouse = { version = "0.12", features = ["inserter"] }

# Misc
chrono = { version = "0.4", features = ["serde"] }
uuid   = { version = "1", features = ["serde"] }
```

---

## `main.rs` Skeleton

```rust
#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    let config = config::load("config.yaml")?;

    let pg = PgClient::connect(&config.source.connection_url).await?;
    let ch = ChClient::connect(&config.destination.connection_url).await?;

    // Step 1: schema sync
    SchemaManager::new(&pg, &ch)
        .sync_all(&config.tables)
        .await?;

    // Step 2: cursor init from ClickHouse
    let mut cursors = CursorStore::default();
    for table in &config.tables {
        let values = ch.fetch_last_cursor(table).await?;
        cursors.set(&table.source, values);
    }

    // Step 3: run ticker loop
    SyncEngine::new(config, pg, ch, cursors).run().await
}
```
