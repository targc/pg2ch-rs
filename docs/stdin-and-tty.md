# Stdin and TTY

## How config is loaded

```rust
let arg = std::env::args().nth(1);  // first CLI arg, or None
```

```rust
match arg.as_deref() {
```
`.as_deref()` converts `Option<String>` → `Option<&str>` so we can match against string literals like `"-"`.

---

```rust
Some("-") => read_stdin()?,
```
User ran `pg2ch -`. The `-` is a Unix convention meaning "use stdin". Always reads from stdin regardless of TTY.

---

```rust
Some(path) => fs::read_to_string(path)...?,
```
User ran `pg2ch config.yaml`. Just read that file. No TTY check needed.

---

```rust
None => {
    if !is_tty() {
        read_stdin()?
```
No CLI arg given. Check if something is piped in:
- `cat config.yaml | pg2ch` → `is_tty()` returns `false` → `!is_tty()` is `true` → read stdin

---

```rust
    } else {
        fs::read_to_string("config.yaml")...?
    }
```
- `pg2ch` (interactive, nothing piped) → `is_tty()` returns `true` → `!is_tty()` is `false` → fall back to reading `config.yaml` from disk

Without this branch, running `pg2ch` interactively with no args would hang waiting for keyboard input.

---

## Decision tree

```
arg given?
├── yes, "-"      → stdin (explicit)
├── yes, "path"   → read file
└── no
    ├── pipe/heredoc (isatty=false) → stdin (auto-detected)
    └── interactive  (isatty=true)  → config.yaml (fallback)
```

---

## What is a TTY?

`isatty(fd)` is a kernel syscall that asks: **"is this file descriptor connected to a real terminal?"**

- A terminal (keyboard) is backed by a TTY device (`/dev/pts/0`, etc.)
- A pipe is backed by a pipe buffer in kernel memory

```
fd 0 → TTY device  → isatty = true  → keyboard (interactive)
fd 0 → pipe buffer → isatty = false → piped data
```

**TTY** itself stands for TeleTYpewriter — a historical name for the physical terminal. Today it just means "the keyboard/terminal session".
