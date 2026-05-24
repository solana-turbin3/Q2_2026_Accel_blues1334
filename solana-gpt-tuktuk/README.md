# Solana GPT Oracle + Tuktuk

Schedules the MagicBlock Solana GPT Oracle via Helium's Tuktuk. The program sends a recurring prompt to the GPT oracle on a cron schedule and stores the LLM response on-chain through a callback.

## Status (devnet)

| Component | Address | Status |
|---|---|---|
| Program | `9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt` | Deployed |
| GptConfig PDA | `78CeW1kDLMv1e2cA1AgXuAD5sxiQRpnVABmig9n53sM7` | Initialized |
| Payer PDA (system-owned) | `7a3BXHaJijeFt7RyzTNzi7qNWzuqCSuRTUqX7XP1NDtH` | Funded |
| Task Queue (Tuktuk) | `BACUbkyYirqJHR5wd6SXu6PpXR9Y94k5B1Aat4uzusTG` | Active, `min_crank_reward=50000` |
| Cron Job (Tuktuk) | `9WgdTdQ8i3ZuURt9epgViMgVs7Ezh1rJVjQjUECK32XC` | Schedule `0 * * * * *` (every 1 min) |

Tuktuk fires `ask_gpt` every minute, which CPIs into MagicBlock's oracle `InteractWithLlm`. Sample successful execution: tx [`2Vz8t99Dwoz...`](https://explorer.solana.com/tx/2Vz8t99Dwoz27gEAqVLSK1gvi1gs2hSnt3PQWx8EwCnD85cUBwwKhFQscqiA8z1MkmPfS41KsmQNhe26Q3VnrWJU?cluster=devnet).

