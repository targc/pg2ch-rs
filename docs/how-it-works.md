# How It Works

## The Big Picture

pg2ch copies rows from PostgreSQL into ClickHouse continuously. It does this using a **cursor** — a bookmark that tracks "where we left off" — so each sync only fetches rows that are new since the last run.

```
PostgreSQL  ──────────────────────────────▶  ClickHouse
            SELECT new rows (via cursor)       INSERT (upsert)
```

---

## Startup

When pg2ch starts, it does three things before syncing:

**1. Schema sync**
For each table in your config, it checks if the table exists in ClickHouse. If not, it reads the schema from PostgreSQL and creates it automatically.

**2. Cursor init**
It queries ClickHouse for the last (most recent) row in each table:
```sql
SELECT updated_at, id FROM users ORDER BY updated_at DESC, id DESC LIMIT 1
```
This becomes the starting cursor. If the ClickHouse table is empty, the cursor starts at zero — triggering a full initial load.

**3. Sync loop**
Runs a continuous loop: sync all tables → wait `interval_ms` → repeat.

---

## The Cursor

A cursor is just the value(s) of certain columns from the last row that was synced. You configure which columns to use:

```yaml
cursors:
  - updated_at
  - id
```

Each tick, pg2ch fetches only rows that come *after* the cursor:

```sql
SELECT * FROM users
WHERE (updated_at, id) > ('2024-01-10 12:00:00', 99)
ORDER BY updated_at, id
LIMIT 1000
```

After inserting those rows into ClickHouse, the cursor advances to the last row's values. Next tick, it picks up from there.

```
Cursor: (2024-01-10, 99)
         │
         ▼
  Fetch rows where (updated_at, id) > cursor
         │
         ▼
  Insert into ClickHouse
         │
         ▼
  Cursor: (2024-01-11, 150)   ← advanced to last row
```

---

## Batching

If there are many new rows, they're fetched in pages:

```
tick
 ├── fetch page 1 (1000 rows) → insert → advance cursor
 ├── fetch page 2 (1000 rows) → insert → advance cursor
 ├── fetch page 3 (300 rows)  → insert → advance cursor
 └── page < 1000 → done, wait interval_ms
```

The batch sizes are configurable (`query_batch_size`, `upsert_batch_size`).

---

## Multiple Tables

All tables sync **concurrently** within each tick — one task per table running in parallel.

---

## Restarts

Cursor state lives in memory. On restart, pg2ch re-initializes cursors from ClickHouse — it queries the last row again and picks up from there. No state files, no external coordination needed.

---

## What It Does NOT Do

- **Hard deletes** — if you `DELETE FROM users WHERE id = 5` in PostgreSQL, that row stays in ClickHouse. Use a `deleted_at` column (soft delete) if you need deletion tracking.
- **Schema changes** — if you add a column to PostgreSQL after the ClickHouse table was created, pg2ch won't alter the ClickHouse table automatically.
