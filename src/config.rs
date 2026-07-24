use serde::Deserialize;
use std::fs;
use std::io::Read;
use crate::error::AppError;

#[derive(Deserialize)]
pub struct Config {
    pub interval_ms: u64,
    pub batch_size: usize,
    /// Warn when a table has synced 0 rows for this many consecutive ticks, then
    /// again every that many ticks. Catches a frozen cursor, which is otherwise
    /// indistinguishable from an idle table. `0` disables the warning.
    ///
    /// Note this fires for genuinely static tables too (lookup tables that simply
    /// never change) — tune it per deployment or set `0` if that noise isn't wanted.
    #[serde(default = "default_stall_warn_ticks")]
    pub stall_warn_ticks: u64,
    pub source: DbConfig,
    pub destination: DbConfig,
    pub tables: Vec<TableConfig>,
}

/// 1200 ticks — one hour at the default `interval_ms: 3000`.
fn default_stall_warn_ticks() -> u64 {
    1200
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
    /// ClickHouse ORDER BY key (deduplication key for ReplacingMergeTree).
    /// Defaults to `cursors` if not set.
    pub primary_key: Option<Vec<String>>,
}

impl TableConfig {
    pub fn dest_name(&self) -> &str {
        self.dest.as_deref().unwrap_or(&self.source)
    }

    pub fn ch_order_by(&self) -> &[String] {
        self.primary_key.as_deref().unwrap_or(&self.cursors)
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
