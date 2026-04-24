# pg2ch

Syncs PostgreSQL tables to ClickHouse using cursor-based incremental loading.

- Automatically creates ClickHouse tables from PostgreSQL schemas
- Tracks sync position with cursor columns (e.g. `updated_at`, `id`)
- Recovers cursor state from ClickHouse on restart — no state files needed
- Syncs all tables concurrently on each tick

> **Note:** Hard deletes are not captured. Use soft deletes (`deleted_at`) if you need deletion tracking.

---

## Quick Start

```bash
cargo run << 'EOF'
interval_ms: 5000
query_batch_size: 1000
upsert_batch_size: 1000
source:
  connection_url: postgres://user:pass@localhost/mydb
destination:
  connection_url: clickhouse://default:@localhost/mydb
tables:
  - source: users
    cursors: [updated_at, id]
EOF
```

---

## Docs

- [How It Works](docs/how-it-works.md) — sync loop, cursors, batching explained simply
- [Running](docs/running.md) — all the ways to run (file, stdin, pipe, Docker)
- [Config Reference](docs/config.md) — all config fields explained with examples
- [Stdin & TTY](docs/stdin-and-tty.md) — how piped config and terminal detection work

---

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

---

## Error Handling

| Scenario                  | Behavior |
|---------------------------|----------|
| Config / connection error | Fatal — process exits |
| Schema creation fails     | Fatal — process exits |
| Cursor missing at runtime | Fatal — process exits (let orchestrator restart) |
| PostgreSQL fetch fails    | Log error, skip table this tick |
| ClickHouse insert fails   | Log error, skip table this tick; cursor not advanced, retried next tick |

---

## Limitations

- No TLS for PostgreSQL (plaintext only)
- Schema changes after table creation are not applied automatically
- Array columns are not supported (synced as `null`)
