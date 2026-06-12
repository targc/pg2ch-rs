# Running pg2ch

## Ways to Provide Config

pg2ch looks for config in this order:

| Priority | How | Example |
|----------|-----|---------|
| 1 | CLI argument (file path) | `pg2ch config.yaml` |
| 2 | Explicit stdin flag | `pg2ch -` |
| 3 | Piped stdin (no TTY) | `cat config.yaml \| pg2ch` |
| 4 | Default fallback | `pg2ch` → reads `./config.yaml` |

---

## With `cargo run`

Note the `--` separator — it tells cargo to pass what follows to the binary, not to cargo itself.

**From a file:**
```bash
cargo run -- config.yaml
```

**Piped inline (no file needed):**
```bash
cat config.yaml | cargo run
```

**Heredoc inline:**
```bash
cargo run << 'EOF'
interval_ms: 5000
batch_size: 1000
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

## Built Binary

```bash
cargo build --release

./target/release/pg2ch config.yaml
cat config.yaml | ./target/release/pg2ch
```

---

## Docker

```bash
docker build -t pg2ch .

# Mount a config file
docker run -v $(pwd)/config.yaml:/config.yaml pg2ch

# Pipe config inline (-i keeps stdin open)
cat config.yaml | docker run -i pg2ch
```

---

## Log Level

Controlled by the `RUST_LOG` environment variable. Default is `info`.

```bash
RUST_LOG=debug cargo run -- config.yaml   # verbose
RUST_LOG=warn  cargo run -- config.yaml   # quiet
```

---

## What is "stdin" and "TTY"?

When you run `cat config.yaml | pg2ch`, the config YAML is sent through a **pipe** — stdin is no longer your keyboard, it's data from the previous command.

A **TTY** (terminal) means stdin is your keyboard (interactive). pg2ch checks whether stdin is a pipe or a TTY to decide whether to read from it automatically:

- **Pipe detected** → read config from stdin
- **TTY detected** → fall back to `config.yaml` file (otherwise the program would hang waiting for you to type)
