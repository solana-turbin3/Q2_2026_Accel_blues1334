use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{mint_to, Mint, MintTo, Token, TokenAccount};

use crate::constants::*;
use crate::error::StakeError;
use crate::helpers::{accrued_rewards, write_asset_attributes, AssetStakeState};
use crate::state::{BaseAssetV1Wrap, BaseCollectionV1Wrap, Config};

/// Claim accumulated rewards WITHOUT unstaking.
///
/// The asset stays frozen and staked. Only the reward checkpoint (`last_claim`)
/// advances — the `staked_at` timestamp that governs the freeze period is left
/// untouched, so claiming never extends or resets the freeze window. A user can
/// claim and then unstake immediately afterwards (provided the original freeze
/// period has elapsed).
#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = asset.owner == owner.key() @ StakeError::InvalidOwner,
        constraint = asset.update_authority == mpl_core::types::UpdateAuthority::Collection(collection.key()) @ StakeError::InvalidCollection,
    )]
    pub asset: Account<'info, BaseAssetV1Wrap>,

    // Must be writable: Core's UpdatePluginV1 marks the collection writable when
    // updating an asset plugin that belongs to a collection. We do not change the
    // collection's own statistics here.
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

    /// CHECK: PDA update authority of the collection; signs the attribute CPI.
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [REWARDS_MINT_SEED, config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: Account<'info, TokenAccount>,

    pub mpl_core_program: Program<'info, crate::state::MplCore>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimRewards>) -> Result<()> {
    let state = AssetStakeState::read(&ctx.accounts.asset.to_account_info())?;
    require!(state.staked, StakeError::AssetNotStaked);

    let now = Clock::get()?.unix_timestamp;
    let (amount, new_last_claim) = accrued_rewards(now, state.last_claim, &ctx.accounts.config)?;

    if amount == 0 {
        msg!("Nothing to claim yet (less than a full staked day accrued)");
        return Ok(());
    }

    let collection_key = ctx.accounts.collection.key();
    let ua_signer_seeds: &[&[&[u8]]] = &[&[
        UPDATE_AUTHORITY_SEED,
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ]];

    // Advance only the reward checkpoint. staked_at is preserved so the freeze
    // period is unaffected.
    let attributes = state.to_attribute_list(true, state.staked_at, new_last_claim);
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

    // Mint rewards, signed by the config PDA (the mint authority).
    let config_signer_seeds: &[&[&[u8]]] = &[&[
        CONFIG_SEED,
        collection_key.as_ref(),
        &[ctx.accounts.config.bump],
    ]];

    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.rewards_mint.to_account_info(),
                to: ctx.accounts.user_rewards_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            config_signer_seeds,
        ),
        amount,
    )?;

    msg!("Claimed {} reward base units for asset {}", amount, ctx.accounts.asset.key());
    Ok(())
}
