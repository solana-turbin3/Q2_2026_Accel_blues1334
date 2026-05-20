# accel_anchor-esrow

This repository contains **two Rust/Anchor projects**:

1. The main Anchor workspace (root) with:
   - `programs/anchor-escrow-q2-2026`: escrow example program
   - `programs/withelist-q2-2026`: Token-2022 transfer-hook whitelist program
2. A standalone copy/variant of the whitelist transfer-hook feature:
   - `whitelist-transfer-hook-q2-2026/`

All projects are on **Anchor `1.0.2`** and the token-related tests are written to work with **Token-2022**.

## Programs

- **`anchor_escrow_q2_2026`**
  - **Path:** `programs/anchor-escrow-q2-2026/`
  - **Localnet Program ID (Anchor.toml):** `d5Yda6GmqiHgyhf9skzY3ffFRu3Ku9hhqNEToPguytW`

- **`withelist_q2_2026`** (transfer hook whitelist)
  - **Path:** `programs/withelist-q2-2026/`
  - **Localnet Program ID (Anchor.toml):** `AsyYG7z3cNhF2rszsC96iNoZTdKwq4QhoSa9K3WgaSxp`

- **`whitelist_transfer_hook_q2`** (standalone transfer hook project)
  - **Path:** `whitelist-transfer-hook-q2-2026/`
  - **Program ID (declared in code):** `EUkbfr6mqkXx4XFAdFaRQP79kw4ibQbEZwjmxUUkQxao`

## Prerequisites

- **Solana/Agave CLI** installed
- **Anchor CLI** `1.0.2`
- Rust toolchain compatible with your Solana/Agave installation

## Build & Test (main workspace)

From the repo root:

```bash
anchor build
```

If you get a **Program ID mismatch**, run:

```bash
anchor keys sync
anchor build
```

If you intentionally want to skip the check:

```bash
anchor build --ignore-keys
```

### Run tests

The Rust tests use LiteSVM:

```bash
cargo test --workspace
```

Note:

- The `anchor build` command may print `Error: IDL doesn't exist` after compiling tests. This comes from the test runner and **does not mean the build failed**.

## Build & Test (standalone `whitelist-transfer-hook-q2-2026`)

From `whitelist-transfer-hook-q2-2026/`:

```bash
cargo test -- --nocapture
```

This project includes an end-to-end test in `tests/test_initialize.rs` that:

- **Initializes the whitelist PDA**
- **Creates a Token-2022 mint with TransferHook extension**
- **Initializes the ExtraAccountMetaList PDA** (`extra-account-metas`)
- **Verifies the transfer hook rejects transfers** when the payer is not whitelisted
- **Re-adds the payer and verifies the transfer succeeds**

## Token-2022 notes

- Tests create mints and ATAs using:
  - `spl-token-2022-interface`
  - `spl-associated-token-account-interface`
- When using Anchor account constraints like `associated_token::...` with Token-2022, the accounts must specify the token program (e.g. `associated_token::token_program = token_program`) to avoid deriving ATAs with the legacy token program.
