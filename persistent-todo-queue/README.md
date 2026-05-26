# persistent-todo-queue

CLI todo app with a Borsh-persisted FIFO queue — challenge solution.

## Layout

- [src/todo.rs](src/todo.rs) — `Todo { id, description, created_at }` with `Borsh{Ser,De}serialize + Debug`.
- [src/queue.rs](src/queue.rs) — generic `Queue<T>` over `VecDeque<T>` (`enqueue`, `dequeue`, `peek`, `len`, `is_empty`).
- [src/store.rs](src/store.rs) — `Store` loads/saves a `Queue<T>` to/from a Borsh-encoded file with atomic rename on save.
- [src/main.rs](src/main.rs) — `todo` CLI with `add`, `list`, `done` subcommands.

## Build and run

```bash
cargo build
cargo run -- add "Buy chocolate"
cargo run -- add "Pay bills"
cargo run -- list
cargo run -- done
cargo run -- list
```

The queue is persisted to `todos.bin` in the current directory by default.
Use `--file <path>` to override.

## Tests

```bash
cargo test
```

Covers FIFO order, peek semantics, generic `T`, persistence roundtrip, save overwrite, reload order, and empty-file handling.

## Design notes

- `Queue<T>` is fully generic over `T`. The persistence methods on `Store` add
  `Borsh{Ser,De}serialize` bounds only where the bytes are produced/consumed,
  so the queue API itself stays unconstrained.
- The store does an atomic rename (`<file>.bin.tmp` → `<file>.bin`) so a crash
  mid-save cannot leave a partially written file.
- IDs are assigned as `max(pending ids) + 1`. When the queue empties, IDs
  restart at 1 — the spec doesn't require monotonic IDs across the lifetime
  of the queue.
- No JSON, no Serde — only Borsh, per the challenge requirements.
