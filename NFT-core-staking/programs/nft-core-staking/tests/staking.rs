//! End-to-end LiteSVM tests for the Metaplex Core NFT staking program.
//!
//! These tests load the real `mpl-core` program (dumped to
//! `tests/fixtures/mpl_core.so`) plus our compiled program and exercise the full
//! flow: create collection -> initialize -> mint -> stake -> claim_rewards ->
//! unstake, including clock warping to validate the freeze period and rewards.
//!
//! The headline assertion (`claim_then_unstake_same_block`) proves that claiming
//! rewards does NOT reset the freeze clock: a user can claim and unstake in the
//! same block once the original freeze period has elapsed.

use std::str::FromStr;

use litesvm::LiteSVM;
use mpl_core::Collection;
use solana_account::Account;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

const PROGRAM_ID: &str = "5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS";
const MPL_CORE_ID: &str = "CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d";
const TOKEN_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_ID: &str = "11111111111111111111111111111111";

// Anchor instruction discriminators: sha256("global:<name>")[..8].
const IX_INITIALIZE: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];
const IX_CREATE_COLLECTION: [u8; 8] = [156, 251, 92, 54, 233, 2, 16, 82];
const IX_MINT_ASSET: [u8; 8] = [84, 175, 211, 156, 56, 250, 104, 118];
const IX_STAKE: [u8; 8] = [206, 176, 202, 18, 200, 209, 179, 108];
const IX_CLAIM_REWARDS: [u8; 8] = [4, 144, 132, 71, 116, 23, 151, 80];
const IX_UNSTAKE: [u8; 8] = [90, 95, 107, 42, 205, 124, 50, 225];

const SECONDS_PER_DAY: i64 = 86_400;
const REWARDS_BPS: u16 = 500; // 0.05 reward token per staked day (6 decimals)
const FREEZE_PERIOD_DAYS: u16 = 1;

fn pk(s: &str) -> Pubkey {
    Pubkey::from_str(s).unwrap()
}

/// Borsh-encode a string: 4-byte little-endian length prefix + UTF-8 bytes.
fn push_string(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

struct Ctx {
    program_id: Pubkey,
    update_authority: Pubkey,
    config: Pubkey,
    rewards_mint: Pubkey,
    payer_ata: Pubkey,
}

fn derive(program_id: Pubkey, collection: Pubkey, payer: Pubkey) -> Ctx {
    let (update_authority, _) =
        Pubkey::find_program_address(&[b"update_authority", collection.as_ref()], &program_id);
    let (config, _) =
        Pubkey::find_program_address(&[b"config", collection.as_ref()], &program_id);
    let (rewards_mint, _) =
        Pubkey::find_program_address(&[b"rewards_mint", config.as_ref()], &program_id);
    let (payer_ata, _) = Pubkey::find_program_address(
        &[payer.as_ref(), pk(TOKEN_ID).as_ref(), rewards_mint.as_ref()],
        &pk(ATA_ID),
    );
    Ctx {
        program_id,
        update_authority,
        config,
        rewards_mint,
        payer_ata,
    }
}

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(pk(MPL_CORE_ID), "tests/fixtures/mpl_core.so")
        .unwrap();
    svm.add_program_from_file(pk(PROGRAM_ID), "../../target/deploy/nft_core_staking.so")
        .unwrap();
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 100_000_000_000).unwrap();
    (svm, payer)
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ix: Instruction,
    extra_signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra_signers);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
}

fn warp_days(svm: &mut LiteSVM, days: i64, extra_secs: i64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.unix_timestamp += days * SECONDS_PER_DAY + extra_secs;
    clock.slot += 1;
    svm.set_sysvar(&clock);
}

fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    match svm.get_account(ata) {
        // SPL token account amount is a u64 at byte offset 64.
        Some(acc) if acc.data.len() >= 72 => {
            u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
        }
        _ => 0,
    }
}

fn collection_total_staked(svm: &LiteSVM, collection: &Pubkey) -> i64 {
    let acc: Account = svm.get_account(collection).unwrap();
    let parsed = Collection::from_bytes(&acc.data).unwrap();
    let attrs = parsed
        .plugin_list
        .attributes
        .expect("collection should carry an Attributes plugin");
    attrs
        .attributes
        .attribute_list
        .iter()
        .find(|a| a.key == "total_staked")
        .map(|a| a.value.parse::<i64>().unwrap())
        .expect("total_staked attribute should exist")
}

