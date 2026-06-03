//! Instruction handlers.
//!
//! Each handler receives the raw account slice and the instruction payload
//! (with the leading discriminator byte already stripped by the dispatcher).

mod check;
mod contribute;
mod initialize;
mod refund;

pub use check::check_contributions;
pub use contribute::contribute;
pub use initialize::initialize;
pub use refund::refund;
