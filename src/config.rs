use serde::Deserialize;
use std::fs;
use std::io::Read;
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

/// Loads config from:
/// 1. First CLI arg as a file path  (e.g. `pg2ch config.yaml`)
/// 2. `-` or no arg + stdin is not a TTY → read YAML from stdin
/// 3. Fallback: `config.yaml` in current directory
pub fn load_from_arg() -> Result<Config, AppError> {
    let arg = std::env::args().nth(1);

    let content = match arg.as_deref() {
        Some("-") => read_stdin()?,
        Some(path) => fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("failed to read {}: {}", path, e)))?,
        None => {
            // Try stdin if it's not a TTY (i.e. piped), else fall back to config.yaml
            if !is_tty() {
                read_stdin()?
            } else {
                fs::read_to_string("config.yaml")
                    .map_err(|e| AppError::Config(format!("failed to read config.yaml: {}", e)))?
            }
        }
    };

    parse(&content)
}

fn read_stdin() -> Result<String, AppError> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| AppError::Config(format!("failed to read stdin: {}", e)))?;
    Ok(buf)
}

fn is_tty() -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { libc::isatty(std::io::stdin().as_raw_fd()) == 1 }
}

fn parse(content: &str) -> Result<Config, AppError> {
    serde_yaml::from_str(content)
        .map_err(|e| AppError::Config(format!("failed to parse config: {}", e)))
}