/// Create a collection, initialize config, mint an asset to `payer`, and stake it.
/// Returns (ctx, collection_pubkey, asset_pubkey).
fn bootstrap_and_stake(svm: &mut LiteSVM, payer: &Keypair) -> (Ctx, Pubkey, Pubkey) {
    let program_id = pk(PROGRAM_ID);
    let collection = Keypair::new();
    let asset = Keypair::new();
    let ctx = derive(program_id, collection.pubkey(), payer.pubkey());

    // create_collection(name, uri)
    let mut data = IX_CREATE_COLLECTION.to_vec();
    push_string(&mut data, "Core Stakers");
    push_string(&mut data, "https://example.com/collection.json");
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(collection.pubkey(), true),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new_readonly(pk(MPL_CORE_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data,
    };
    send(svm, payer, ix, &[&collection]).expect("create_collection");

    // initialize(rewards_bps, freeze_period)
    let mut data = IX_INITIALIZE.to_vec();
    data.extend_from_slice(&REWARDS_BPS.to_le_bytes());
    data.extend_from_slice(&FREEZE_PERIOD_DAYS.to_le_bytes());
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(collection.pubkey(), false),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new(ctx.config, false),
            AccountMeta::new(ctx.rewards_mint, false),
            AccountMeta::new_readonly(pk(TOKEN_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data,
    };
    send(svm, payer, ix, &[]).expect("initialize");

    // mint_asset(name, uri)
    let mut data = IX_MINT_ASSET.to_vec();
    push_string(&mut data, "Staker #1");
    push_string(&mut data, "https://example.com/asset.json");
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(asset.pubkey(), true),
            AccountMeta::new(collection.pubkey(), false),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new_readonly(pk(MPL_CORE_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data,
    };
    send(svm, payer, ix, &[&asset]).expect("mint_asset");

    // stake
    let ix = stake_ix(&ctx, collection.pubkey(), asset.pubkey(), payer.pubkey());
    send(svm, payer, ix, &[]).expect("stake");

    (ctx, collection.pubkey(), asset.pubkey())
}

fn stake_ix(ctx: &Ctx, collection: Pubkey, asset: Pubkey, owner: Pubkey) -> Instruction {
    Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(asset, false),
            AccountMeta::new(collection, false),
            AccountMeta::new_readonly(ctx.config, false),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new_readonly(pk(MPL_CORE_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data: IX_STAKE.to_vec(),
    }
}

fn claim_ix(ctx: &Ctx, collection: Pubkey, asset: Pubkey, owner: Pubkey) -> Instruction {
    Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(asset, false),
            AccountMeta::new(collection, false),
            AccountMeta::new_readonly(ctx.config, false),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new(ctx.rewards_mint, false),
            AccountMeta::new(ctx.payer_ata, false),
            AccountMeta::new_readonly(pk(MPL_CORE_ID), false),
            AccountMeta::new_readonly(pk(TOKEN_ID), false),
            AccountMeta::new_readonly(pk(ATA_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data: IX_CLAIM_REWARDS.to_vec(),
    }
}

fn unstake_ix(ctx: &Ctx, collection: Pubkey, asset: Pubkey, owner: Pubkey) -> Instruction {
    Instruction {
        program_id: ctx.program_id,
        accounts: vec![
            AccountMeta::new(owner, true),
            AccountMeta::new(asset, false),
            AccountMeta::new(collection, false),
            AccountMeta::new_readonly(ctx.config, false),
            AccountMeta::new_readonly(ctx.update_authority, false),
            AccountMeta::new(ctx.rewards_mint, false),
            AccountMeta::new(ctx.payer_ata, false),
            AccountMeta::new_readonly(pk(MPL_CORE_ID), false),
            AccountMeta::new_readonly(pk(TOKEN_ID), false),
            AccountMeta::new_readonly(pk(ATA_ID), false),
            AccountMeta::new_readonly(pk(SYSTEM_ID), false),
        ],
        data: IX_UNSTAKE.to_vec(),
    }
}

#[test]
fn stake_sets_collection_counter_and_freezes() {
    let (mut svm, payer) = setup();
    let (_ctx, collection, _asset) = bootstrap_and_stake(&mut svm, &payer);
    // Task 2: the collection-level total_staked counter is now 1.
    assert_eq!(collection_total_staked(&svm, &collection), 1);
}

#[test]
fn unstake_before_freeze_period_fails() {
    let (mut svm, payer) = setup();
    let (ctx, collection, asset) = bootstrap_and_stake(&mut svm, &payer);
    // No time has passed: freeze period (1 day) has not elapsed.
    let res = send(
        &mut svm,
        &payer,
        unstake_ix(&ctx, collection, asset, payer.pubkey()),
        &[],
    );
    assert!(res.is_err(), "unstake should fail before the freeze period elapses");
}

#[test]
fn claim_rewards_mints_without_unstaking() {
    let (mut svm, payer) = setup();
    let (ctx, collection, asset) = bootstrap_and_stake(&mut svm, &payer);

    warp_days(&mut svm, 2, 0); // 2 full staked days

    assert_eq!(token_balance(&svm, &ctx.payer_ata), 0);
    send(
        &mut svm,
        &payer,
        claim_ix(&ctx, collection, asset, payer.pubkey()),
        &[],
    )
    .expect("claim_rewards");

    // 2 days * 500 bps * 1e6 / 1e4 = 100_000 base units (0.1 token).
    assert_eq!(token_balance(&svm, &ctx.payer_ata), 100_000);
    // Still staked: counter unchanged.
    assert_eq!(collection_total_staked(&svm, &collection), 1);
}

/// The key task-1 requirement: claiming rewards must not extend or reset the
/// freeze period. After exactly one freeze period, a user can claim and then
/// unstake in the very same block.
#[test]
fn claim_then_unstake_same_block() {
    let (mut svm, payer) = setup();
    let (ctx, collection, asset) = bootstrap_and_stake(&mut svm, &payer);

    // Advance just past the 1-day freeze period.
    warp_days(&mut svm, 1, 60);

    // Claim rewards (advances the reward checkpoint, not the freeze clock).
    send(
        &mut svm,
        &payer,
        claim_ix(&ctx, collection, asset, payer.pubkey()),
        &[],
    )
    .expect("claim_rewards");
    let balance_after_claim = token_balance(&svm, &ctx.payer_ata);
    assert_eq!(balance_after_claim, 50_000); // 1 day * 500 bps

    // Unstake immediately. This only succeeds because staked_at was untouched by
    // the claim — i.e. the freeze period is measured from the original stake.
    send(
        &mut svm,
        &payer,
        unstake_ix(&ctx, collection, asset, payer.pubkey()),
        &[],
    )
    .expect("unstake should succeed right after claiming");

    // No extra full day accrued between claim and unstake -> balance unchanged.
    assert_eq!(token_balance(&svm, &ctx.payer_ata), balance_after_claim);
    // Counter back to zero.
    assert_eq!(collection_total_staked(&svm, &collection), 0);
}
