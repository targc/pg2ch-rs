# pg2ch

A containerized Rust service that syncs PostgreSQL tables to ClickHouse using micro-batch cursor-based incremental loading.

## How It Works

1. **Schema sync** — on startup, reads source table schemas and auto-creates missing ClickHouse tables (with mapped column types).
2. **Initial load** — reads all rows from the beginning, cursor starts at zero/min.
3. **Incremental sync** — after initial load, each tick queries only rows where cursor columns advanced beyond the last seen position.
4. **Ticker interval** — waits for the current sync batch to finish before starting the next tick (not a cron). This prevents overlapping runs.
5. **Upsert to ClickHouse** — inserts rows in batches; ClickHouse table engine should be `ReplacingMergeTree` to handle deduplication.

## Cursor Strategy

Each table has one or more cursor columns (e.g., `updated_at`, `id`). The service tracks the last synced values and queries:

```sql
SELECT * FROM source_table
WHERE (updated_at, id) > ($last_updated_at, $last_id)
ORDER BY updated_at, id
LIMIT $query_batch_size
```

Cursor state is kept **in-memory**. Lifecycle:

1. **Startup** — queries each destination ClickHouse table for the last record to initialize cursors:
   ```sql
   SELECT updated_at, id FROM dest_table ORDER BY updated_at DESC, id DESC LIMIT 1
   ```
2. **Running** — cursors are always expected to be in memory. If a cursor is missing at sync time, it's an unrecoverable error — log and restart the process.

ClickHouse is the source of truth on cold start; no state file needed.

> Note: cursor-based sync does not capture hard deletes. Use soft deletes (`deleted_at`) if deletion tracking is needed.

## Configuration (YAML)

```yaml
interval_ms: 5000        # wait between sync cycles (after current cycle finishes)
query_batch_size: 1000   # rows per SELECT from PostgreSQL
upsert_batch_size: 1000  # rows per INSERT into ClickHouse

source:
  connection_url: postgres://user:pass@host/db

destination:
  connection_url: clickhouse://user:pass@host/db

tables:
  - source: example_users
    dest: example_users     # optional, defaults to source name
    cursors:
      - updated_at
      - id
  - source: example_orders
    cursors:
      - updated_at
      - id
  - source: example_transactions
    cursors:
      - created_at
      - id
```

## Type Mapping (PostgreSQL → ClickHouse)

| PostgreSQL           | ClickHouse              |
|----------------------|-------------------------|
| `int2`, `int4`       | `Int32`                 |
| `int8`               | `Int64`                 |
| `float4`, `float8`   | `Float64`               |
| `numeric`/`decimal`  | `Decimal(P, S)`         |
| `boolean`            | `Bool`                  |
| `text`, `varchar`    | `String`                |
| `uuid`               | `UUID`                  |
| `timestamp`          | `DateTime64(6)`         |
| `timestamptz`        | `DateTime64(6, 'UTC')`  |
| `date`               | `Date`                  |
| `jsonb`, `json`      | `String`                |
| `_type` (array)      | `Array(T)`              |
| nullable column      | `Nullable(T)` wrapper   |

## Deployment

- Runs as a single Docker container (no external dependencies beyond PG + CH).
- Config mounted via volume or environment-injected.
- No state file needed — cursor is recovered from ClickHouse on restart.
