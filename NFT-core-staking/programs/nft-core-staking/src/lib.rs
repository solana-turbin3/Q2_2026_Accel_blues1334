pub mod constants;
pub mod error;
pub mod helpers;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("5ENTKY4nGmnwAvcEM3xnE8UeAaB4K3UFcgw995cfhwbS");

#[program]
pub mod nft_core_staking {
    use super::*;

    /// Create the per-collection staking config and rewards mint.
    pub fn initialize(
        ctx: Context<Initialize>,
        rewards_bps: u16,
        freeze_period: u16,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, rewards_bps, freeze_period)
    }

    /// Create a Core collection owned (update authority) by the program PDA,
    /// seeded with a `total_staked` statistics attribute.
    pub fn create_collection(
        ctx: Context<CreateCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        instructions::create_collection::handler(ctx, name, uri)
    }

    /// Mint a Core asset into the collection (helper for demos / tests).
    pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
        instructions::mint_asset::handler(ctx, name, uri)
    }

    /// Stake an asset (freeze + tag + increment collection counter).
    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        instructions::stake::handler(ctx)
    }

    /// Claim accrued rewards without unstaking (does not affect the freeze period).
    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        instructions::claim_rewards::handler(ctx)
    }

    /// Unstake an asset (enforce freeze period, pay rewards, thaw, decrement counter).
    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        instructions::unstake::handler(ctx)
    }
}
