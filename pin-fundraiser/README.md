# Pinocchio Fundraiser

A port of the Anchor **Token Fundraiser** example to [Pinocchio], rewritten for
minimal compute-unit (CU) consumption and binary size.

A *maker* opens a campaign for an SPL token; contributors fund a program-owned
vault; once the target is met the maker releases the funds; if the campaign
expires unfunded, contributors get refunded.

| Metric            | Anchor original         | This port (Pinocchio) |
| ----------------- | ----------------------- | --------------------- |
| Program binary    | ~200–400 KB (typical)   | **28 KB**             |
| `initialize`      | ~20k–40k CU (typical)   | **~4.2k CU**          |
| `contribute`      | ~15k–30k CU (typical)   | **~2.9k CU**          |
| `check`           | ~15k–25k CU (typical)   | **~2.6k CU**          |
| `refund`          | ~15k–30k CU (typical)   | **~3.2k CU**          |

CU figures are measured by the Mollusk and LiteSVM tests in this repo against
the real compiled bytecode (Anchor figures are typical ranges for comparable
instructions, shown for scale).

[Pinocchio]: https://github.com/anza-xyz/pinocchio

---

## Layout

```
pin-fundraiser/
├── src/
│   ├── lib.rs               # entrypoint + instruction dispatch
│   ├── constants.rs         # economic constants + PDA seeds
│   ├── error.rs             # FundraiserError -> ProgramError::Custom
│   ├── pda.rs               # PDA verification helper
│   ├── state.rs             # zero-copy Fundraiser / Contributor accounts
│   └── instructions/        # one file per instruction
├── tests-mollusk/           # Rust tests (Mollusk) — runs the SBF bytecode, reports CU
└── ts/                      # TypeScript: shared client, LiteSVM tests, devnet runner
```

## Build

```bash
cargo build-sbf            # produces target/deploy/pinocchio_fundraiser.so
```

## Test

```bash
# Rust / Mollusk (CU benchmarks + correctness against the compiled .so)
# (tests-mollusk is a standalone crate, so cd into it — no -p flag)
cd tests-mollusk && cargo test -- --nocapture

# TypeScript / LiteSVM (second engine, same .so, web3.js v1 client)
cd ts && npm install && npm run test:litesvm
```

## Run on devnet

```bash
cd ts && npm install
./devnet/deploy.sh                       # builds, syncs crate::ID, deploys
export FUNDRAISER_PROGRAM_ID=<printed id>
npm run devnet                           # init -> contribute -> refund, live
```

`deploy.sh` deploys to the address of `target/deploy/pinocchio_fundraiser-keypair.json`
and rewrites `crate::ID` in `src/lib.rs` to match it (the program embeds its own
ID for PDA derivation and account ownership, so the two must agree). The devnet
runner reads `FUNDRAISER_PROGRAM_ID` so the client derives the same PDAs.

---

## Accounts & instruction encoding

All multi-account data is little-endian. The first byte of every instruction is
the discriminator.

### State (no Anchor discriminator)

`Fundraiser` (PDA `["fundraiser", maker]`, **122 bytes**):

| offset | field            | type   |
| ------ | ---------------- | ------ |
| 0      | maker            | Pubkey |
| 32     | mint_to_raise    | Pubkey |
| 64     | vault            | Pubkey |
| 96     | amount_to_raise  | u64    |
| 104    | current_amount   | u64    |
| 112    | time_started     | i64    |
| 120    | duration (days)  | u8     |
| 121    | bump             | u8     |

`Contributor` (PDA `["contributor", fundraiser, contributor]`, **8 bytes**):
`amount: u64`.

The `vault` is a dedicated PDA token account at `["vault", fundraiser]` whose
authority is the fundraiser PDA.

### Instructions

| disc | name                  | data (after disc)                         |
| ---- | --------------------- | ----------------------------------------- |
| 0    | `initialize`          | `amount:u64, duration:u8, fr_bump:u8, vault_bump:u8` |
| 1    | `contribute`          | `amount:u64, contributor_bump:u8`         |
| 2    | `check_contributions` | *(empty)*                                 |
| 3    | `refund`              | `contributor_bump:u8`                     |

Account orders are documented at the top of each file in `src/instructions/`.

---

## Optimization notes

- **No discriminators on state.** Identity comes from the PDA seeds, so we drop
  Anchor's 8-byte prefix — less rent and no (de)serialization cost.
- **True zero-copy state.** `Fundraiser`/`Contributor` are `#[repr(C)]` byte
  layouts with 1-byte alignment; fields are read/written in place. Compile-time
  asserts pin their size and alignment.
- **Bumps passed in, not searched.** The client supplies the canonical bumps;
  the program validates with a single `create_program_address` (or, for accounts
  it creates, relies on the runtime's signed-CPI check) instead of looping
  `find_program_address` on-chain.
- **Dedicated PDA vault instead of an ATA.** `initialize` creates the vault with
  one `CreateAccount` + `InitializeAccount3`, avoiding a CPI to the Associated
  Token Account program. The vault address is pinned in state and checked on
  every later instruction.
- **Minimal CPIs and reads.** `contribute` skips reading the mint entirely (the
  effective minimum is `1`), and the vault is validated by a cheap address
  comparison against the pinned `vault` field.
- **`check`/`refund` also close the vault**, returning its rent — the Anchor
  original left the vault account (and its rent) stranded.
- Build profile: `lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip=true`.

## Behavioural parity

The economic rules are reproduced 1:1 from the Anchor source, **including two
quirks in the original time checks** (kept intentionally so behaviour matches):

- `contribute` proceeds only when `duration <= elapsed_days`.
- `refund` proceeds only when `duration >= elapsed_days`.

(The example's tests use `duration = 0`, where both hold on the same day.) If you
want "still-active" semantics instead, flip those comparisons in
`src/instructions/contribute.rs` and `refund.rs` — they're each a single,
clearly-commented line.

## Security checks preserved

- Signer enforcement on `maker` / `contributor`.
- `maker` recorded in the campaign must match on `check` / `refund`.
- The vault is pinned to the address chosen at `initialize` and verified every
  call; PDA accounts are validated against their canonical derivation.
- All token movements out of the vault are signed by the fundraiser PDA.
