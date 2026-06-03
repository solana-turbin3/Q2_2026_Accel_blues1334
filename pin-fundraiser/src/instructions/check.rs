//! `check_contributions` — once the target is met, release the vault to the
//! maker and close both the vault and the fundraiser.
//!
//! Accounts:
//!   0. `[signer, writable]` maker        — campaign owner, receives the funds + rent
//!   1. `[writable]`         fundraiser   — closed, rent returned to maker
//!   2. `[writable]`         vault        — fundraiser-owned token account, drained + closed
//!   3. `[writable]`         maker_ata    — destination token account (must already exist)
//!   4. `[]`                 token_program
//!
//! Instruction data: empty (the bump is read from the stored state).
//!
//! ## Difference from the Anchor original
//!
//! The Anchor version left the vault ATA open. Because our vault is a dedicated
//! PDA token account, we also `CloseAccount` it here and return its rent to the
//! maker — otherwise those lamports would be stranded once the fundraiser is
//! gone.

use pinocchio::{
    account::AccountView,
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    ProgramResult,
};
use pinocchio_token::{
    instructions::{CloseAccount, Transfer},
    state::Account as TokenAccount,
};

use crate::{constants::FUNDRAISER_SEED, error::FundraiserError, state::Fundraiser};

pub fn check_contributions(
    _program_id: &Address,
    accounts: &mut [AccountView],
    _data: &[u8],
) -> ProgramResult {
    let [maker, fundraiser, vault, maker_ata, _token_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // --- read fundraiser state --------------------------------------------
    let (amount_to_raise, fr_vault, bump) = {
        let fr = Fundraiser::load(fundraiser)?;
        // Only the maker recorded in the campaign may release the funds.
        if fr.maker != *maker.address().as_array() {
            return Err(ProgramError::IllegalOwner);
        }
        (fr.amount_to_raise(), fr.vault, fr.bump)
    };

    // --- verify the vault and that the target has been met ----------------
    if vault.address().as_array() != &fr_vault {
        return Err(FundraiserError::InvalidVault.into());
    }
    let vault_amount = TokenAccount::from_account_view(vault)?.amount();
    if vault_amount < amount_to_raise {
        return Err(FundraiserError::TargetNotMet.into());
    }

    // --- release the funds and close the vault, signed by the PDA ---------
    let bump_seed = [bump];
    let signer_seeds = [
        Seed::from(FUNDRAISER_SEED),
        Seed::from(maker.address().as_array()),
        Seed::from(&bump_seed),
    ];
    let signer = Signer::from(&signer_seeds);

    Transfer::new(vault, maker_ata, fundraiser, vault_amount).invoke_signed(&[signer.clone()])?;
    CloseAccount::new(vault, maker, fundraiser).invoke_signed(&[signer])?;

    // --- close the fundraiser account, returning its rent to the maker ----
    let rent = fundraiser.lamports();
    maker.set_lamports(maker.lamports() + rent);
    fundraiser.close()?;

    Ok(())
}
