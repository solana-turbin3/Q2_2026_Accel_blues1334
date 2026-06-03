//! `contribute` — donate tokens into a campaign's vault.
//!
//! Accounts:
//!   0. `[signer, writable]` contributor
//!   1. `[writable]`         fundraiser
//!   2. `[writable]`         contributor_account — PDA `["contributor", fundraiser, contributor]`
//!   3. `[writable]`         contributor_ata     — source token account
//!   4. `[writable]`         vault               — destination (fundraiser-owned)
//!   5. `[]`                 token_program
//!   6. `[]`                 system_program
//!
//! Instruction data (9 bytes, after the discriminator):
//!   `amount: u64 | contributor_bump: u8`
//!
//! ## Parity note
//!
//! The duration check below is reproduced *exactly* as in the original Anchor
//! program (`duration <= elapsed_days`). See `README.md` → "Behavioural parity".

use pinocchio::{
    account::AccountView,
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::Transfer;

use crate::{
    constants::{
        CONTRIBUTOR_SEED, MAX_CONTRIBUTION_PERCENTAGE, PERCENTAGE_SCALER, SECONDS_TO_DAYS,
    },
    error::FundraiserError,
    pda::assert_pda,
    state::{Contributor, Fundraiser},
};

pub fn contribute(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [contributor, fundraiser, contributor_account, contributor_ata, vault, _token_program, _system_program, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // --- parse instruction data -------------------------------------------
    if data.len() != 9 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let contributor_bump = data[8];

    if !contributor.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // --- read fundraiser state (released before any CPI) ------------------
    let (amount_to_raise, duration, time_started, fr_vault) = {
        let fr = Fundraiser::load(fundraiser)?;
        (
            fr.amount_to_raise(),
            fr.duration,
            fr.time_started(),
            fr.vault,
        )
    };

    // The vault must be the exact token account pinned at initialization.
    if vault.address().as_array() != &fr_vault {
        return Err(FundraiserError::InvalidVault.into());
    }

    let max_contribution = amount_to_raise
        .checked_mul(MAX_CONTRIBUTION_PERCENTAGE)
        .ok_or(FundraiserError::ContributionTooBig)?
        / PERCENTAGE_SCALER;

    // Minimum is `1.pow(decimals) == 1`, so the check collapses to `amount > 1`.
    if amount <= 1 {
        return Err(FundraiserError::ContributionTooSmall.into());
    }
    if amount > max_contribution {
        return Err(FundraiserError::ContributionTooBig.into());
    }

    // Duration check — preserved verbatim from the Anchor source.
    let now = Clock::get()?.unix_timestamp;
    let elapsed_days = ((now - time_started) / SECONDS_TO_DAYS) as u8;
    if duration > elapsed_days {
        return Err(FundraiserError::FundraiserEnded.into());
    }

    // --- ensure the contributor record exists, fetch its running total ----
    let needs_init = contributor_account.data_len() == 0;
    let current_contribution = if needs_init {
        0
    } else {
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
        Contributor::load_mut(contributor_account)?.amount()
    };

    if current_contribution > max_contribution
        || current_contribution
            .checked_add(amount)
            .ok_or(FundraiserError::MaximumContributionsReached)?
            > max_contribution
    {
        return Err(FundraiserError::MaximumContributionsReached.into());
    }

    if needs_init {
        let c_bump = [contributor_bump];
        let c_seeds = [
            Seed::from(CONTRIBUTOR_SEED),
            Seed::from(fundraiser.address().as_array()),
            Seed::from(contributor.address().as_array()),
            Seed::from(&c_bump),
        ];
        CreateAccount::with_minimum_balance(
            contributor,
            contributor_account,
            Contributor::LEN as u64,
            program_id,
            None,
        )?
        .invoke_signed(&[Signer::from(&c_seeds)])?;
    }

    // --- move the tokens contributor_ata -> vault -------------------------
    Transfer::new(contributor_ata, vault, contributor, amount).invoke()?;

    // --- update running totals --------------------------------------------
    {
        let mut fr = Fundraiser::load_mut(fundraiser)?;
        let new_total = fr
            .current_amount()
            .checked_add(amount)
            .ok_or(FundraiserError::ContributionTooBig)?;
        fr.set_current_amount(new_total);
    }
    {
        let mut c = Contributor::load_mut(contributor_account)?;
        c.set_amount(current_contribution + amount);
    }

    Ok(())
}
