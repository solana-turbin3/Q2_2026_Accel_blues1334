use anchor_lang::prelude::*;
use ephemeral_vrf_sdk::anchor::vrf;
use ephemeral_vrf_sdk::instructions::{create_request_randomness_ix, RequestRandomnessParams};
use ephemeral_vrf_sdk::types::SerializableAccountMeta;

use crate::state::UserAccount;

/// Request randomness from inside an Ephemeral Rollup.
///
/// Same as `request_randomness` but constrained to the ephemeral queue
/// (`ephemeral_vrf_sdk::consts::DEFAULT_EPHEMERAL_QUEUE`) so it can be invoked
/// while the `user_account` is delegated to a MagicBlock validator.
#[vrf]
#[derive(Accounts)]
pub struct RequestRandomnessEr<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"user", payer.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Account<'info, UserAccount>,

    /// CHECK: The ephemeral oracle queue
    #[account(mut, address = ephemeral_vrf_sdk::consts::DEFAULT_EPHEMERAL_QUEUE)]
    pub oracle_queue: AccountInfo<'info>,
}

impl<'info> RequestRandomnessEr<'info> {
    pub fn request_randomness_er(&mut self, client_seed: u8) -> Result<()> {
        let ix = create_request_randomness_ix(RequestRandomnessParams {
            payer: self.payer.key(),
            oracle_queue: self.oracle_queue.key(),
            callback_program_id: crate::ID,
            callback_discriminator:
                crate::instruction::CallbackRandomness::DISCRIMINATOR.to_vec(),
            caller_seed: [client_seed; 32],
            accounts_metas: Some(vec![SerializableAccountMeta {
                pubkey: self.user_account.key(),
                is_signer: false,
                is_writable: true,
            }]),
            ..Default::default()
        });

        self.invoke_signed_vrf(&self.payer.to_account_info(), &ix)?;
        Ok(())
    }
}
