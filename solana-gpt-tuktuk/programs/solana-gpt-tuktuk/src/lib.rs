use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;
pub mod tuktuk_types;

use instructions::*;

declare_id!("9Qq5wSMFCAFSL6W4dtpgbfwqcjSd1UaiuXVmepSGDVKt");

#[program]
pub mod solana_gpt_tuktuk {
    use super::*;

    /// Initialize the GptConfig account.
    /// The oracle context account must be created externally first.
    pub fn initialize(ctx: Context<Initialize>, prompt: String) -> Result<()> {
        ctx.accounts.handler(prompt, &ctx.bumps)
    }

    /// CPI to the oracle's interact_with_llm with the stored prompt.
    /// Can be called directly or via Tuktuk scheduler.
    pub fn ask_gpt(ctx: Context<AskGpt>) -> Result<()> {
        ctx.accounts.handler(&ctx.bumps)
    }

    /// Callback invoked by the oracle identity PDA after LLM processing.
    /// Stores the response on-chain in GptConfig.
    pub fn receive_response(ctx: Context<ReceiveResponse>, response: String) -> Result<()> {
        ctx.accounts.handler(response)
    }

    /// Schedule ask_gpt via Tuktuk queue_task_v0 CPI.
    pub fn schedule_ask_gpt(
        ctx: Context<ScheduleAskGpt>,
        task_id: u16,
        trigger: tuktuk_types::tuktuk::types::TriggerV0,
    ) -> Result<()> {
        ctx.accounts.handler(task_id, trigger, &ctx.bumps)
    }
}
