use anchor_lang::prelude::*;

use crate::state::UserAccount;

#[derive(Accounts)]
pub struct CallbackRandomness<'info> {
    /// This check ensures that the vrf_program_identity (which is a PDA) is a signer,
    /// enforcing the callback is executed by the VRF program through CPI.
    #[account(address = ephemeral_vrf_sdk::consts::VRF_PROGRAM_IDENTITY)]
    pub vrf_program_identity: Signer<'info>,

    #[account(mut)]
    pub user_account: Account<'info, UserAccount>,
}

impl<'info> CallbackRandomness<'info> {
    pub fn callback_randomness(&mut self, randomness: [u8; 32]) -> Result<()> {
        let rnd = ephemeral_vrf_sdk::rnd::random_u64(&randomness);
        self.user_account.data = rnd;
        Ok(())
    }
}
