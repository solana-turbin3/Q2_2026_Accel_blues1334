use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{mint_to, Mint, MintTo, Token, TokenAccount};
use mpl_core::instructions::UpdatePluginV1CpiBuilder;
use mpl_core::types::{FreezeDelegate, Plugin, UpdateAuthority};

use crate::constants::*;
use crate::error::StakeError;
use crate::helpers::{
    accrued_rewards, bump_collection_total_staked, write_asset_attributes, AssetStakeState,
};
use crate::state::{BaseAssetV1Wrap, BaseCollectionV1Wrap, Config};

/// Unstake an asset: enforce the freeze period, pay out any remaining rewards,
/// thaw the asset, and decrement the collection's `total_staked` counter.
#[derive(Accounts)]
pub struct Unstake<'info> {
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

pub fn handler(ctx: Context<Unstake>) -> Result<()> {
    let state = AssetStakeState::read(&ctx.accounts.asset.to_account_info())?;
    require!(state.staked, StakeError::AssetNotStaked);

    let now = Clock::get()?.unix_timestamp;

    // Freeze period is measured from staked_at, which claim_rewards never moves.
    let staked_days = now
        .checked_sub(state.staked_at)
        .ok_or(StakeError::InvalidTimestamp)?
        / SECONDS_PER_DAY;
    require!(
        staked_days >= ctx.accounts.config.freeze_period as i64,
        StakeError::FreezePeriodNotElapsed
    );

    // Final reward payout for time accrued since the last claim.
    let (amount, _) = accrued_rewards(now, state.last_claim, &ctx.accounts.config)?;

    let collection_key = ctx.accounts.collection.key();
    let ua_signer_seeds: &[&[&[u8]]] = &[&[
        UPDATE_AUTHORITY_SEED,
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ]];

    // 1. Reset stake state on the asset.
    let attributes = state.to_attribute_list(false, 0, 0);
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

    // 2. Thaw the asset (FreezeDelegate -> frozen: false), signed by the PDA.
    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
        .invoke_signed(ua_signer_seeds)?;

    // 3. Mint any remaining rewards, signed by the config PDA.
    if amount > 0 {
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
    }

    // 4. Decrement the collection-level staked counter.
    bump_collection_total_staked(
        -1,
        &ctx.accounts.mpl_core_program.to_account_info(),
        &ctx.accounts.collection.to_account_info(),
        &ctx.accounts.owner.to_account_info(),
        &ctx.accounts.update_authority.to_account_info(),
        &ctx.accounts.system_program.to_account_info(),
        ua_signer_seeds,
    )?;

    msg!(
        "Asset {} unstaked after {} days, paid {} reward base units",
        ctx.accounts.asset.key(),
        staked_days,
        amount
    );
    Ok(())
}
