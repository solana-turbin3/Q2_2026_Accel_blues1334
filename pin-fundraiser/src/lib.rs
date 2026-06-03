//! # Pinocchio Fundraiser
//!
//! A port of the Anchor "Token Fundraiser" example to [Pinocchio], optimized
//! for compute-unit consumption and binary size.
//!
//! The program lets a *maker* open a fundraising campaign for an SPL token,
//! collect contributions into a program-owned vault, release the funds once the
//! target is met, and refund contributors if the campaign expires unfunded.
//!
//! See `README.md` for the full account layout, instruction encoding and the
//! design decisions behind the port.
//!
//! [Pinocchio]: https://github.com/anza-xyz/pinocchio

#![allow(unexpected_cfgs)]

use pinocchio::{account::AccountView, address::Address, error::ProgramError, ProgramResult};

pub mod constants;
pub mod error;
pub mod instructions;
pub mod pda;
pub mod state;

/// Reference program ID (the address this crate is normally deployed to).
///
/// NOTE: This constant is *not* load-bearing — the program derives PDAs and
/// assigns account ownership using the `program_id` the runtime passes to the
/// entrypoint, so it works correctly at whatever address it is deployed to.
/// The constant is kept only for clients/tests that want a default.
pub const ID: Address = Address::from_str_const("HeaBbw9V4mTWhMXrT2EB6W3EdZXgZrW2fm3Kq3CTUsLt");

/// On-chain entrypoint, allocator and panic handler.
///
/// Gated behind the `bpf-entrypoint` feature so the crate can also be consumed
/// as a plain library (e.g. by integration tests or a client) without emitting
/// the global symbols twice.
#[cfg(feature = "bpf-entrypoint")]
mod entrypoint {
    pinocchio::entrypoint!(crate::process_instruction);
}

/// Instruction discriminators (the first byte of the instruction data).
#[repr(u8)]
enum Instruction {
    Initialize = 0,
    Contribute = 1,
    CheckContributions = 2,
    Refund = 3,
}

impl Instruction {
    #[inline(always)]
    fn from_u8(tag: u8) -> Result<Self, ProgramError> {
        Ok(match tag {
            0 => Instruction::Initialize,
            1 => Instruction::Contribute,
            2 => Instruction::CheckContributions,
            3 => Instruction::Refund,
            _ => return Err(ProgramError::InvalidInstructionData),
        })
    }
}

/// Program entrypoint: split the discriminator from the payload and dispatch.
#[inline(always)]
pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let (tag, data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match Instruction::from_u8(*tag)? {
        Instruction::Initialize => instructions::initialize(program_id, accounts, data),
        Instruction::Contribute => instructions::contribute(program_id, accounts, data),
        Instruction::CheckContributions => {
            instructions::check_contributions(program_id, accounts, data)
        }
        Instruction::Refund => instructions::refund(program_id, accounts, data),
    }
}
