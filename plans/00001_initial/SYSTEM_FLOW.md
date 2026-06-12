# System Flow — pg2ch

## Overview Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│ STARTUP                                                                 │
│                                                                         │
│  config.yaml ──▶ Config ──▶ PgClient ──▶ ChClient                      │
│                                 │              │                        │
│                          SchemaManager ────────┤                        │
│                          (introspect PG,        │                       │
│                           CREATE CH tables)     │                       │
│                                                 ▼                       │
│                                          CursorStore                    │
│                                     (SELECT last row                    │
│                                      per table from CH)                 │
└─────────────────────────────────┬───────────────────────────────────────┘
                                  │  all OK
                                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ TICKER LOOP  (SyncEngine)                                               │
│                                                                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  tick                                                            │   │
│  │                                                                  │   │
│  │  ┌─ TableWorker(users) ──────────────────────────────────────┐   │   │
│  │  │  loop:                                                    │   │   │
│  │  │   cursor ──▶ SELECT batch (PG) ──▶ INSERT chunks (CH)    │   │   │
│  │  │              advance cursor                               │   │   │
│  │  │              repeat until empty or last page             │   │   │
│  │  └───────────────────────────────────────────────────────────┘   │   │
│  │  ┌─ TableWorker(orders) ─────────────────────────────────────┐   │   │
│  │  │  (same pipeline, runs concurrently)                       │   │   │
│  │  └───────────────────────────────────────────────────────────┘   │   │
│  │  ┌─ TableWorker(transactions) ───────────────────────────────┐   │   │
│  │  │  (same pipeline, runs concurrently)                       │   │   │
│  │  └───────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  │  join_all() ──▶ sleep(interval_ms)                               │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                          (repeat forever)                               │
└─────────────────────────────────────────────────────────────────────────┘
```

## TableWorker Detail

```
                    ┌──────────────────────────────────┐
                    │  CursorStore (in-memory)          │
                    │  { "users": [2024-01-10, 99] }   │
                    └───────────────┬──────────────────┘
                                    │ get cursor
                                    ▼
               ┌────────────────────────────────────────┐
               │  PostgreSQL                            │
               │  SELECT * FROM users                   │
               │  WHERE (updated_at, id) > ($1, $2)    │
               │  ORDER BY updated_at, id               │
               │  LIMIT 1000                            │
               └────────────────┬───────────────────────┘
                                │ rows []
                    ┌───────────┴──────────┐
               empty│                      │has rows
                    ▼                      ▼
                  (done)       ┌───────────────────────┐
                               │  ClickHouse           │
                               │  INSERT INTO users    │
                               │  VALUES ... (chunk)   │
                               └───────────┬───────────┘
                                           │ ok
                                           ▼
                               ┌───────────────────────┐
                               │  CursorStore          │
                               │  set cursor =         │
                               │  last row values      │
                               └───────────┬───────────┘
                                           │
                              ┌────────────┴────────────┐
                    last page │                         │ full page
                              ▼                         ▼
                           (done)               (next iteration)
```

---

## 1. Startup Flow

```
main()
 │
 ├─ load config.yaml
 │   └─ FAIL → exit(1)
 │
 ├─ PgClient::connect(source.connection_url)
 │   └─ FAIL → exit(1)
 │
 ├─ ChClient::connect(destination.connection_url)
 │   └─ FAIL → exit(1)
 │
 ├─ SchemaManager::sync_all(tables)          ← for each table:
 │   ├─ introspect PG columns (information_schema.columns)
 │   ├─ check if CH table exists
 │   └─ if missing → CREATE TABLE ... ENGINE = ReplacingMergeTree
 │       └─ FAIL → exit(1)
 │
 ├─ CursorStore::init(tables)                ← for each table:
 │   ├─ SELECT last record from CH dest table (ORDER BY cursors DESC LIMIT 1)
 │   ├─ if CH table empty → cursor = zero/min values (triggers full load)
 │   └─ populate in-memory CursorStore
 │       └─ FAIL → exit(1)
 │
 └─ SyncEngine::run()                        ← ticker loop starts
```

---

## 2. Ticker Loop

```
SyncEngine::run()
 │
 └─ loop:
     ├─ sync_all_tables()     ← wait until ALL tables finish
     └─ sleep(interval_ms)    ← then wait before next cycle
```

Tables are synced **concurrently** within each cycle (one async task per table, all joined before sleeping).

---

## 3. Per-Table Sync Flow (`TableWorker::run`)

```
TableWorker::run(table)
 │
 └─ loop:                                    ← paginate until caught up
     │
     ├─ cursors.get(table.source)
     │   └─ MISSING → fatal error, process exits
     │
     ├─ pg.fetch_batch(table, cursor, batch_size)
     │   └─ FAIL → log error, break (skip table this tick)
     │
     ├─ if rows.is_empty() → break           ← table is caught up
     │
     ├─ ch.insert_batch(table.dest, rows)
     │   └─ FAIL → log error, break          ← cursor NOT advanced
     │
     ├─ cursors.set(table.source, last_row.cursor_values())
     │
     └─ if rows.len() < batch_size → break           ← last page reached
```

---

## 4. PostgreSQL Fetch Query

Cursor columns are compared as a tuple for correct ordering:

```sql
-- single cursor (e.g. updated_at + id)
SELECT *
FROM   {source_table}
WHERE  (updated_at, id) > ($1, $2)
ORDER BY updated_at ASC, id ASC
LIMIT  {batch_size}
```

On first run (empty CH table), cursor values are `(MIN, MIN)` so the `WHERE` clause matches all rows.

---

## 5. ClickHouse Insert

The batch is inserted as a single statement:

```
INSERT INTO {dest_table} (col1, col2, ...) VALUES (...)
```

ClickHouse `ReplacingMergeTree` deduplicates by primary key asynchronously.
The `ORDER BY` key on CH table = cursor columns (same order as config `cursors[]`).

---

## 6. Cursor Init Detail

```
ch.fetch_last_cursor(table)
 │
 ├─ query:
 │    SELECT {cursor_cols}
 │    FROM   {dest_table}
 │    ORDER BY {cursor_cols DESC}
 │    LIMIT 1
 │
 ├─ if row found    → CursorValues = [val1, val2, ...]
 └─ if no rows      → CursorValues = [MIN_VALUE, MIN_VALUE, ...]
                       (full table load on first tick)
```

`MIN_VALUE` per type:
| Type         | Min value              |
|--------------|------------------------|
| Int / Float  | `0`                    |
| DateTime64   | `1970-01-01 00:00:00`  |
| UUID         | `00000000-0000-...`     |
| String       | `""`                   |

---

## 7. Error Summary

| Phase        | Error                        | Action                        |
|--------------|------------------------------|-------------------------------|
| Startup      | Config invalid               | exit(1)                       |
| Startup      | PG/CH connect fail           | exit(1)                       |
| Startup      | Schema creation fail         | exit(1)                       |
| Startup      | Cursor init fail             | exit(1)                       |
| Runtime      | Cursor missing for table     | exit(1) — let orchestrator restart |
| Runtime      | PG fetch fails               | log error, skip table this tick |
| Runtime      | CH insert fails              | log error, skip table this tick (cursor not advanced, retried next tick) |