> **Callback note:** The MagicBlock off-chain LLM service processes interactions in sparse batches. Until they crank a batch that includes our pending Interaction, `GptConfig.latest_response` stays empty. The reference project ([`solana-gpt-tuktuk`](https://github.com/sdfgsdfg/solana-gpt-tuktuk)) is in the same state (`is_processed=false` since Feb 2026) — this is an infrastructure constraint of the public devnet oracle, not a program issue.

## Architecture

### External programs

- **GPT Oracle** (`LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab`) — MagicBlock's on-chain LLM oracle
- **Tuktuk** (`tuktukUrfhXT6ZT77QTU8RQtvgL967uRuVagWF57zVA`) — Helium's task scheduling system
- **Cron** (`cronchYQyjEEoCM4y4qF9MJVtBdwwthWNhKKr8GqWoKM`) — Tuktuk's cron extension

### Instructions

| Instruction | Description |
|---|---|
| `initialize` | Creates the `GptConfig` PDA (admin, oracle context, prompt, latest response) |
| `ask_gpt` | CPIs into the oracle's `interact_with_llm` with the stored prompt and callback info |
| `receive_response` | Callback signed by the oracle identity PDA — stores the LLM response in `GptConfig` |
| `schedule_ask_gpt` | Schedules `ask_gpt` via Tuktuk `queue_task_v0` (optional alternative to off-chain script) |

### Flow

```
1. Admin calls initialize          → creates GptConfig + oracle context
2. Admin calls yarn cron           → registers a Tuktuk cron job for ask_gpt
3. Tuktuk crank turner fires       → executes ask_gpt
4. ask_gpt CPIs into oracle        → creates an Interaction account on the oracle
5. MagicBlock off-chain LLM        → processes the Interaction (batched)
6. Oracle CPIs receive_response    → writes the LLM response into GptConfig
```

### Key design decisions

- **System-owned payer PDA** (`seeds=[b"payer"]`): Never initialized as an Anchor account so it stays system-owned. This lets it pay rent for oracle interaction accounts via `invoke_signed` without hitting "Transfer: from must not carry data".
- **Manual oracle CPI**: The oracle instruction is built by hand (discriminator + borsh) to avoid Anchor version conflicts with the oracle crate.
- **Tuktuk integration via `declare_program!`**: We load the Tuktuk IDL at compile time to get strongly-typed CPI clients without depending on the Tuktuk crate.
- **Idempotent cron setup**: `yarn cron` reuses an existing queue/cron with the same name and only funds it if the balance is low.

## Quick start

### Prerequisites

- Solana CLI configured for devnet (`solana config set --url devnet`)
- A wallet with ≥ 0.15 SOL at `~/.config/solana/id.json`
- A Helius (or other dedicated) RPC API key — public devnet rate-limits the crank turner. Free key at https://helius.dev.

### Build and deploy

```bash
yarn install
anchor build --no-idl
solana program deploy target/deploy/solana_gpt_tuktuk.so \
  --program-id target/deploy/solana_gpt_tuktuk-keypair.json \
  --with-compute-unit-price 100000
```

`--no-idl` is required: Anchor CLI 1.0.x cannot extract an IDL from a program that uses `declare_program!`. The `.so` builds and deploys correctly.

### End-to-end test (manual single call)

```bash
yarn test
```

[`test.ts`](test.ts) walks five steps: create oracle context → `initialize` GptConfig → fund the Payer PDA → call `ask_gpt` → poll for the response.

### Schedule with Tuktuk

Edit [`tuktuk/crank-devnet.toml`](tuktuk/crank-devnet.toml) and set `rpc_url` to your Helius (or paid) RPC endpoint. The public devnet endpoint will rate-limit the crank turner and prevent it from picking up tasks.

Then in three terminals:

```bash
# Terminal 1 — register the Tuktuk task queue + cron job
yarn cron

# Terminal 2 — run a local crank turner that picks up your tasks
yarn crank

# Terminal 3 — watch for the GPT response
yarn wait
```

`yarn cron` is idempotent: re-running it skips creation when resources exist, tops up the cron job balance if low, and adds the cron transaction.

### Environment variables for `yarn cron`

- `RPC_URL` — defaults to devnet (public)
- `ADMIN_SECRET_KEY` — base58 or JSON array (optional; falls back to `~/.config/solana/id.json`)
- `TASK_QUEUE_NAME` — defaults to `gpt-tuktuk-<walletPrefix>-v2` (the wallet-derived suffix avoids name collisions with other devnet users)
- `CRON_NAME` — defaults to `ask-gpt-<walletPrefix>-v2`
- `CRON_SCHEDULE` — 6-field cron expression (`sec min hour dom month dow`), defaults to `0 * * * * *` (every minute). Override e.g. `CRON_SCHEDULE="0 */5 * * * *"` for every 5 minutes.
- `FUNDING_LAMPORTS` — initial funding for the cron job (default `0.02 SOL` ≈ 400 cranks at 50000 lamports each)

## Inspect on-chain state

```bash
yarn check                        # prompt + latest_response from GptConfig
yarn debug                        # full Interaction account dump + recent signatures
node scripts/full-status.cjs      # comprehensive snapshot (program/cron/queue/response)
```

## Troubleshooting

### Crank turner crashes with `429 Too Many Requests`

The public devnet RPC rate-limits hard. The crank turner watches every Tuktuk queue, which generates many RPC calls per second. Use a dedicated RPC (Helius free tier works) in [`tuktuk/crank-devnet.toml`](tuktuk/crank-devnet.toml).

### Crank turner logs `task fee too high` on your task

The task queue's `min_crank_reward` was 0, but the turner refuses to lose money on the base transaction fee. This setup uses `min_crank_reward = 50000` (≈ 0.00005 SOL per crank) so turners will pick up tasks.

### `yarn cron` fails with `Account cronJobNameMapping not provided`

Closing Tuktuk cron jobs / task queues requires the name mapping account as an extra arg — the SDK's `closeCronJobV0` does not auto-resolve it. The [`reset`](scripts/reset.cjs) script removes cron transactions and queue authorities (recovers some rent) but cannot fully close the parent accounts; use a fresh name suffix (`-v3`, etc.) if you need a clean slate.

### Custom error `1` from Tuktuk

Tuktuk's `tuktuk_config.min_deposit` is currently 1 SOL — fresh task queues require that deposit. If your wallet has less, reuse an existing queue instead of creating a new one (this project's `-v2` already exists and is reused).

### `latest_response` stays empty

The MagicBlock off-chain LLM service has been processing batches very sparingly (callbacks observed around 2026-05-04, 2026-05-15, then a multi-month gap). All on-chain pieces in this project verifiably work; the response simply waits for MagicBlock to crank the next batch. The reference project shows the same empty state.

## Layout

```
.
├── Anchor.toml
├── Cargo.toml                              # workspace
├── README.md
├── package.json                            # merged: anchor client + Tuktuk SDK + cron deps
├── tsconfig.json
├── programs/solana-gpt-tuktuk/
│   ├── Cargo.toml
│   ├── idls/tuktuk.json                    # consumed by declare_program!
│   └── src/
│       ├── lib.rs
│       ├── state.rs                        # GptConfig
│       ├── tuktuk_types.rs                 # declare_program! + compile_transaction
│       └── instructions/{initialize,ask_gpt,receive_response,schedule_ask_gpt}.rs
├── scripts/
│   ├── cron.ts                             # register Tuktuk task queue + cron job
│   ├── check.mjs                           # prompt + latest_response snapshot
│   ├── debug.mjs                           # full Interaction account dump
│   ├── wait-callback.mjs                   # poll for GPT response
│   ├── full-status.cjs                     # program + cron + queue + response snapshot
│   ├── inspect-tuktuk.cjs                  # detailed Tuktuk queue/cron diagnostic
│   ├── reset.cjs                           # best-effort cleanup of old queues/crons
│   └── ...
├── tuktuk/
│   └── crank-devnet.toml                   # tuktuk-crank-turner config (set rpc_url here)
├── test.ts                                 # end-to-end devnet test
└── target/deploy/solana_gpt_tuktuk-keypair.json
```

## Verified devnet activity

| Step | Tx | Time |
|---|---|---|
| Program deploy | [`27kBei939pMb...`](https://explorer.solana.com/tx/27kBei939pMbuHwaiqnJuqGCgzmvBpkXY7BFhTQDQbPpLBAJz7SSY5cVu2UySXqzJFo4jSa74Ze4ZUBbRZ1i32v4?cluster=devnet) | 2026-05-24 16:53 |
| `initialize` GptConfig | [`3bFMVMBY4ek...`](https://explorer.solana.com/tx/3bFMVMBY4ekbGKpUwFiHzfP5rhcyXyymx5BmarotweAVzwHhM6V6N7qDc47HRNqCKFnoiubPrzggQ13ngTkQ4L3X?cluster=devnet) | 2026-05-24 16:57 |
| Manual `ask_gpt` (via `yarn test`) | [`twkh7V29YYy...`](https://explorer.solana.com/tx/twkh7V29YYyVq6EdsNpEQisxcf1gBaJs74ncUHm91TVzQgQKH6LMCPVvDMnnJE5UVFEqayyVrMV1YNFSKEcmFDb?cluster=devnet) | 2026-05-24 16:57 |
| Tuktuk-scheduled `ask_gpt` | [`2Vz8t99Dwoz...`](https://explorer.solana.com/tx/2Vz8t99Dwoz27gEAqVLSK1gvi1gs2hSnt3PQWx8EwCnD85cUBwwKhFQscqiA8z1MkmPfS41KsmQNhe26Q3VnrWJU?cluster=devnet) | 2026-05-24 19:21 |
| `receive_response` callback | Pending — waiting for MagicBlock off-chain batch | — |
