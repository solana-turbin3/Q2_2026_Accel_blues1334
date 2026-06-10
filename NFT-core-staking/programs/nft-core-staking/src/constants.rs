use anchor_lang::prelude::*;

/// PDA seed prefixes.
#[constant]
pub const CONFIG_SEED: &[u8] = b"config";
#[constant]
pub const UPDATE_AUTHORITY_SEED: &[u8] = b"update_authority";
#[constant]
pub const REWARDS_MINT_SEED: &[u8] = b"rewards_mint";

/// Decimals of the rewards token mint.
pub const REWARDS_DECIMALS: u8 = 6;

/// Seconds in one day, used to convert elapsed time into whole staked days.
pub const SECONDS_PER_DAY: i64 = 86_400;

// --- On-asset attribute keys (state stored on the Core asset itself) ---

/// "true" / "false": whether the asset is currently staked.
pub const ATTR_STAKED: &str = "staked";
/// Unix timestamp (i64 as string) when the asset was staked. Drives the freeze
/// period only — it is NEVER moved by `claim_rewards`.
pub const ATTR_STAKED_AT: &str = "staked_at";
/// Unix timestamp (i64 as string) of the last reward checkpoint. Drives reward
/// accrual and advances on every `claim_rewards` (and the final `unstake` mint).
pub const ATTR_LAST_CLAIM: &str = "last_claim";

// --- On-collection attribute keys (collection-level statistics) ---

/// Number of assets in the collection that are currently staked.
pub const ATTR_TOTAL_STAKED: &str = "total_staked";
