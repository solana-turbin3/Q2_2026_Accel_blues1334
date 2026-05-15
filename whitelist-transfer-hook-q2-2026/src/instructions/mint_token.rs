use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke, system_instruction},
};
use spl_token_2022_interface::{
    extension::{transfer_hook::instruction::initialize as init_transfer_hook, ExtensionType},
    instruction::{initialize_mint2, initialize_mint_close_authority},
    state::Mint,
    ID as TOKEN_2022_PROGRAM_ID,
};

#[derive(Accounts)]
pub struct TokenFactory<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(address = TOKEN_2022_PROGRAM_ID)]
    pub token_program: UncheckedAccount<'info>,
}

impl<'info> TokenFactory<'info> {
    pub fn initialize_mint(&mut self) -> Result<()> {
        let mint_size = ExtensionType::try_calculate_account_len::<Mint>(&[
            ExtensionType::TransferHook,
            ExtensionType::MintCloseAuthority,
        ])
        .map_err(|_| ProgramError::InvalidAccountData)?;

        let mint_rent = Rent::get()?.minimum_balance(mint_size);

        let create_mint_ix = system_instruction::create_account(
            &self.user.key(),
            &self.mint.key(),
            mint_rent,
            mint_size as u64,
            &TOKEN_2022_PROGRAM_ID,
        );
        invoke(
            &create_mint_ix,
            &[
                self.user.to_account_info(),
                self.mint.to_account_info(),
                self.system_program.to_account_info(),
            ],
        )?;

        let init_hook_ix = init_transfer_hook(
            &TOKEN_2022_PROGRAM_ID,
            &self.mint.key(),
            Some(self.user.key()),
            Some(crate::ID),
        )
        .map_err(|_| ProgramError::InvalidInstructionData)?;
        invoke(
            &init_hook_ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;

        let init_close_authority_ix = initialize_mint_close_authority(
            &TOKEN_2022_PROGRAM_ID,
            &self.mint.key(),
            Some(&self.user.key()),
        )
        .map_err(|_| ProgramError::InvalidInstructionData)?;
        invoke(
            &init_close_authority_ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;

        let init_mint_ix = initialize_mint2(
            &TOKEN_2022_PROGRAM_ID,
            &self.mint.key(),
            &self.user.key(),
            None,
            9,
        )
        .map_err(|_| ProgramError::InvalidInstructionData)?;
        invoke(
            &init_mint_ix,
            &[
                self.mint.to_account_info(),
                self.token_program.to_account_info(),
            ],
        )?;

        Ok(())
    }
}
