# Concurrency

## Tables sync concurrently

Within each tick, all tables are synced **in parallel** — one async task per table, all spawned at the same time.

```rust
for table in &self.config.tables {
    handles.push(tokio::spawn(async move {
        worker.run().await
    }));
}

for handle in handles {
    handle.await  // wait for ALL to finish
}
```

```
tick N:
  ├── users        ─────────────────┐
  ├── orders       ──────────┐      │
  └── transactions ────┘     │      │
                             ▼      ▼
                         all done → sleep(interval_ms) → tick N+1
```

The next tick only starts after **all** tables have finished. This prevents overlapping runs — if one table is slow, the others wait before the next cycle begins.

---

## Within a single table

Each table worker is sequential — it paginates through batches one at a time:

```
table worker (users):
  batch 1 → insert → advance cursor
  batch 2 → insert → advance cursor
  batch 3 → insert → advance cursor (last page, done)
```

No parallelism within a single table. This keeps cursor state simple and safe.

---

## Summary

| Level | Behaviour |
|-------|-----------|
| Across tables | Concurrent (parallel tasks per tick) |
| Within a table | Sequential (one batch at a time) |
| Across ticks | Non-overlapping (next tick waits for all tables) |
