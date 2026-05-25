# generic-storage-msf

Generic storage system with multiple serialization formats — challenge solution.

## About wincode

[`wincode`](https://docs.rs/wincode/) is a bincode-compatible binary
serializer/deserializer maintained by the Anza team (Solana). It produces the
same byte layout as bincode but is optimized for in-place initialization,
which makes deserialization faster on the hot path inside the Solana stack.

## Layout

- `src/lib.rs` — `Serializer<T>` trait, `Borsh` / `Wincode` / `Json`
  implementations, `Storage<T, S>` struct backed by `PhantomData<T>`, and unit
  tests.
- `examples/demo.rs` — runnable example that saves/loads a `Person` value with
  all three formats and demonstrates conversion between them.
- `benches/serializers.rs` — `criterion` benchmark comparing the three formats.

## Design notes

The trait is declared as `Serializer<T>` (rather than exposing a generic
`fn to_bytes<T>(…)` method) because each backend requires different bounds on
`T`:

- **Borsh** needs `BorshSerialize + BorshDeserialize`.
- **Wincode** needs `wincode::Serialize<Src = T> + wincode::DeserializeOwned<Dst = T>`
  (derived via `#[derive(SchemaWrite, SchemaRead)]`, gated behind the `derive`
  feature of the crate).
- **JSON** needs `serde::Serialize + serde::de::DeserializeOwned`.

Each `impl` declares its own bounds, and `Storage<T, S: Serializer<T>>`
automatically inherits whatever `S` requires.

## Usage

```rust
use generic_storage_msf::{Borsh, Storage};

let person = Person { name: "Marcelo".to_string(), age: 40 };
let mut storage: Storage<Person, _> = Storage::new(Borsh);
storage.save(&person).unwrap();
let loaded = storage.load().unwrap();
```

## Commands

```bash
cargo test                 # run the 6 unit tests
cargo run --example demo   # show all three formats side by side
cargo bench                # benchmark the three formats
```
