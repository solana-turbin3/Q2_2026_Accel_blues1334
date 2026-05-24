use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};
use sha2::{Sha256, Digest};

use crate::state::GptConfig;

/// Oracle program ID
pub const ORACLE_PROGRAM_ID: Pubkey =
    pubkey!("LLMrieZMpbJFwN52WgmBNMxYojrpRVYXdC1RCweEbab");

/// Oracle AccountMeta for callback registration (matches oracle's struct)
#[derive(AnchorSerialize)]
struct OracleAccountMeta {
    pubkey: Pubkey,
    is_signer: bool,
    is_writable: bool,
}

/// Args for interact_with_llm (matches oracle's Borsh layout)
#[derive(AnchorSerialize)]
struct InteractWithLlmArgs {
    text: String,
    callback_program_id: Pubkey,
    callback_discriminator: [u8; 8],
    account_metas: Option<Vec<OracleAccountMeta>>,
}

#[derive(Accounts)]
pub struct AskGpt<'info> {
    #[account(
        seeds = [b"gpt_config"],
        bump = gpt_config.bump,
    )]
    pub gpt_config: Account<'info, GptConfig>,

    /// CHECK: System-owned PDA used as payer for oracle interaction.
    /// Never init'd as an Anchor account — stays system-owned so it can pay rent.
    /// Fund with SOL before first use.
    #[account(
        mut,
        seeds = [b"payer"],
        bump,
    )]
    pub payer: UncheckedAccount<'info>,

    /// CHECK: Oracle interaction PDA — created/managed by oracle program.
    /// Seeds: [b"interaction", payer.key(), context_account.key()] under oracle.
    #[account(mut)]
    pub interaction: UncheckedAccount<'info>,

    /// CHECK: Oracle context account (stored in gpt_config)
    #[account(address = gpt_config.context_account)]
    pub context_account: UncheckedAccount<'info>,

    /// CHECK: Oracle program
    #[account(address = ORACLE_PROGRAM_ID)]
    pub oracle_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> AskGpt<'info> {
    pub fn handler(&mut self, bumps: &AskGptBumps) -> Result<()> {
        // Compute interact_with_llm discriminator: sha256("global:interact_with_llm")[..8]
        let mut hasher = Sha256::new();
        hasher.update(b"global:interact_with_llm");
        let hash = hasher.finalize();
        let disc: [u8; 8] = hash[..8].try_into().unwrap();

        // Compute our receive_response discriminator for the callback
        let mut cb_hasher = Sha256::new();
        cb_hasher.update(b"global:receive_response");
        let cb_hash = cb_hasher.finalize();
        let cb_disc: [u8; 8] = cb_hash[..8].try_into().unwrap();

        // Build args
        let args = InteractWithLlmArgs {
            text: self.gpt_config.prompt.clone(),
            callback_program_id: crate::ID,
            callback_discriminator: cb_disc,
            account_metas: Some(vec![
                OracleAccountMeta {
                    pubkey: self.gpt_config.key(),
                    is_signer: false,
                    is_writable: true,
                },
            ]),
        };

        let mut ix_data = disc.to_vec();
        AnchorSerialize::serialize(&args, &mut ix_data)?;

        // Use the system-owned payer PDA as the oracle's payer
        let ix = Instruction {
            program_id: ORACLE_PROGRAM_ID,
            accounts: vec![
                AccountMeta { pubkey: self.payer.key(), is_signer: true, is_writable: true },
                AccountMeta { pubkey: self.interaction.key(), is_signer: false, is_writable: true },
                AccountMeta { pubkey: self.context_account.key(), is_signer: false, is_writable: false },
                AccountMeta { pubkey: self.system_program.key(), is_signer: false, is_writable: false },
            ],
            data: ix_data,
        };

        let payer_seeds: &[&[u8]] = &[b"payer", &[bumps.payer]];

        invoke_signed(
            &ix,
            &[
                self.payer.to_account_info(),
                self.interaction.to_account_info(),
                self.context_account.to_account_info(),
                self.system_program.to_account_info(),
                self.oracle_program.to_account_info(),
            ],
            &[payer_seeds],
        )?;

        msg!("Asked GPT oracle with prompt");
        Ok(())
    }
}
