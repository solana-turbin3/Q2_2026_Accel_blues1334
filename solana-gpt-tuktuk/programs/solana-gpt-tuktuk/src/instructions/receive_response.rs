use anchor_lang::prelude::*;

use crate::state::GptConfig;
use crate::instructions::ask_gpt::ORACLE_PROGRAM_ID;

#[derive(Accounts)]
pub struct ReceiveResponse<'info> {
    /// CHECK: Oracle identity PDA (seeds=[b"identity"] under oracle program).
    /// Must be a signer — the oracle program signs via PDA when CPIing the callback.
    #[account(
        signer,
        constraint = {
            let (expected, _) = Pubkey::find_program_address(&[b"identity"], &ORACLE_PROGRAM_ID);
            identity.key() == expected
        },
    )]
    pub identity: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"gpt_config"],
        bump = gpt_config.bump,
    )]
    pub gpt_config: Account<'info, GptConfig>,
}

impl<'info> ReceiveResponse<'info> {
    pub fn handler(&mut self, response: String) -> Result<()> {
        // Truncate to max storage size if needed
        let truncated = if response.len() > 512 {
            response[..512].to_string()
        } else {
            response
        };

        self.gpt_config.latest_response = truncated;
        msg!("Received GPT response, stored on-chain");
        Ok(())
    }
}
