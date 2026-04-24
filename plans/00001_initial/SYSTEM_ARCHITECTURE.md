# System Architecture — pg2ch

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        pg2ch (Rust)                         │
│                                                             │
│  ┌──────────┐    ┌──────────────┐    ┌───────────────────┐  │
│  │  Config  │───▶│   Startup    │───▶│   Sync Engine     │  │
│  │  Loader  │    │  (init phase)│    │  (ticker loop)    │  │
│  └──────────┘    └──────────────┘    └───────────────────┘  │
│                         │                      │            │
│                         ▼                      ▼            │
│                  ┌────────────┐       ┌─────────────────┐   │
│                  │  Schema    │       │  Table Workers  │   │
│                  │  Manager   │       │  (per table)    │   │
│                  └────────────┘       └─────────────────┘   │
│                                                             │
└───────────────┬──────────────────────────────┬─────────────┘
                │                              │
                ▼                              ▼
        ┌──────────────┐              ┌──────────────────┐
        │  PostgreSQL  │              │   ClickHouse     │
        │  (source)    │              │  (destination)   │
        └──────────────┘              └──────────────────┘
```

---

## Modules

### `config`
Loads and validates `config.yaml`. Produces a typed `Config` struct used throughout the app.

```
Config
├── interval_ms
├── query_batch_size
├── upsert_batch_size
├── source.connection_url
├── destination.connection_url
└── tables[]
    ├── source
    ├── dest (optional)
    └── cursors[]
```

---

### `startup` (init phase, runs once)

Two sequential steps before the ticker starts:

**Step 1 — Schema sync** (`schema_manager`)
- For each configured table, introspect PostgreSQL column types via `information_schema`.
- Check if destination table exists in ClickHouse.
- If missing: generate and execute `CREATE TABLE ... ENGINE = ReplacingMergeTree(...)`.
- Type mapping: see REQUIREMENTS.md.

**Step 2 — Cursor init** (`cursor_store`)
- For each table, query ClickHouse for the last record:
  ```sql
  SELECT col1, col2 FROM dest_table ORDER BY col1 DESC, col2 DESC LIMIT 1
  ```
- If table is empty → cursor = zero/min values (full table load).
- Populate in-memory `CursorStore` (hash map keyed by table name).
- If any cursor fails to initialize → fatal error, process exits.

---

### `sync_engine` (ticker loop)

Drives the sync cycle. Uses a non-overlapping ticker:

```
loop {
    sync_all_tables().await   // runs all table workers sequentially or concurrently
    sleep(interval_ms).await
}
```

Tables can be synced **concurrently** (one task per table) within each tick.

---

### `table_worker` (per-table sync)

Runs the extract → transform → load pipeline for a single table.

```
loop {
    1. Read cursor from CursorStore (error + restart if missing)
    2. SELECT batch from PostgreSQL using cursor
    3. If batch is empty → break (table is caught up)
    4. INSERT batch into ClickHouse
    5. Update cursor in CursorStore to last row's cursor values
    6. If batch < query_batch_size → break (last page)
    7. Else → continue (more pages)
}
```

**Extract** (PostgreSQL)
```sql
SELECT * FROM source_table
WHERE (cursor_col1, cursor_col2) > ($last1, $last2)
ORDER BY cursor_col1, cursor_col2
LIMIT $query_batch_size
```

**Load** (ClickHouse)
- Rows are split into chunks of `upsert_batch_size`.
- Each chunk is inserted via `INSERT INTO dest_table VALUES (...)`.
- ClickHouse `ReplacingMergeTree` handles deduplication by primary key.

---

### `cursor_store`

Thread-safe in-memory store.

```
CursorStore: HashMap<TableName, Vec<CursorValue>>
```

- Initialized at startup from ClickHouse.
- Updated after each successful batch insert.
- Read at start of each sync cycle per table.
- Missing entry at runtime = unrecoverable error.

---

### `schema_manager`

Handles schema introspection and DDL.

- Reads PostgreSQL `information_schema.columns` for column names, types, nullability.
- Maps PG types → CH types.
- Generates `CREATE TABLE` DDL for ClickHouse with `ReplacingMergeTree`.
- The primary key / `ORDER BY` for ClickHouse is derived from the cursor columns.

---

## Startup Sequence

```
1. Load config
2. Connect to PostgreSQL
3. Connect to ClickHouse
4. schema_manager: sync all table schemas
5. cursor_store: init all cursors from ClickHouse
6. Start sync_engine ticker loop
```

Any failure in steps 1–5 = fatal, process exits (let container orchestrator restart).

---

## Error Handling

| Scenario                          | Behavior                        |
|-----------------------------------|---------------------------------|
| Config invalid                    | Fatal exit at startup           |
| PG/CH connection failure          | Fatal exit at startup           |
| Schema creation fails             | Fatal exit at startup           |
| Cursor init fails                 | Fatal exit at startup           |
| Cursor missing at runtime         | Fatal exit (restart)            |
| Batch SELECT fails                | Log error, skip table this tick |
| Batch INSERT fails                | Log error, skip table this tick (cursor not advanced) |
| Partial batch inserted            | Not retried; next tick re-fetches from last good cursor |

---

## File Structure (proposed)

```
src/
├── main.rs              # startup sequence, connects components
├── config.rs            # config loading and validation
├── schema_manager.rs    # schema introspection + DDL generation
├── cursor_store.rs      # in-memory cursor state
├── sync_engine.rs       # ticker loop
├── table_worker.rs      # per-table ETL pipeline
├── pg_client.rs         # PostgreSQL query helpers
├── ch_client.rs         # ClickHouse insert helpers
└── type_map.rs          # PG → CH type mapping
```
