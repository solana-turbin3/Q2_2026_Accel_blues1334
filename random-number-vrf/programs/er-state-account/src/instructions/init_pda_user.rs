use anchor_lang::prelude::*;

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct InitPdaUser<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: PDA user that will own the user account
    pub pda_user: AccountInfo<'info>,

    #[account(
        init,
        payer = payer,
        space = UserAccount::INIT_SPACE,
        seeds = [b"user", pda_user.key().as_ref()],
        bump
    )]
    pub user_account: Account<'info, UserAccount>,

    pub system_program: Program<'info, System>,
}

impl<'info> InitPdaUser<'info> {
    pub fn initialize(&mut self, bumps: &InitPdaUserBumps) -> Result<()> {
        self.user_account.set_inner(UserAccount {
            user: *self.pda_user.key,
            data: 0,
            bump: bumps.user_account,
        });

        Ok(())
    }
}
