//! Program constants — kept identical to the original Anchor implementation
//! so the economic behaviour of the fundraiser is preserved 1:1.

/// Minimum (base) target a maker must raise. The effective minimum is
/// `MIN_AMOUNT_TO_RAISE.pow(mint_decimals)`, matching the Anchor program.
pub const MIN_AMOUNT_TO_RAISE: u64 = 3;

/// Number of seconds in a day, used to convert the elapsed time into days.
pub const SECONDS_TO_DAYS: i64 = 86_400;

/// Maximum percentage of the target a single contributor may provide.
pub const MAX_CONTRIBUTION_PERCENTAGE: u64 = 10;

/// Scaler used to express [`MAX_CONTRIBUTION_PERCENTAGE`] as a percentage.
pub const PERCENTAGE_SCALER: u64 = 100;

/// PDA seed prefix for a `Fundraiser` account.
pub const FUNDRAISER_SEED: &[u8] = b"fundraiser";

/// PDA seed prefix for the `vault` token account.
pub const VAULT_SEED: &[u8] = b"vault";

/// PDA seed prefix for a `Contributor` account.
pub const CONTRIBUTOR_SEED: &[u8] = b"contributor";
