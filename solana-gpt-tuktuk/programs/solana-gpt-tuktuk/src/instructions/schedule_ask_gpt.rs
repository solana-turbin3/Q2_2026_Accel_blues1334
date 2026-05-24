use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::state::GptConfig;
use crate::instructions::ask_gpt::ORACLE_PROGRAM_ID;
use crate::tuktuk_types::{tuktuk, compile_transaction, TUKTUK_PROGRAM_ID};

#[derive(Accounts)]
#[instruction(task_id: u16)]
pub struct ScheduleAskGpt<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"gpt_config"],
        bump = gpt_config.bump,
        constraint = gpt_config.admin == admin.key(),
    )]
    pub gpt_config: Account<'info, GptConfig>,

    /// CHECK: System-owned payer PDA (same as in ask_gpt)
    #[account(
        seeds = [b"payer"],
        bump,
    )]
    pub payer_pda: UncheckedAccount<'info>,

    /// CHECK: Queue authority PDA — signs the tuktuk CPI
    #[account(
        seeds = [b"queue_authority"],
        bump,
    )]
    pub queue_authority: UncheckedAccount<'info>,

    /// CHECK: Tuktuk task_queue_authority PDA — validated by tuktuk program
    pub task_queue_authority: UncheckedAccount<'info>,

    /// CHECK: Tuktuk task queue account
    #[account(mut)]
    pub task_queue: UncheckedAccount<'info>,

    /// CHECK: Tuktuk task PDA — derived from task_queue + task_id
    #[account(mut)]
    pub task: UncheckedAccount<'info>,

    /// CHECK: Tuktuk program
    #[account(address = TUKTUK_PROGRAM_ID)]
    pub tuktuk_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> ScheduleAskGpt<'info> {
    pub fn handler(
        &mut self,
        task_id: u16,
        trigger: tuktuk::types::TriggerV0,
        bumps: &ScheduleAskGptBumps,
    ) -> Result<()> {
        // Derive the oracle interaction PDA (payer = payer_pda)
        let (interaction, _) = Pubkey::find_program_address(
            &[
                b"interaction",
                self.payer_pda.key().as_ref(),
                self.gpt_config.context_account.as_ref(),
            ],
            &ORACLE_PROGRAM_ID,
        );

        // Build the ask_gpt instruction that tuktuk will execute
        let ask_ix = Instruction {
            program_id: crate::ID,
            accounts: vec![
                AccountMeta { pubkey: self.gpt_config.key(), is_signer: false, is_writable: false },
                AccountMeta { pubkey: self.payer_pda.key(), is_signer: true, is_writable: true },
                AccountMeta { pubkey: interaction, is_signer: false, is_writable: true },
                AccountMeta { pubkey: self.gpt_config.context_account, is_signer: false, is_writable: false },
                AccountMeta { pubkey: ORACLE_PROGRAM_ID, is_signer: false, is_writable: false },
                AccountMeta { pubkey: anchor_lang::system_program::ID, is_signer: false, is_writable: false },
            ],
            data: crate::instruction::AskGpt {}.data(),
        };

        // payer PDA needs to sign the oracle CPI inside ask_gpt
        let payer_seeds = vec![
            b"payer".to_vec(),
            vec![bumps.payer_pda],
        ];

        let (compiled_tx, remaining_accounts) = compile_transaction(
            vec![ask_ix],
            vec![payer_seeds],
        )?;

        let args = tuktuk::types::QueueTaskArgsV0 {
            id: task_id,
            trigger,
            transaction: tuktuk::types::TransactionSourceV0::CompiledV0(compiled_tx),
            crank_reward: None,
            free_tasks: 0,
            description: "ask_gpt".to_string(),
        };

        // Build the queue_task_v0 CPI
        let disc = tuktuk::client::args::QueueTaskV0::DISCRIMINATOR;
        let mut ix_data = disc.to_vec();
        AnchorSerialize::serialize(&args, &mut ix_data)?;

        let mut cpi_accounts = vec![
            AccountMeta { pubkey: self.admin.key(), is_signer: true, is_writable: true },
            AccountMeta { pubkey: self.queue_authority.key(), is_signer: true, is_writable: false },
            AccountMeta { pubkey: self.task_queue_authority.key(), is_signer: false, is_writable: false },
            AccountMeta { pubkey: self.task_queue.key(), is_signer: false, is_writable: true },
            AccountMeta { pubkey: self.task.key(), is_signer: false, is_writable: true },
            AccountMeta { pubkey: self.system_program.key(), is_signer: false, is_writable: false },
        ];
        cpi_accounts.extend(remaining_accounts);

        let cpi_ix = Instruction {
            program_id: TUKTUK_PROGRAM_ID,
            accounts: cpi_accounts,
            data: ix_data,
        };

        let signer_seeds: &[&[u8]] = &[b"queue_authority", &[bumps.queue_authority]];

        invoke_signed(
            &cpi_ix,
            &[
                self.admin.to_account_info(),
                self.queue_authority.to_account_info(),
                self.task_queue_authority.to_account_info(),
                self.task_queue.to_account_info(),
                self.task.to_account_info(),
                self.system_program.to_account_info(),
                self.tuktuk_program.to_account_info(),
                self.gpt_config.to_account_info(),
                self.payer_pda.to_account_info(),
            ],
            &[signer_seeds],
        )?;

        msg!("GPT query scheduled (task_id={})", task_id);
        Ok(())
    }
}
