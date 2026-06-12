use crate::error::AppError;
use rust_decimal::Decimal;
use rustls::ClientConfig;
use serde_json::{Map, Value};
use tokio_postgres::{types::Type, Client, NoTls, Row};
use tokio_postgres_rustls::MakeRustlsConnect;
use tracing::warn;

pub struct ColumnInfo {
    pub name: String,
    pub pg_type: String,
    pub is_nullable: bool,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
}

pub struct PgClient {
    client: Client,
}

impl PgClient {
    pub async fn connect(url: &str) -> Result<Self, AppError> {
        let ssl = url.contains("sslmode=") && !url.contains("sslmode=disable");
        let client = if ssl {
            let tls = MakeRustlsConnect::new(build_tls_config());
            let (client, conn) = tokio_postgres::connect(url, tls).await?;
            tokio::spawn(async move { if let Err(e) = conn.await { tracing::error!("{}", e); } });
            client
        } else {
            let (client, conn) = tokio_postgres::connect(url, NoTls).await?;
            tokio::spawn(async move { if let Err(e) = conn.await { tracing::error!("{}", e); } });
            client
        };
        Ok(Self { client })
    }

    pub async fn get_columns(&self, table: &str) -> Result<Vec<ColumnInfo>, AppError> {
        let (schema, table_name) = if let Some((s, t)) = table.split_once('.') {
            (s, t)
        } else {
            ("public", table)
        };

        let rows = self.client.query(
            "SELECT column_name, udt_name, is_nullable, numeric_precision, numeric_scale \
             FROM information_schema.columns \
             WHERE table_schema = $1 AND table_name = $2 \
             ORDER BY ordinal_position",
            &[&schema, &table_name],
        ).await?;

        Ok(rows.iter().map(|row| ColumnInfo {
            name: row.get::<_, String>(0),
            pg_type: row.get::<_, String>(1),
            is_nullable: row.get::<_, &str>(2) == "YES",
            numeric_precision: row.get::<_, Option<i32>>(3),
            numeric_scale: row.get::<_, Option<i32>>(4),
        }).collect())
    }

    pub async fn fetch_batch(
        &self,
        table: &str,
        cursor_cols: &[String],
        cursor_values: &[Value],
        batch_size: usize,
    ) -> Result<Vec<Map<String, Value>>, AppError> {
        let order = cursor_cols.iter()
            .map(|c| format!("{} ASC", c))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = if cursor_values.is_empty() {
            format!("SELECT * FROM {} ORDER BY {} LIMIT {}", table, order, batch_size)
        } else {
            let cols = cursor_cols.join(", ");
            let vals = cursor_values.iter()
                .map(format_pg_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "SELECT * FROM {} WHERE ({}) > ({}) ORDER BY {} LIMIT {}",
                table, cols, vals, order, batch_size
            )
        };

        let rows = self.client.query(&sql, &[]).await?;
        Ok(rows.iter().map(row_to_map).collect())
    }
}

fn build_tls_config() -> ClientConfig {
    let result = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    for cert in result.certs {
        roots.add(cert).ok();
    }
    ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

fn format_pg_value(v: &Value) -> String {
    match v {
        Value::Null => "NULL".to_string(),
        Value::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        _ => "NULL".to_string(),
    }
}

fn row_to_map(row: &Row) -> Map<String, Value> {
    let mut map = Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let value = col_to_json(row, i, col.type_());
        map.insert(col.name().to_string(), value);
    }
    map
}

fn col_to_json(row: &Row, idx: usize, typ: &Type) -> Value {
    match *typ {
        Type::BOOL => row.try_get::<_, Option<bool>>(idx)
            .ok().flatten().map(Value::Bool).unwrap_or(Value::Null),

        Type::INT2 => row.try_get::<_, Option<i16>>(idx)
            .ok().flatten().map(|v| Value::Number(v.into())).unwrap_or(Value::Null),

        Type::INT4 => row.try_get::<_, Option<i32>>(idx)
            .ok().flatten().map(|v| Value::Number(v.into())).unwrap_or(Value::Null),

        Type::INT8 => row.try_get::<_, Option<i64>>(idx)
            .ok().flatten().map(|v| Value::Number(v.into())).unwrap_or(Value::Null),

        Type::FLOAT4 => row.try_get::<_, Option<f32>>(idx)
            .ok().flatten()
            .and_then(|v| serde_json::Number::from_f64(v as f64))
            .map(Value::Number).unwrap_or(Value::Null),

        Type::FLOAT8 => row.try_get::<_, Option<f64>>(idx)
            .ok().flatten()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number).unwrap_or(Value::Null),

        Type::NUMERIC => row.try_get::<_, Option<Decimal>>(idx)
            .ok().flatten()
            .map(|v| {
                serde_json::from_str::<Value>(&v.to_string()).unwrap_or(Value::Null)
            })
            .unwrap_or(Value::Null),

        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
            row.try_get::<_, Option<String>>(idx)
                .ok().flatten().map(Value::String).unwrap_or(Value::Null)
        }

        Type::UUID => row.try_get::<_, Option<uuid::Uuid>>(idx)
            .ok().flatten()
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),

        Type::TIMESTAMP => row.try_get::<_, Option<chrono::NaiveDateTime>>(idx)
            .ok().flatten()
            .map(|v| Value::String(v.format("%Y-%m-%d %H:%M:%S%.6f").to_string()))
            .unwrap_or(Value::Null),

        Type::TIMESTAMPTZ => row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx)
            .ok().flatten()
            .map(|v| Value::String(v.format("%Y-%m-%d %H:%M:%S%.6f").to_string()))
            .unwrap_or(Value::Null),

        Type::DATE => row.try_get::<_, Option<chrono::NaiveDate>>(idx)
            .ok().flatten()
            .map(|v| Value::String(v.format("%Y-%m-%d").to_string()))
            .unwrap_or(Value::Null),

        Type::JSONB | Type::JSON => row.try_get::<_, Option<Value>>(idx)
            .ok().flatten()
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),

        _ => {
            // Enums and other custom types are representable as strings.
            row.try_get::<_, Option<String>>(idx)
                .ok()
                .flatten()
                .map(Value::String)
                .unwrap_or_else(|| {
                    warn!("unsupported postgres type {:?} at index {}, returning null", typ, idx);
                    Value::Null
                })
        }
    }
}
