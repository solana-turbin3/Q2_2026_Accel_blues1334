//! `initialize` — open a new fundraising campaign.
//!
//! Accounts:
//!   0. `[signer, writable]` maker            — pays for the new accounts
//!   1. `[]`                 mint_to_raise     — SPL mint being collected
//!   2. `[writable]`         fundraiser        — PDA `["fundraiser", maker]`
//!   3. `[writable]`         vault             — PDA `["vault", fundraiser]` token account
//!   4. `[]`                 token_program
//!   5. `[]`                 system_program
//!
//! Instruction data (11 bytes, after the discriminator):
//!   `amount: u64 | duration: u8 | fundraiser_bump: u8 | vault_bump: u8`

use pinocchio::{
    account::AccountView,
    address::Address,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::{
    instructions::InitializeAccount3,
    state::{Account as TokenAccount, Mint},
};

use crate::{
    constants::{FUNDRAISER_SEED, MIN_AMOUNT_TO_RAISE, VAULT_SEED},
    error::FundraiserError,
    state::Fundraiser,
};

pub fn initialize(
    program_id: &Address,
    accounts: &mut [AccountView],
    data: &[u8],
) -> ProgramResult {
    let [maker, mint_to_raise, fundraiser, vault, _token_program, _system_program, ..] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // --- parse instruction data -------------------------------------------
    if data.len() != 11 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let amount = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let duration = data[8];
    let fundraiser_bump = data[9];
    let vault_bump = data[10];

    // --- validation -------------------------------------------------------
    if !maker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Effective minimum target is `3.pow(decimals)`, identical to Anchor.
    let decimals = Mint::from_account_view(mint_to_raise)?.decimals();
    let min_amount = MIN_AMOUNT_TO_RAISE
        .checked_pow(decimals as u32)
        .ok_or(FundraiserError::InvalidAmount)?;
    if amount <= min_amount {
        return Err(FundraiserError::InvalidAmount.into());
    }

    // --- create the fundraiser PDA account --------------------------------
    // The signed CPI proves `fundraiser == PDA(["fundraiser", maker], bump)`:
    // the runtime rejects the call unless the seeds derive this exact address.
    {
        let fr_bump = [fundraiser_bump];
        let fr_seeds = [
            Seed::from(FUNDRAISER_SEED),
            Seed::from(maker.address().as_array()),
            Seed::from(&fr_bump),
        ];
        CreateAccount::with_minimum_balance(
            maker,
            fundraiser,
            Fundraiser::LEN as u64,
            program_id,
            None,
        )?
        .invoke_signed(&[Signer::from(&fr_seeds)])?;
    }

    // --- create and initialize the vault token account --------------------
    {
        let v_bump = [vault_bump];
        let v_seeds = [
            Seed::from(VAULT_SEED),
            Seed::from(fundraiser.address().as_array()),
            Seed::from(&v_bump),
        ];
        CreateAccount::with_minimum_balance(
            maker,
            vault,
            TokenAccount::LEN as u64,
            &pinocchio_token::ID,
            None,
        )?
        .invoke_signed(&[Signer::from(&v_seeds)])?;
    }
    // Vault authority is the fundraiser PDA.
    InitializeAccount3::new(vault, mint_to_raise, fundraiser.address()).invoke()?;

    // --- write the fundraiser state ---------------------------------------
    let time_started = Clock::get()?.unix_timestamp;
    let mut fr = Fundraiser::load_mut(fundraiser)?;
    fr.maker = *maker.address().as_array();
    fr.mint_to_raise = *mint_to_raise.address().as_array();
    fr.vault = *vault.address().as_array();
    fr.set_amount_to_raise(amount);
    fr.set_current_amount(0);
    fr.set_time_started(time_started);
    fr.duration = duration;
    fr.bump = fundraiser_bump;

    Ok(())
}
