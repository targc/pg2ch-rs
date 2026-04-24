# pg2ch-rs

## Plans

Implementation plans live in `./plans/`. Each plan is a numbered directory:

```
plans/
├── 00001_initial/
│   ├── REQUIREMENTS.md        ← entry point (what & why)
│   ├── SYSTEM_ARCHITECTURE.md
│   ├── SYSTEM_FLOW.md
│   └── CODE_PROJECT_STRUCTURE.md
├── 00002_next_feature/
│   ├── REQUIREMENTS.md
│   └── ...
```

### Conventions
- **`REQUIREMENTS.md`** is the index of each plan — read it first to understand the plan's scope and goal.
- Plans are **ordered by prefix** (`00001`, `00002`, ...). Implement in order.
- Each plan is self-contained: its own requirements, architecture, and structure docs.
