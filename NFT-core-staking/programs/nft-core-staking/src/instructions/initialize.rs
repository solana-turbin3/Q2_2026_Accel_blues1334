use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

use crate::constants::*;
use crate::state::{BaseCollectionV1Wrap, Config};

/// Create the per-collection staking `Config` and its rewards token mint.
///
/// The collection's update authority must already be the program's
/// `update_authority` PDA (set when the collection is created via
/// `create_collection`).
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// The Core collection these stake settings apply to.
    #[account(
        constraint = collection.update_authority == update_authority.key() @ crate::error::StakeError::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1Wrap>,

    /// CHECK: PDA that owns the collection (its update authority). Never read or
    /// written directly here; only used to validate the collection.
    #[account(
        seeds = [UPDATE_AUTHORITY_SEED, collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED, collection.key().as_ref()],
        bump,
    )]
    pub config: Account<'info, Config>,

    /// Rewards token mint; mint authority is the `config` PDA.
    #[account(
        init,
        payer = admin,
        seeds = [REWARDS_MINT_SEED, config.key().as_ref()],
        bump,
        mint::decimals = REWARDS_DECIMALS,
        mint::authority = config,
    )]
    pub rewards_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>, rewards_bps: u16, freeze_period: u16) -> Result<()> {
    ctx.accounts.config.set_inner(Config {
        rewards_bps,
        freeze_period,
        rewards_bump: ctx.bumps.rewards_mint,
        bump: ctx.bumps.config,
    });

    msg!(
        "Initialized staking config for collection {}: {} bps/day, {} day freeze",
        ctx.accounts.collection.key(),
        rewards_bps,
        freeze_period
    );
    Ok(())
}
