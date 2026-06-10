use anchor_lang::prelude::*;

/// Per-collection staking configuration. Also the mint authority of the
/// collection's rewards token.
///
/// PDA seeds: `[b"config", collection]`.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// Reward rate, in basis points, expressed as "reward tokens per staked day".
    /// Tokens minted per whole staked day = `rewards_bps / 10_000` (scaled by mint decimals).
    pub rewards_bps: u16,
    /// Minimum staking duration, in days, before an asset may be unstaked.
    pub freeze_period: u16,
    /// Bump for the rewards mint PDA.
    pub rewards_bump: u8,
    /// Bump for this config PDA.
    pub bump: u8,
}
