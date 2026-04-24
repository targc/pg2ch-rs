# pg2ch

Syncs PostgreSQL tables to ClickHouse using cursor-based incremental loading.

## How It Works

1. On startup, creates missing ClickHouse tables by introspecting PostgreSQL schemas.
2. Initializes cursors from the last record in each ClickHouse table (full load if empty).
3. Runs a ticker loop — each tick queries new rows from PostgreSQL and upserts them into ClickHouse.

Cursor state is in-memory. ClickHouse is the source of truth on restart — no state files needed.

> **Note:** Cursor-based sync does not capture hard deletes. Use soft deletes (`deleted_at`) if you need deletion tracking.

## Setup

### 1. Create config

```bash
cp config.example.yaml config.yaml
```

Edit `config.yaml`:

```yaml
interval_ms: 5000        # wait between sync cycles (after current finishes)
query_batch_size: 1000   # rows per SELECT from PostgreSQL
upsert_batch_size: 1000  # rows per INSERT into ClickHouse

source:
  connection_url: postgres://user:pass@localhost/mydb

destination:
  connection_url: clickhouse://default:@localhost/mydb

tables:
  - source: users          # PostgreSQL table name
    dest: users            # ClickHouse table name (optional, defaults to source)
    cursors:
      - updated_at         # cursor columns, ordered — used for WHERE and ORDER BY
      - id
```

### 2. Run

**Binary:**
```bash
cargo build --release
./target/release/pg2ch
```

**Docker:**
```bash
docker build -t pg2ch .
docker run -v $(pwd)/config.yaml:/config.yaml pg2ch
```

Log level is controlled via `RUST_LOG` (default: `info`):
```bash
RUST_LOG=debug ./target/release/pg2ch
```

## Cursor Strategy

Each table must have one or more cursor columns (typically `updated_at` + `id`). On each tick:

```sql
-- PostgreSQL fetch
SELECT * FROM users
WHERE (updated_at, id) > ($last_updated_at, $last_id)
ORDER BY updated_at, id
LIMIT 1000
```

The ClickHouse table is created with `ReplacingMergeTree` ordered by the cursor columns for deduplication.

## Type Mapping

| PostgreSQL            | ClickHouse              |
|-----------------------|-------------------------|
| `int2`, `int4`        | `Int32`                 |
| `int8`                | `Int64`                 |
| `float4`, `float8`    | `Float64`               |
| `numeric` / `decimal` | `Decimal(P, S)`         |
| `boolean`             | `Bool`                  |
| `text`, `varchar`     | `String`                |
| `uuid`                | `UUID`                  |
| `timestamp`           | `DateTime64(6)`         |
| `timestamptz`         | `DateTime64(6, 'UTC')`  |
| `date`                | `Date`                  |
| `jsonb`, `json`       | `String`                |
| `_type` (array)       | `Array(T)`              |
| nullable column       | `Nullable(T)`           |

## Error Handling

| Scenario                    | Behavior                                      |
|-----------------------------|-----------------------------------------------|
| Config / connection failure | Fatal — process exits (let orchestrator restart) |
| Schema creation fails       | Fatal                                         |
| Cursor missing at runtime   | Fatal                                         |
| PostgreSQL fetch fails      | Log error, skip table this tick               |
| ClickHouse insert fails     | Log error, skip table this tick (cursor not advanced, retried next tick) |

## Limitations

- No TLS support for PostgreSQL connections (plaintext only in current build).
- Hard deletes in PostgreSQL are not replicated — use soft deletes.
- Array column values are not supported (returned as `null`).
