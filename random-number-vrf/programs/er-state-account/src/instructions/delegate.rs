use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::{anchor::delegate, cpi::DelegateConfig};

#[delegate]
#[derive(Accounts)]
pub struct Delegate<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        del,
        seeds = [b"user", user.key().as_ref()],
        bump,
    )]
    /// CHECK: Ownership will be changed by the delegation CPI, so we must not
    /// deserialize/serialize this as an Anchor `Account<T>`.
    pub user_account: AccountInfo<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub validator: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Delegate<'info> {
    
    pub fn delegate(&mut self, _bumps: &DelegateBumps) -> Result<()> {

        // NOTE: do NOT include the bump here; ephemeral_rollups_sdk re-derives
        // it internally with find_program_address and appends it via
        // seeds_with_bump before calling invoke_signed.
        let pda_seeds: &[&[u8]] = &[
            b"user",
            self.user.key.as_ref(),
        ];

        self.delegate_user_account(
            &self.user, 
            pda_seeds, 
            DelegateConfig {
                validator: Some(self.validator.key()),
                ..DelegateConfig::default()
            }
        )?;

        Ok(())
    }
}