# NFT Core Staking

An Anchor **1.0.2** program for staking **Metaplex Core** NFTs, built on `mpl-core 0.12.0`
and the Solana 3.x toolchain.

It implements two features on top of the classic "freeze-to-stake" Core pattern:

1. **`claim_rewards`** — a standalone instruction that lets a user collect accumulated
   rewards **without unstaking**, and crucially **without touching the freeze period**.
2. **Collection-level statistics** — the number of currently-staked assets in a collection,
   stored on-chain in a collection-level **Attributes** plugin (`total_staked`).

Reference material this is based on:
- [AndreiaCanadas/anchor-core-staking](https://github.com/AndreiaCanadas/anchor-core-staking) — the staking design (Anchor 0.31).
- [bergabman/anchor-1-mplxcore](https://github.com/bergabman/anchor-1-mplxcore) — how to use `mpl-core` with **Anchor 1.x** (account wrapper types, no patch sections).
- [Metaplex Core docs](https://www.metaplex.com/docs/smart-contracts/core) — plugins (FreezeDelegate, Attributes).

---

## How staking works

There is **no per-stake PDA**. All stake state lives on the Core asset itself:

| Where | Plugin | Data |
|-------|--------|------|
| Asset | `FreezeDelegate` | `frozen: true` while staked (locks transfer/burn) |
| Asset | `Attributes` | `staked`, `staked_at`, `last_claim` |
| Collection | `Attributes` | `total_staked` |

Rewards are an SPL token (6 decimals) minted by the program. One mint per collection.

### PDAs

| PDA | Seeds | Role |
|-----|-------|------|
| `config` | `[b"config", collection]` | stake settings + **mint authority** of the rewards token |
| `update_authority` | `[b"update_authority", collection]` | the collection's update authority; **signs every plugin CPI** |
| `rewards_mint` | `[b"rewards_mint", config]` | the rewards token mint |

When a collection is created (`create_collection`), its update authority is set to the
`update_authority` PDA. That lets the program later mutate plugins on **any** asset in the
collection (freeze/thaw, attributes) by signing as that PDA — without needing the asset
owner for every action.

### Two clocks (the key design decision)

The original reference uses a single `staked_at` timestamp for **both** reward accrual
**and** freeze-period enforcement. That creates a problem: if `claim_rewards` reset
`staked_at` (to avoid paying the same time twice), it would also reset the freeze clock, so
a user who claims would be forced to wait another full freeze period before they could
unstake.

This program splits the two concerns into **two independent timestamps**:

- **`staked_at`** — set once at `stake`, used **only** for the freeze period. Never moved by `claim_rewards`.
- **`last_claim`** — the reward checkpoint. Advances on every `claim_rewards` and at `unstake`.

Reward accrual counts **whole staked days** and advances `last_claim` by exactly the days
consumed, so leftover seconds keep accruing toward the next claim (no rounding loss).

```
reward_base_units = whole_days * rewards_bps * 10^decimals / 10_000
```

Result: a user can `claim_rewards` and then `unstake` in the very same block, as long as the
**original** freeze period has elapsed. This is verified by the
`claim_then_unstake_same_block` test.

---

## Instructions

| Instruction | Description |
|-------------|-------------|
| `initialize(rewards_bps, freeze_period)` | Create the per-collection `Config` and rewards mint. |
| `create_collection(name, uri)` | Create a Core collection owned by the program PDA, seeded with `total_staked = 0`. |
| `mint_asset(name, uri)` | Mint a Core asset into the collection (demo/test helper). |
| `stake()` | Freeze the asset, write `staked`/`staked_at`/`last_claim`, **+1** to `total_staked`. |
| `claim_rewards()` | Mint rewards accrued since `last_claim`; advance `last_claim` only. Asset stays frozen & staked. **Does not affect the freeze period.** |
| `unstake()` | Require freeze period elapsed (from `staked_at`), pay remaining rewards, thaw, reset attributes, **−1** to `total_staked`. |

---

## Project layout

```
programs/nft-core-staking/src/
├── lib.rs                      # program entrypoints
├── constants.rs                # seeds, attribute keys, reward decimals
├── error.rs                    # StakeError
├── helpers.rs                  # attribute parsing, reward math, Core CPI write helpers
├── state/
│   ├── config.rs               # Config account
│   └── core_accounts.rs        # Anchor wrappers for Core BaseAssetV1 / BaseCollectionV1
└── instructions/
    ├── initialize.rs
    ├── create_collection.rs
    ├── mint_asset.rs
    ├── stake.rs
    ├── claim_rewards.rs
    └── unstake.rs
```

### Anchor 1.x ⇄ mpl-core compatibility

`mpl-core`'s own `anchor` feature pins `anchor-lang 0.31.1`, which conflicts with
`anchor-lang 1.0.2`. So this program **does not** enable that feature. Instead, `mpl-core`
is used with default (`borsh-v1`) features and we provide thin Anchor wrappers in
[`state/core_accounts.rs`](programs/nft-core-staking/src/state/core_accounts.rs):

- `MplCore` — a unit struct implementing `anchor_lang::Id` so we can write `Program<'info, MplCore>`.
- `BaseAssetV1Wrap` / `BaseCollectionV1Wrap` — newtypes over the Core account structs implementing
  `AccountDeserialize` (validating the 1-byte `Key`), a no-op `AccountSerialize`, `Owner = mpl_core::ID`,
  `Discriminator = &[]`, `Deref` to the inner type, and a `#[cfg(feature = "idl-build")] impl IdlBuild`
  so `anchor build`'s IDL generation compiles.

Because `mpl-core 0.12.0` and Anchor 1.0.2 both target the Solana 3.x crate generation, **no
`[patch.crates-io]` overrides are needed**.

---

## Build & test

Requires: `anchor-cli 1.0.2`, `solana-cli 3.x`, Rust 1.89+.

```bash
anchor build --no-idl   # see "IDL generation" below for why --no-idl
cargo test --manifest-path programs/nft-core-staking/Cargo.toml
```

The tests are self-contained [LiteSVM](https://github.com/LiteSVM/litesvm) integration tests
([`tests/staking.rs`](programs/nft-core-staking/tests/staking.rs)). They load the real
`mpl-core` program from `tests/fixtures/mpl_core.so` and warp the clock to validate the
freeze period and reward accrual. They cover:

- `stake_sets_collection_counter_and_freezes` — staking sets `total_staked = 1`.
- `unstake_before_freeze_period_fails` — unstaking before the freeze period errors.
- `claim_rewards_mints_without_unstaking` — claiming after 2 days mints `100_000` base units, asset stays staked.
- `claim_then_unstake_same_block` — **claim then immediately unstake succeeds** (freeze clock untouched by claim).

> The `tests/fixtures/mpl_core.so` binary was produced with
> `solana program dump CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d mpl_core.so --url devnet`.
> Re-dump it if you need to refresh it.

### Stack-offset warnings — fixed via a local mpl-core fork

Upstream `mpl-core 0.12.0` makes `cargo build-sbf` emit three non-fatal
`Stack offset … exceeded max offset of 4096` messages for its client-side decode helpers:

```
mpl_core::hooked::plugin::registry_records_to_plugin_list   (frame 4480, +144)
mpl_core::hooked::asset::Asset::deserialize                 (frame 4288,  +8)
mpl_core::hooked::collection::Collection::deserialize       (frame 4224, +48)
```

The big stack local is the 19-field [`PluginsList`](vendor/mpl-core/src/hooked/advanced_types.rs)
struct. This repo silences all three with a vendored fork at [`vendor/mpl-core`](vendor/mpl-core),
wired in via `[patch.crates-io]` in the root [`Cargo.toml`](Cargo.toml), applying the two
techniques from [this Solana StackExchange answer](https://solana.stackexchange.com/questions/11706):

1. **`#[inline(never)]`** on `registry_records_to_plugin_list` and on the `deserialize_with_plugins`
   helpers, so those large frames are not inlined into `Asset::deserialize` / `Collection::deserialize`.
2. **Heap allocation (`Box`)** of the `PluginsList` accumulator inside
   `registry_records_to_plugin_list`, moving the 19-field struct off the stack frame.

Result: `anchor build` is **warning-free** (0 stack-offset messages), and the integration tests
still pass — confirming the patched `Collection::from_bytes` decode is behaviour-preserving.

> These functions are also dead code on-chain (this program reads plugins via `fetch_plugin`,
> not the `Asset`/`Collection` advanced decode), so the warnings were never a runtime risk — the
> fork just keeps the build output clean.

### IDL generation

With `anchor-cli 1.0.2` against this virtual-workspace layout, the `anchor build` IDL step does
not pass `--features idl-build` to its test invocation, so it runs the cached non-feature test
binary and reports `Error: IDL doesn't exist`. Work around it by building the `.so` with
`anchor build --no-idl` and generating the IDL directly from the (working) idl-build test:

```bash
# emits the IDL JSON between "--- IDL begin program ---" / "--- IDL end program ---"
cargo test --features idl-build \
  --manifest-path programs/nft-core-staking/Cargo.toml \
  --lib -- --nocapture --test-threads=1 __anchor_private_print_idl
```

The assembled IDL is committed at [`idl/nft_core_staking.json`](idl/nft_core_staking.json).
`anchor deploy` itself works fine and even publishes this IDL on-chain.

---

## Deployment (devnet)

Deployed and verified on **devnet**:

| | |
|---|---|
| Program ID | `5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS` |
| ProgramData | `4ZZywDQhNnUv4jR6CypncAbYz4hWYexEcx91qBU6AwPD` |
| Upgrade authority | `3njzSa5GMB7nPyP4xwKdmMS9KMhc7DF3yjHhcFG5YTSy` |
| Size / rent | 450,808 bytes · ~3.14 SOL · on-chain IDL published |

```bash
# Anchor.toml is configured for [provider] cluster = "devnet"
anchor deploy
solana program show 5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS --url devnet
```

> **Note on a moved project:** Cargo embeds absolute paths in `target/`, and `target/deploy/`
> holds the program keypair. If you `cargo clean` or move the folder and lose that keypair,
> Anchor generates a new program ID — run `anchor keys sync` and update the `PROGRAM_ID`
> constant in [`tests/staking.rs`](programs/nft-core-staking/tests/staking.rs) to match.
>
> To avoid that, the program keypair is backed up (outside `target/`) at
> [`keys/nft_core_staking-keypair.json`](keys/). After a `cargo clean`, restore it with
> `cp keys/nft_core_staking-keypair.json target/deploy/`. The sensitive **upgrade authority**
> wallet is kept outside the repo and is never committed — see [`keys/README.md`](keys/README.md).

### Live devnet run (verification)

The full flow was executed on **devnet** with [`scripts/devnet_e2e.js`](scripts/devnet_e2e.js)
(`node scripts/devnet_e2e.js`). Every operation landed on-chain — click a signature to inspect it:

| Operation | Devnet transaction |
|---|---|
| `create_collection` | [`5FZY8Com43hAPr1d…`](https://explorer.solana.com/tx/5FZY8Com43hAPr1dpctESZuNRxZjXBSMUweuEhsnwPkGKM5xcFKb28NG3CvcrYFQvnsFamVkEHpjbzA8tqkTqeL2?cluster=devnet) |
| `initialize` | [`3afATvf8ML2YWTCd…`](https://explorer.solana.com/tx/3afATvf8ML2YWTCdNXoYPbnZLspd93jptCyqN1gBYahrkQQ2R1GuD3Bpse7jRKvYh2tLng91SSKmkJvorTRMBaCQ?cluster=devnet) |
| `mint_asset` | [`oh3JFJQtHHNdThic…`](https://explorer.solana.com/tx/oh3JFJQtHHNdThicbnsFKucZnZGAaHjDLa7z4TAugNHzd5zcXxhZKp7NpdQVYofDamnt1Gfvq3w6DmRfhYetcBm?cluster=devnet) |
| `stake` | [`ohvxGSbfehDchyph…`](https://explorer.solana.com/tx/ohvxGSbfehDchyphqxwYM76qnSWqkG6WPHRCUn7zmbgqZJxpcLsWaLTEUQ16CfHHsay1CeXyTgaYngnnkpb28r3?cluster=devnet) |
| `claim_rewards` | [`5d7mH8b9DF89SCXW…`](https://explorer.solana.com/tx/5d7mH8b9DF89SCXWYCY8h4jnrbY3TMGSkedSLaGvsmLnUqqnA1dniyPNxzdVjjHN9YZiDouqrEfCeyJwok8GKCRi?cluster=devnet) |
| `unstake` | [`Q5D48UM8biHgVm41…`](https://explorer.solana.com/tx/Q5D48UM8biHgVm41ogcgyreWnTvzZfVHZrK52hxbzU9D2KdVBNNuJxijYhBtL9fbGSAYuxQ2BWJ9jovPCptWZS7?cluster=devnet) |

- Collection: [`8DewGAkLE8BXgXJB4qKukosUxQemZpa3uXqx5nhAzK4B`](https://explorer.solana.com/address/8DewGAkLE8BXgXJB4qKukosUxQemZpa3uXqx5nhAzK4B?cluster=devnet)
- Asset: [`d29c9iXv6s1s2C1tkUgWgjDDgy4wGZH5v8zhegmAER8`](https://explorer.solana.com/address/d29c9iXv6s1s2C1tkUgWgjDDgy4wGZH5v8zhegmAER8?cluster=devnet)

> This run uses `freeze_period = 0` so `unstake` is allowed immediately, and `claim_rewards`
> succeeds but mints **0** because less than one whole staked day elapsed in real time (rewards
> and the freeze period are measured in days). The reward arithmetic and the
> "claim doesn't reset the freeze clock" guarantee are proven deterministically in the LiteSVM
> tests via clock warping.

---

## Reward rate example

With `REWARDS_DECIMALS = 6` and `rewards_bps = 500`:

```
tokens per staked day = 500 / 10_000 = 0.05 token = 50_000 base units
```

So 2 staked days → `100_000` base units (0.1 token).
