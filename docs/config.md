# Config Reference

Config is a YAML file. See `config.example.yaml` for a full example.

---

## Top-level fields

```yaml
interval_ms: 5000
query_batch_size: 1000
upsert_batch_size: 1000
```

| Field | Description |
|-------|-------------|
| `interval_ms` | How long to wait (in milliseconds) **after** each sync cycle finishes before starting the next one. This is not a fixed-interval cron — the next tick starts only after the current one fully completes. |
| `query_batch_size` | How many rows to fetch from PostgreSQL per SELECT. Lower = less memory, more round-trips. |
| `upsert_batch_size` | How many rows to insert into ClickHouse per INSERT. Can be the same as `query_batch_size`. |

---

## `source` — PostgreSQL

```yaml
source:
  connection_url: postgres://user:pass@host:5432/dbname
```

Standard PostgreSQL connection URL. Port defaults to `5432` if omitted.

---

## `destination` — ClickHouse

```yaml
destination:
  connection_url: clickhouse://user:pass@host:8123/dbname
```

ClickHouse connection URL. Port defaults to `8123` (HTTP API) if omitted. Password can be empty:
```yaml
connection_url: clickhouse://default:@localhost/mydb
```

---

## `tables`

```yaml
tables:
  - source: users         # required: PostgreSQL table name
    dest: ch_users        # optional: ClickHouse table name (defaults to source)
    cursors:
      - updated_at        # cursor columns, in order
      - id
```

| Field | Required | Description |
|-------|----------|-------------|
| `source` | yes | Table name in PostgreSQL |
| `dest` | no | Table name in ClickHouse. Defaults to the same as `source`. |
| `cursors` | yes | One or more columns used to track sync progress. See below. |

---

## Choosing Cursor Columns

Cursor columns determine what counts as "new" data. Good choices:

- **`updated_at` + `id`** — works for tables with an update timestamp and a numeric primary key. Catches both new rows and updates.
- **`created_at` + `id`** — for append-only tables (e.g. events, logs) where rows are never updated.
- **`id` only** — if rows are only inserted, never updated.

Rules:
1. Cursor columns must be monotonically increasing (timestamps or auto-increment IDs).
2. The combination of cursor columns must uniquely identify ordering. Using `updated_at` alone is risky because multiple rows can share the same timestamp — always pair it with `id`.
3. The order matters — list the primary sort column first.

```yaml
# Good: timestamp + id handles ties
cursors:
  - updated_at
  - id

# Risky: ties at the same timestamp could cause rows to be skipped
cursors:
  - updated_at
```

---

## Auto-created ClickHouse Tables

When pg2ch creates a ClickHouse table, it:
- Maps PostgreSQL column types to ClickHouse types (see [type mapping](../README.md#type-mapping))
- Uses `ReplacingMergeTree` engine for upsert/deduplication
- Sets `ORDER BY` to the cursor columns

Example — if your PostgreSQL table is:
```sql
CREATE TABLE users (
    id         SERIAL PRIMARY KEY,
    name       TEXT NOT NULL,
    email      TEXT,
    updated_at TIMESTAMP NOT NULL
);
```

pg2ch creates in ClickHouse:
```sql
CREATE TABLE users (
    id         Int32,
    name       String,
    email      Nullable(String),
    updated_at DateTime64(6)
) ENGINE = ReplacingMergeTree()
ORDER BY (updated_at, id);
```
