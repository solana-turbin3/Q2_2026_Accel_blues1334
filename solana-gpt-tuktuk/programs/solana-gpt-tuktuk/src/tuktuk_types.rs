use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use std::collections::HashMap;

// Generate types + CPI client from the tuktuk IDL
declare_program!(tuktuk);

pub use tuktuk::ID as TUKTUK_PROGRAM_ID;

/// Compile instructions into a CompiledTransactionV0 + remaining accounts.
/// This is a port of tuktuk-program's compile_transaction utility.
pub fn compile_transaction(
    instructions: Vec<Instruction>,
    signer_seeds: Vec<Vec<Vec<u8>>>,
) -> Result<(tuktuk::types::CompiledTransactionV0, Vec<AccountMeta>)> {
    let mut pubkeys_to_metadata: HashMap<Pubkey, AccountMeta> = HashMap::new();

    for ix in &instructions {
        pubkeys_to_metadata
            .entry(ix.program_id)
            .or_insert(AccountMeta {
                pubkey: ix.program_id,
                is_signer: false,
                is_writable: false,
            });

        for key in &ix.accounts {
            let entry = pubkeys_to_metadata
                .entry(key.pubkey)
                .or_insert(AccountMeta {
                    is_signer: false,
                    is_writable: false,
                    pubkey: key.pubkey,
                });
            entry.is_writable |= key.is_writable;
            entry.is_signer |= key.is_signer;
        }
    }

    let mut sorted_accounts: Vec<Pubkey> = pubkeys_to_metadata.keys().cloned().collect();
    sorted_accounts.sort_by(|a, b| {
        let a_meta = &pubkeys_to_metadata[a];
        let b_meta = &pubkeys_to_metadata[b];

        fn get_priority(meta: &AccountMeta) -> u8 {
            match (meta.is_signer, meta.is_writable) {
                (true, true) => 0,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 3,
            }
        }

        get_priority(a_meta).cmp(&get_priority(b_meta))
    });

    let mut num_rw_signers = 0u8;
    let mut num_ro_signers = 0u8;
    let mut num_rw = 0u8;

    for k in &sorted_accounts {
        let metadata = &pubkeys_to_metadata[k];
        if metadata.is_signer && metadata.is_writable {
            num_rw_signers += 1;
        } else if metadata.is_signer {
            num_ro_signers += 1;
        } else if metadata.is_writable {
            num_rw += 1;
        }
    }

    let accounts_to_index: HashMap<Pubkey, u8> = sorted_accounts
        .iter()
        .enumerate()
        .map(|(i, k)| (*k, i as u8))
        .collect();

    let compiled_instructions: Vec<tuktuk::types::CompiledInstructionV0> = instructions
        .iter()
        .map(|ix| tuktuk::types::CompiledInstructionV0 {
            program_id_index: *accounts_to_index.get(&ix.program_id).unwrap(),
            accounts: ix
                .accounts
                .iter()
                .map(|k| *accounts_to_index.get(&k.pubkey).unwrap())
                .collect(),
            data: ix.data.clone(),
        })
        .collect();

    let remaining_accounts = sorted_accounts
        .iter()
        .enumerate()
        .map(|(index, k)| AccountMeta {
            pubkey: *k,
            is_signer: false,
            is_writable: index < num_rw_signers as usize
                || (index >= (num_rw_signers + num_ro_signers) as usize
                    && index < (num_rw_signers + num_ro_signers + num_rw) as usize),
        })
        .collect();

    Ok((
        tuktuk::types::CompiledTransactionV0 {
            num_ro_signers,
            num_rw_signers,
            num_rw,
            instructions: compiled_instructions,
            signer_seeds,
            accounts: sorted_accounts,
        },
        remaining_accounts,
    ))
}
