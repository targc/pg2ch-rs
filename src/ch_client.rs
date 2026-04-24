use crate::error::AppError;
use reqwest::Client;
use serde_json::{Map, Value};
use url::Url;

pub struct ChClient {
    http: Client,
    endpoint: String,
    params: Vec<(String, String)>,
    database: String,
}

impl ChClient {
    pub async fn connect(url_str: &str) -> Result<Self, AppError> {
        let url = Url::parse(url_str)
            .map_err(|e| AppError::Clickhouse(format!("invalid url: {}", e)))?;

        let host = url.host_str().unwrap_or("localhost");
        let port = url.port().unwrap_or(8123);
        let database = url.path().trim_start_matches('/').to_string();
        let database = if database.is_empty() { "default".to_string() } else { database };
        let user = {
            let u = url.username();
            if u.is_empty() { "default".to_string() } else { u.to_string() }
        };
        let password = url.password().unwrap_or("").to_string();

        let endpoint = format!("http://{}:{}/", host, port);
        let params = vec![
            ("database".to_string(), database.clone()),
            ("user".to_string(), user),
            ("password".to_string(), password),
        ];

        let http = Client::new();
        let client = Self { http, endpoint, params, database };

        // Verify connectivity
        client.execute("SELECT 1").await
            .map_err(|e| AppError::Clickhouse(format!("connection test failed: {}", e)))?;

        Ok(client)
    }

    pub async fn execute(&self, sql: &str) -> Result<(), AppError> {
        let resp = self.http
            .post(&self.endpoint)
            .query(&self.params)
            .body(sql.to_string())
            .send()
            .await
            .map_err(|e| AppError::Clickhouse(format!("request failed: {}", e)))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Clickhouse(format!("query failed: {}", body.trim())));
        }

        Ok(())
    }

    pub async fn query_rows(&self, sql: &str) -> Result<Vec<Map<String, Value>>, AppError> {
        let full_sql = format!("{} FORMAT JSONEachRow", sql);

        let resp = self.http
            .post(&self.endpoint)
            .query(&self.params)
            .body(full_sql)
            .send()
            .await
            .map_err(|e| AppError::Clickhouse(format!("request failed: {}", e)))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Clickhouse(format!("query failed: {}", body.trim())));
        }

        let text = resp.text().await
            .map_err(|e| AppError::Clickhouse(format!("failed to read response: {}", e)))?;

        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str::<Map<String, Value>>(l)
                    .map_err(|e| AppError::Clickhouse(format!("failed to parse row: {}", e)))
            })
            .collect()
    }

    pub async fn insert_rows(&self, table: &str, rows: &[Map<String, Value>]) -> Result<(), AppError> {
        if rows.is_empty() {
            return Ok(());
        }

        let body: String = rows.iter()
            .map(|r| serde_json::to_string(r).expect("Map serialization cannot fail"))
            .collect::<Vec<_>>()
            .join("\n");

        let mut params = self.params.clone();
        params.push(("query".to_string(), format!("INSERT INTO {} FORMAT JSONEachRow", table)));

        let resp = self.http
            .post(&self.endpoint)
            .query(&params)
            .body(body)
            .send()
            .await
            .map_err(|e| AppError::Clickhouse(format!("insert request failed: {}", e)))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Clickhouse(format!("insert failed: {}", body.trim())));
        }

        Ok(())
    }

    pub async fn table_exists(&self, table: &str) -> Result<bool, AppError> {
        let sql = format!(
            "SELECT 1 FROM system.tables WHERE database = '{}' AND name = '{}'",
            escape_ch_string(&self.database),
            escape_ch_string(table),
        );
        let rows = self.query_rows(&sql).await?;
        Ok(!rows.is_empty())
    }

    pub async fn fetch_last_cursor(
        &self,
        table: &str,
        cursor_cols: &[String],
    ) -> Result<Vec<Value>, AppError> {
        if cursor_cols.is_empty() {
            return Ok(vec![]);
        }

        let cols = cursor_cols.join(", ");
        let order = cursor_cols.iter()
            .map(|c| format!("{} DESC", c))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!("SELECT {} FROM {} ORDER BY {} LIMIT 1", cols, table, order);
        let rows = self.query_rows(&sql).await?;

        if rows.is_empty() {
            return Ok(vec![]); // empty table → full load
        }

        let row = &rows[0];
        Ok(cursor_cols.iter()
            .map(|c| row.get(c).cloned().unwrap_or(Value::Null))
            .collect())
    }
}

fn escape_ch_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}
