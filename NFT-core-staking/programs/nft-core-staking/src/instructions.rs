pub mod claim_rewards;
pub mod create_collection;
pub mod initialize;
pub mod mint_asset;
pub mod stake;
pub mod unstake;

// Each module exposes a `handler` fn and its Accounts context struct. The
// duplicate `handler` names in these globs are harmless: the program entrypoints
// in `lib.rs` always call them by full path (e.g. `instructions::stake::handler`).
#[allow(ambiguous_glob_reexports)]
pub use claim_rewards::*;
#[allow(ambiguous_glob_reexports)]
pub use create_collection::*;
#[allow(ambiguous_glob_reexports)]
pub use initialize::*;
#[allow(ambiguous_glob_reexports)]
pub use mint_asset::*;
#[allow(ambiguous_glob_reexports)]
pub use stake::*;
#[allow(ambiguous_glob_reexports)]
pub use unstake::*;
