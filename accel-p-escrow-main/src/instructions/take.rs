use pinocchio::{
    AccountView, ProgramResult,
    cpi::{Seed, Signer},
    error::ProgramError,
};

use crate::state::Escrow;

pub fn process_take_instruction(accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
    let [
        taker,
        maker,
        mint_a,
        mint_b,
        escrow_account,
        taker_ata_a,
        taker_ata_b,
        maker_ata_b,
        escrow_ata,
        system_program,
        token_program,
        _associated_token_program @ ..,
    ] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !taker.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !escrow_account.owned_by(&crate::ID) {
        return Err(ProgramError::IllegalOwner);
    }

    {
        let vault_state = pinocchio_token::state::Account::from_account_view(escrow_ata)?;
        if vault_state.owner() != escrow_account.address() {
            return Err(ProgramError::IllegalOwner);
        }
        if vault_state.mint() != mint_a.address() {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    {
        let taker_b_state = pinocchio_token::state::Account::from_account_view(taker_ata_b)?;
        if taker_b_state.owner() != taker.address() {
            return Err(ProgramError::IllegalOwner);
        }
        if taker_b_state.mint() != mint_b.address() {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let (maker_seed, amount_to_receive, amount_to_give, bump) = {
        let escrow_state = Escrow::from_account_info(escrow_account)?;
        if escrow_state.maker() != maker.address() {
            return Err(ProgramError::InvalidAccountData);
        }
        if escrow_state.mint_a() != mint_a.address() {
            return Err(ProgramError::InvalidAccountData);
        }
        if escrow_state.mint_b() != mint_b.address() {
            return Err(ProgramError::InvalidAccountData);
        }
        (
            *escrow_state.maker_raw(),
            escrow_state.amount_to_receive(),
            escrow_state.amount_to_give(),
            escrow_state.bump,
        )
    };

    pinocchio_associated_token_account::instructions::CreateIdempotent {
        funding_account: taker,
        account: maker_ata_b,
        wallet: maker,
        mint: mint_b,
        system_program,
        token_program,
    }
    .invoke()?;

    pinocchio_associated_token_account::instructions::CreateIdempotent {
        funding_account: taker,
        account: taker_ata_a,
        wallet: taker,
        mint: mint_a,
        system_program,
        token_program,
    }
    .invoke()?;

    pinocchio_token::instructions::Transfer::new(
        taker_ata_b,
        maker_ata_b,
        taker,
        amount_to_receive,
    )
    .invoke()?;

    let bump_bytes = [bump];
    let signer_seeds = [
        Seed::from(b"escrow"),
        Seed::from(maker_seed.as_ref()),
        Seed::from(bump_bytes.as_ref()),
    ];

    pinocchio_token::instructions::Transfer::new(
        escrow_ata,
        taker_ata_a,
        escrow_account,
        amount_to_give,
    )
    .invoke_signed(&[Signer::from(&signer_seeds)])?;

    pinocchio_token::instructions::CloseAccount::new(escrow_ata, maker, escrow_account)
        .invoke_signed(&[Signer::from(&signer_seeds)])?;

    let escrow_lamports = escrow_account.lamports();
    let maker_new_lamports = maker
        .lamports()
        .checked_add(escrow_lamports)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    maker.set_lamports(maker_new_lamports);
    escrow_account.set_lamports(0);
    escrow_account.close()?;

    Ok(())
}
