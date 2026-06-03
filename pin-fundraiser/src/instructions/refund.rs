//! `refund` — return a contributor's tokens when the campaign expired without
//! reaching its target, and close the contributor record.
//!
//! Accounts:
//!   0. `[signer, writable]` contributor
//!   1. `[]`                 maker               — needed for the fundraiser seeds
//!   2. `[writable]`         fundraiser
//!   3. `[writable]`         contributor_account — PDA, closed back to the contributor
//!   4. `[writable]`         contributor_ata     — destination token account
//!   5. `[writable]`         vault               — fundraiser-owned source
//!   6. `[]`                 token_program
//!
//! Instruction data (1 byte, after the discriminator): `contributor_bump: u8`.
//!
//! ## Parity note
//!
//! The duration check is reproduced verbatim from Anchor (`duration >=
//! elapsed_days`). See `README.md` → "Behavioural parity".

use pinocchio::{
    account::AccountView,
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_token::{instructions::Transfer, state::Account as TokenAccount};

use crate::{
    constants::{CONTRIBUTOR_SEED, FUNDRAISER_SEED, SECONDS_TO_DAYS},
    error::FundraiserError,
    pda::assert_pda,
    state::{Contributor, Fundraiser},
};

pub fn refund(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [contributor, maker, fundraiser, contributor_account, contributor_ata, vault, _token_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if data.len() != 1 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let contributor_bump = data[0];

    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // --- read fundraiser state --------------------------------------------
    let (amount_to_raise, duration, time_started, fr_vault, bump) = {
        let fr = Fundraiser::load(fundraiser)?;
        if fr.maker != *maker.address().as_array() {
            return Err(ProgramError::IllegalOwner);
        }
        (
            fr.amount_to_raise(),
            fr.duration,
            fr.time_started(),
            fr.vault,
            fr.bump,
        )
    };

    // Duration check — preserved verbatim from the Anchor source.
    let now = Clock::get()?.unix_timestamp;
    let elapsed_days = ((now - time_started) / SECONDS_TO_DAYS) as u8;
    if duration < elapsed_days {
        return Err(FundraiserError::FundraiserNotEnded.into());
    }

    // --- verify the vault and that the target was NOT met -----------------
    if vault.address().as_array() != &fr_vault {
        return Err(FundraiserError::InvalidVault.into());
    }
    let vault_amount = TokenAccount::from_account_view(vault)?.amount();
    if vault_amount >= amount_to_raise {
        return Err(FundraiserError::TargetMet.into());
    }

    // --- verify the contributor record and read its balance ---------------
    assert_pda(
        program_id,
        contributor_account,
        &[
            CONTRIBUTOR_SEED,
            fundraiser.address().as_array(),
            contributor.address().as_array(),
        ],
        contributor_bump,
    )?;
    let refund_amount = Contributor::load_mut(contributor_account)?.amount();

    // --- return the funds, signed by the fundraiser PDA -------------------
    let bump_seed = [bump];
    let signer_seeds = [
        Seed::from(FUNDRAISER_SEED),
        Seed::from(maker.address().as_array()),
        Seed::from(&bump_seed),
    ];
    Transfer::new(vault, contributor_ata, fundraiser, refund_amount)
        .invoke_signed(&[Signer::from(&signer_seeds)])?;

    // --- update the campaign total ----------------------------------------
    {
        let mut fr = Fundraiser::load_mut(fundraiser)?;
        let new_total = fr
            .current_amount()
            .checked_sub(refund_amount)
            .ok_or(FundraiserError::TargetMet)?;
        fr.set_current_amount(new_total);
    }

    // --- close the contributor record, returning rent to the contributor --
    let rent = contributor_account.lamports();
    contributor.set_lamports(contributor.lamports() + rent);
    contributor_account.close()?;

    Ok(())
}
