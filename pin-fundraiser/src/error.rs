//! Program-specific errors.
//!
//! The discriminant order is kept identical to the Anchor `FundraiserError`
//! enum so the custom error codes returned on-chain match the original program
//! (Anchor offsets custom errors by 6000; here the raw code maps 1:1 to the
//! variant index, which is the Pinocchio convention).

use pinocchio::error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FundraiserError {
    /// The amount to raise has not been met.
    TargetNotMet,
    /// The amount to raise has been achieved.
    TargetMet,
    /// The contribution is too big.
    ContributionTooBig,
    /// The contribution is too small.
    ContributionTooSmall,
    /// The maximum amount to contribute has been reached.
    MaximumContributionsReached,
    /// The fundraiser has not ended yet.
    FundraiserNotEnded,
    /// The fundraiser has ended.
    FundraiserEnded,
    /// Invalid total amount — it should be bigger than the minimum.
    InvalidAmount,
    /// An account did not match the expected PDA derivation.
    InvalidPda,
    /// A token account had an unexpected owner or mint.
    InvalidVault,
}

impl From<FundraiserError> for ProgramError {
    #[inline(always)]
    fn from(e: FundraiserError) -> Self {
        // Offset by 1 so the first variant maps to a non-zero custom code
        // (`Custom(0)` is special-cased by the runtime as a generic error).
        ProgramError::Custom(e as u32 + 1)
    }
}
