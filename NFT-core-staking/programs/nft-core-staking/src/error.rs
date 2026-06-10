use anchor_lang::prelude::*;

#[error_code]
pub enum StakeError {
    #[msg("The signer is not the owner of the asset")]
    InvalidOwner,
    #[msg("The asset does not belong to the provided collection")]
    InvalidCollection,
    #[msg("The collection update authority does not match the expected PDA")]
    InvalidUpdateAuthority,
    #[msg("The asset is already staked")]
    AlreadyStaked,
    #[msg("The asset is not currently staked")]
    AssetNotStaked,
    #[msg("A stored timestamp attribute could not be parsed")]
    InvalidTimestamp,
    #[msg("The minimum freeze period has not elapsed yet")]
    FreezePeriodNotElapsed,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("The collection statistics attribute is missing or malformed")]
    InvalidCollectionStats,
}
