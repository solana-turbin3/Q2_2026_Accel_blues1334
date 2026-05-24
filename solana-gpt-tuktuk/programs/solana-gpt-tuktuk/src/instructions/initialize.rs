use anchor_lang::prelude::*;

use crate::state::GptConfig;

#[derive(Accounts)]
#[instruction(prompt: String)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + GptConfig::INIT_SPACE,
        seeds = [b"gpt_config"],
        bump,
    )]
    pub gpt_config: Account<'info, GptConfig>,

    /// CHECK: Oracle context account — created externally via the oracle program
    pub context_account: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn handler(
        &mut self,
        prompt: String,
        bumps: &InitializeBumps,
    ) -> Result<()> {
        self.gpt_config.set_inner(GptConfig {
            admin: self.admin.key(),
            context_account: self.context_account.key(),
            prompt,
            latest_response: String::new(),
            bump: bumps.gpt_config,
        });

        msg!("GptConfig initialized");
        Ok(())
    }
}
