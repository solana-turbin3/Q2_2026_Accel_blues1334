use anchor_lang::prelude::*;
use mpl_core::accounts::BaseAssetV1;
use mpl_core::fetch_plugin;
use mpl_core::instructions::{AddPluginV1CpiBuilder, UpdatePluginV1CpiBuilder};
use mpl_core::types::{FreezeDelegate, Plugin, PluginAuthority, PluginType, UpdateAuthority};

use crate::constants::*;
use crate::error::StakeError;
use crate::helpers::{bump_collection_total_staked, write_asset_attributes, AssetStakeState};
use crate::state::{BaseAssetV1Wrap, BaseCollectionV1Wrap, Config};

/// Stake an asset: freeze it, tag it with stake/claim timestamps, and bump the
/// collection's `total_staked` counter.
#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = asset.owner == owner.key() @ StakeError::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ StakeError::InvalidCollection,
    )]
    pub asset: Account<'info, BaseAssetV1Wrap>,

    #[account(
        mut,
        constraint = collection.update_authority == update_authority.key() @ StakeError::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1Wrap>,

    #[account(
        seeds = [CONFIG_SEED, collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    /// CHECK: PDA update authority of the collection; signs plugin CPIs.
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    pub mpl_core_program: Program<'info, crate::state::MplCore>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Stake>) -> Result<()> {
    let state = AssetStakeState::read(&ctx.accounts.asset.to_account_info())?;
    require!(!state.staked, StakeError::AlreadyStaked);

    let now = Clock::get()?.unix_timestamp;

    let collection_key = ctx.accounts.collection.key();
    let ua_signer_seeds: &[&[&[u8]]] = &[&[
        UPDATE_AUTHORITY_SEED,
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ]];

    // 1. Record stake state on the asset. staked_at and last_claim both start now.
    let attributes = state.to_attribute_list(true, now, now);
    write_asset_attributes(
        attributes,
        state.has_attributes_plugin,
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.asset.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.update_authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ua_signer_seeds,
    )?;

    // 2. Freeze the asset. Add the FreezeDelegate plugin the first time (signed
    //    by the owner, since it is an owner-managed plugin), or just re-freeze an
    //    existing one (signed by the update authority PDA).
    let has_freeze = fetch_plugin::<BaseAssetV1, FreezeDelegate>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::FreezeDelegate,
    )
    .is_ok();

    if has_freeze {
        UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
            .asset(&ctx.accounts.asset.to_account_info())
            .collection(Some(&ctx.accounts.collection.to_account_info()))
            .payer(&ctx.accounts.owner.to_account_info())
            .authority(Some(&ctx.accounts.update_authority.to_account_info()))
            .system_program(&ctx.accounts.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
            .invoke_signed(ua_signer_seeds)?;
    } else {
        AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
            .asset(&ctx.accounts.asset.to_account_info())
            .collection(Some(&ctx.accounts.collection.to_account_info()))
            .payer(&ctx.accounts.owner.to_account_info())
            .authority(Some(&ctx.accounts.owner.to_account_info()))
            .system_program(&ctx.accounts.system_program.to_account_info())
            .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
            // Hand the plugin's authority to the PDA so the program can thaw later.
            .init_authority(PluginAuthority::UpdateAuthority)
            .invoke()?;
    }

    // 3. Bump the collection-level staked counter.
    bump_collection_total_staked(
        1,
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.update_authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ua_signer_seeds,
    )?;

    msg!("Asset {} staked at {}", ctx.accounts.asset.key(), now);
    Ok(())
}
