use clickhouse_rs::Pool;
use serde_json::{Map, Value as JsonValue};

use crate::error::AppError;

pub struct ChClient {
    pool: Pool,
    database: String,
}

impl ChClient {
    pub async fn connect(url_str: &str) -> Result<Self, AppError> {
        let tcp_url = to_tcp_url(url_str);

        let database = url::Url::parse(&tcp_url)
            .ok()
            .map(|u| u.path().trim_start_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "default".to_string());

        let pool = Pool::new(tcp_url.as_str());

        // Test connection
        pool.get_handle()
            .await
            .map_err(|e| AppError::Clickhouse(format!("connection failed: {}", e)))?
            .execute("SELECT 1")
            .await
            .map_err(|e| AppError::Clickhouse(format!("connection test failed: {}", e)))?;

        Ok(Self { pool, database })
    }

    pub async fn execute(&self, sql: &str) -> Result<(), AppError> {
        self.pool
            .get_handle()
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))?
            .execute(sql)
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))
    }

    pub async fn insert_rows(&self, table: &str, rows: &[Map<String, JsonValue>]) -> Result<(), AppError> {
        if rows.is_empty() {
            return Ok(());
        }

        let cols: Vec<&String> = rows[0].keys().collect();
        let cols_str = cols.iter().map(|c| format!("`{}`", c)).collect::<Vec<_>>().join(", ");

        let vals_str = rows.iter().map(|row| {
            let v = cols.iter()
                .map(|c| format_sql_value(row.get(*c).unwrap_or(&JsonValue::Null)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", v)
        }).collect::<Vec<_>>().join(", ");

        let sql = format!("INSERT INTO {} ({}) VALUES {}", table, cols_str, vals_str);
        self.execute(&sql).await
    }

    pub async fn table_exists(&self, table: &str) -> Result<bool, AppError> {
        let sql = format!(
            "SELECT 1 FROM system.tables WHERE database = '{}' AND name = '{}'",
            escape(&self.database),
            escape(table),
        );
        let block = self.pool
            .get_handle()
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))?
            .query(&sql)
            .fetch_all()
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))?;
        Ok(block.row_count() > 0)
    }

    pub async fn fetch_last_cursor(
        &self,
        table: &str,
        cursor_cols: &[String],
    ) -> Result<Vec<JsonValue>, AppError> {
        if cursor_cols.is_empty() {
            return Ok(vec![]);
        }

        // Cast all cursor columns to String so we can read them uniformly.
        // These string values are used in PG WHERE clauses where PG casts them back.
        let cols_cast = cursor_cols.iter()
            .map(|c| format!("toString(`{}`) AS `{}`", c, c))
            .collect::<Vec<_>>()
            .join(", ");
        let order = cursor_cols.iter()
            .map(|c| format!("`{}` DESC", c))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!("SELECT {} FROM `{}` ORDER BY {} LIMIT 1", cols_cast, table, order);

        let block = self.pool
            .get_handle()
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))?
            .query(&sql)
            .fetch_all()
            .await
            .map_err(|e| AppError::Clickhouse(e.to_string()))?;

        if block.row_count() == 0 {
            return Ok(vec![]); // empty table → full load
        }

        let row = block.rows().next().unwrap();
        let values = cursor_cols.iter()
            .map(|c| {
                row.get::<Option<String>, _>(c.as_str())
                    .ok()
                    .flatten()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null)
            })
            .collect();

        Ok(values)
    }
}

/// Converts `clickhouse://` URL to `tcp://`, auto-adds `secure=true` for port 9440,
/// and sets a 30s connection timeout (default is 500ms — too short for cloud TLS).
fn to_tcp_url(url: &str) -> String {
    let tcp = url.replacen("clickhouse://", "tcp://", 1);
    let sep = if tcp.contains('?') { "&" } else { "?" };

    if tcp.contains(":9440") && !tcp.contains("secure=") {
        format!("{}{}secure=true&connection_timeout=30000ms", tcp, sep)
    } else {
        format!("{}{}connection_timeout=30000ms", tcp, sep)
    }
}

/// Formats a JSON value as a ClickHouse SQL literal.
fn format_sql_value(v: &JsonValue) -> String {
    match v {
        JsonValue::Null => "NULL".to_string(),
        JsonValue::Bool(b) => if *b { "1".to_string() } else { "0".to_string() },
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        _ => "NULL".to_string(),
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}
