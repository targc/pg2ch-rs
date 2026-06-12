# Error Handling

## Overview

| Scenario | Behavior |
|----------|----------|
| Config / connection error | Fatal — process exits |
| Schema creation fails | Fatal — process exits |
| Cursor missing at runtime | Fatal — process exits (let orchestrator restart) |
| PostgreSQL fetch fails | Log error, skip table this tick |
| ClickHouse insert fails | Log error, skip table this tick; cursor not advanced, retried next tick |

---

## Insert Failure (Upsert Error)

When a ClickHouse insert fails, the cursor is **not advanced**. The next tick re-fetches and retries from the last good position.

```rust
if let Err(e) = self.ch.insert_rows(table.dest_name(), &rows).await {
    error!("failed to insert into {}: {}", table.dest_name(), e);
    return Ok(()); // cursor not advanced; retried next tick
}

// only reached if insert succeeded
cursors.set(&table.source, new_cursor);
```

**What this looks like across ticks:**

```
tick N:
  fetch 1000 rows → insert (FAIL)
  → log error, return early
  → cursor stays at old position

tick N+1:
  fetch same 1000 rows (cursor unchanged)
  → insert (ok)
  → cursor advances
```

Since each batch is fetched and inserted as a single unit, there are no partial inserts. On failure the entire batch is retried next tick. `ReplacingMergeTree` deduplicates by `primary_key`, so re-inserted rows replace themselves.

---

## Stuck Table

If a chunk fails permanently (e.g. a bad row ClickHouse always rejects), the table gets stuck — it retries the same batch every tick and never advances.

There is currently no dead-letter or skip mechanism. To unblock manually:

1. Identify the failing rows from the logs
2. Fix the data in PostgreSQL or the ClickHouse schema
3. Restart pg2ch

---

## Fatal Errors

Fatal errors call `std::process::exit(1)` immediately. The intent is to let the container orchestrator (Docker, Kubernetes) restart the process cleanly rather than continuing in a broken state.

Cursor state is recovered from ClickHouse on restart — no data is lost.
