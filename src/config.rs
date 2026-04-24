use serde::Deserialize;
use std::fs;
use crate::error::AppError;

#[derive(Deserialize)]
pub struct Config {
    pub interval_ms: u64,
    pub query_batch_size: usize,
    pub upsert_batch_size: usize,
    pub source: DbConfig,
    pub destination: DbConfig,
    pub tables: Vec<TableConfig>,
}

#[derive(Deserialize)]
pub struct DbConfig {
    pub connection_url: String,
}

#[derive(Deserialize, Clone)]
pub struct TableConfig {
    pub source: String,
    pub dest: Option<String>,
    pub cursors: Vec<String>,
}

impl TableConfig {
    pub fn dest_name(&self) -> &str {
        self.dest.as_deref().unwrap_or(&self.source)
    }
}

pub fn load(path: &str) -> Result<Config, AppError> {
    let content = fs::read_to_string(path)
        .map_err(|e| AppError::Config(format!("failed to read {}: {}", path, e)))?;
    serde_yaml::from_str(&content)
        .map_err(|e| AppError::Config(format!("failed to parse config: {}", e)))
}
