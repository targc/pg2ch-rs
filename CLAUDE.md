# pg2ch-rs

## What This Project Is

A containerized **Rust** service that syncs PostgreSQL tables to ClickHouse using **micro-batch cursor-based incremental loading**. No Kafka, no WAL/CDC, no state file — just periodic SQL queries.

## How It Works

1. On startup: auto-creates missing ClickHouse tables (mapped from PG schema).
2. Initializes in-memory cursors by querying the last record per table from ClickHouse.
3. Runs a **non-overlapping ticker loop** — syncs all tables concurrently, then sleeps `interval_ms`.
4. Each table worker paginates: `SELECT WHERE cursor > last` → `INSERT INTO CH` → advance cursor.
5. ClickHouse uses `ReplacingMergeTree` for deduplication.

## Key Design Decisions

- **Cursor state is in-memory only.** Initialized from ClickHouse on startup. Missing cursor at runtime = fatal error + restart.
- **ClickHouse is the source of truth** for cursor position (no state file).
- **Does not capture hard deletes.** Soft deletes (`deleted_at`) required for deletion tracking.
- Tables sync **concurrently** within each tick, but ticks do not overlap.

## Source Layout

```
src/
├── main.rs              # startup sequence (connect → schema → cursors → run)
├── config.rs            # Config, DbConfig, TableConfig structs
├── error.rs             # AppError enum (thiserror)
├── type_map.rs          # PG → CH column type mapping
├── schema_manager.rs    # introspects PG, creates CH tables
├── cursor_store.rs      # HashMap<table, CursorValues> in-memory store
├── sync_engine.rs       # ticker loop, spawns TableWorkers
├── table_worker.rs      # per-table paginated ETL
├── pg_client.rs         # tokio-postgres query helpers
└── ch_client.rs         # clickhouse insert helpers
```

## Key Types

```rust
// Cursor position per table (ordered, matches config cursors[])
type CursorValues = Vec<serde_json::Value>;

// Per-table config
struct TableConfig {
    source: String,
    dest: Option<String>,  // defaults to source name
    cursors: Vec<String>,  // e.g. ["updated_at", "id"]
}
```

## Core Dependencies

- `tokio-postgres` — PostgreSQL async client
- `clickhouse` (loyd) — ClickHouse async client with inserter
- `serde` + `serde_yaml` — config loading
- `thiserror` — error types
- `tracing` — structured logging
