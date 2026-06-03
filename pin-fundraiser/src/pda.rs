//! PDA verification helpers.
//!
//! ## Why we pass bumps in the instruction data
//!
//! Re-deriving a PDA with `find_program_address` on-chain is expensive: it
//! loops calling the (curve) syscall until it finds an off-curve address — up
//! to 255 iterations. Instead, the client passes the already-known canonical
//! `bump`, and we validate the address with a single `create_program_address`
//! call. This mirrors the `bump = ...` constraint Anchor uses internally.
//!
//! For accounts we *create* via a signed CPI (the fundraiser, the vault and the
//! contributor record), the runtime already enforces that the account address
//! equals the address derived from the signer seeds — so no explicit check is
//! needed on the creation path. We only verify explicitly when an account is
//! passed in pre-existing (e.g. the contributor record on `refund`).

use pinocchio::{account::AccountView, address::Address, error::ProgramError};

use crate::error::FundraiserError;

/// Assert that `account` is exactly the PDA derived from `seeds + [bump]` under
/// `program_id` (the executing program's id, from the entrypoint).
#[inline(always)]
pub fn assert_pda(
    program_id: &Address,
    account: &AccountView,
    seeds: &[&[u8]],
    bump: u8,
) -> Result<(), ProgramError> {
    // Build `seeds || [bump]` on the stack (max 4 seeds in this program).
    let bump_seed = [bump];
    let mut full: [&[u8]; 8] = [&[]; 8];
    let n = seeds.len();
    full[..n].copy_from_slice(seeds);
    full[n] = &bump_seed;

    let derived = Address::create_program_address(&full[..n + 1], program_id)
        .map_err(|_| ProgramError::from(FundraiserError::InvalidPda))?;

    if &derived != account.address() {
        return Err(FundraiserError::InvalidPda.into());
    }
    Ok(())
}
