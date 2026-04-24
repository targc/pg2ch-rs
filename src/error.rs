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
