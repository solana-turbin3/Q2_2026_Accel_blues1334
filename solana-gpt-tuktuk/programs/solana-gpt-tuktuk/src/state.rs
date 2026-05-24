use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct GptConfig {
    pub admin: Pubkey,
    /// Oracle context account (created externally via oracle program)
    pub context_account: Pubkey,
    /// The recurring prompt sent to the oracle
    #[max_len(256)]
    pub prompt: String,
    /// Latest response from the GPT oracle
    #[max_len(512)]
    pub latest_response: String,
    pub bump: u8,
}
