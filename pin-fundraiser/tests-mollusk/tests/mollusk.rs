//! Mollusk integration tests for the Pinocchio fundraiser program.
//!
//! Mollusk runs the *compiled SBF program* inside a minimal SVM, so these
//! tests exercise the real on-chain bytecode (CPIs to the SPL Token and System
//! programs included) and report the exact compute-unit cost of each
//! instruction.
//!
//! Run with:  `cargo test -p tests-mollusk -- --nocapture`
//! (build the program first: `cargo build-sbf`)

use std::{collections::HashMap, str::FromStr};

use mollusk_svm::{program::keyed_account_for_system_program, Mollusk};
use mollusk_svm_programs_token::token;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;

const PROGRAM_ID: &str = "HeaBbw9V4mTWhMXrT2EB6W3EdZXgZrW2fm3Kq3CTUsLt";

const DECIMALS: u8 = 6;
const MINT_RENT: u64 = 1_461_600;
const TOKEN_ACC_RENT: u64 = 2_039_280;
const ONE_SOL: u64 = 1_000_000_000;

// ----------------------------------------------------------------------------
// Manual SPL state packing (avoids pulling spl-token-interface into the test).
// ----------------------------------------------------------------------------

fn mint_account(authority: &Pubkey) -> Account {
    let mut data = vec![0u8; 82];
    // mint_authority: COption::Some
    data[0..4].copy_from_slice(&1u32.to_le_bytes());
    data[4..36].copy_from_slice(authority.as_ref());
    // supply = 0  (data[36..44] already zero)
    data[44] = DECIMALS;
    data[45] = 1; // is_initialized
                  // freeze_authority: COption::None (data[46..82] zero)
    Account {
        lamports: MINT_RENT,
        data,
        owner: token::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Account {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    // delegate: COption::None (72..108)
    data[108] = 1; // state = Initialized
                   // is_native: None, delegated_amount: 0, close_authority: None
    Account {
        lamports: TOKEN_ACC_RENT,
        data,
        owner: token::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: vec![],
        owner: system_program::id(),
        executable: false,
        rent_epoch: 0,
    }
}

/// A tiny mutable ledger so we can thread account state across an instruction
/// chain (Mollusk returns the resulting accounts after each call).
struct Ledger {
    mollusk: Mollusk,
    program_id: Pubkey,
    accounts: HashMap<Pubkey, Account>,
}

impl Ledger {
    fn new(program_id: Pubkey) -> Self {
        // Point Mollusk at the program's SBF build output.
        let so_dir = format!("{}/../target/deploy", env!("CARGO_MANIFEST_DIR"));
        std::env::set_var("SBF_OUT_DIR", so_dir);

        let mut mollusk = Mollusk::new(&program_id, "pinocchio_fundraiser");
        token::add_program(&mut mollusk);
        Ledger {
            mollusk,
            program_id,
            accounts: HashMap::new(),
        }
    }

    fn set(&mut self, key: Pubkey, account: Account) {
        self.accounts.insert(key, account);
    }

    fn get(&self, key: &Pubkey) -> Account {
        self.accounts.get(key).cloned().unwrap_or_default()
    }

    /// Resolve the account for a meta, substituting the real (executable)
    /// program accounts for the token and system program ids.
    fn account_for(&self, key: &Pubkey) -> Account {
        if key == &token::ID {
            token::keyed_account().1
        } else if key == &system_program::id() {
            keyed_account_for_system_program().1
        } else {
            self.get(key)
        }
    }

    fn accounts_for(&self, metas: &[AccountMeta]) -> Vec<(Pubkey, Account)> {
        metas
            .iter()
            .map(|m| (m.pubkey, self.account_for(&m.pubkey)))
            .collect()
    }

    /// Build, run and validate an instruction; thread resulting accounts back
    /// into the ledger; return the compute units consumed.
    fn run(&mut self, label: &str, metas: Vec<AccountMeta>, data: Vec<u8>) -> u64 {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: metas.clone(),
            data,
        };
        // Assemble the (pubkey, account) list in meta order (program ids
        // resolve to their executable accounts).
        let accs = self.accounts_for(&metas);

        let result = self.mollusk.process_instruction(&ix, &accs);
        assert!(
            result.program_result.is_ok(),
            "[{label}] expected success, got {:?}",
            result.program_result
        );
        for (k, a) in &result.resulting_accounts {
            self.accounts.insert(*k, a.clone());
        }
        println!("  CU[{label}] = {}", result.compute_units_consumed);
        result.compute_units_consumed
    }

    /// Like `run`, but asserts the program *fails*.
    fn run_expect_err(&mut self, label: &str, metas: Vec<AccountMeta>, data: Vec<u8>) {
        let ix = Instruction {
            program_id: self.program_id,
            accounts: metas.clone(),
            data,
        };
        let accs = self.accounts_for(&metas);

        let result = self.mollusk.process_instruction(&ix, &accs);
        assert!(
            result.program_result.is_err(),
            "[{label}] expected failure, but it succeeded"
        );
        println!("  [{label}] failed as expected: {:?}", result.program_result);
    }
}

// ----------------------------------------------------------------------------
// Instruction-data + account-meta builders
// ----------------------------------------------------------------------------

struct Pdas {
    fundraiser: Pubkey,
    fundraiser_bump: u8,
    vault: Pubkey,
    vault_bump: u8,
}

fn derive_pdas(program_id: &Pubkey, maker: &Pubkey) -> Pdas {
    let (fundraiser, fundraiser_bump) =
        Pubkey::find_program_address(&[b"fundraiser", maker.as_ref()], program_id);
    let (vault, vault_bump) =
        Pubkey::find_program_address(&[b"vault", fundraiser.as_ref()], program_id);
    Pdas {
        fundraiser,
        fundraiser_bump,
        vault,
        vault_bump,
    }
}

fn derive_contributor(program_id: &Pubkey, fundraiser: &Pubkey, contributor: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"contributor", fundraiser.as_ref(), contributor.as_ref()],
        program_id,
    )
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[test]
fn initialize_contribute_refund() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut l = Ledger::new(program_id);

    let maker = Pubkey::new_unique();
    let contributor = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let pdas = derive_pdas(&program_id, &maker);
    let (contributor_pda, contributor_bump) =
        derive_contributor(&program_id, &pdas.fundraiser, &contributor);
    let contributor_ata = Pubkey::new_unique();

    let amount_to_raise: u64 = 30_000_000; // 30 tokens
    let contribution: u64 = 1_000_000; // 1 token (<= 10% cap of 3 tokens)

    // --- seed accounts ----------------------------------------------------
    l.set(maker, system_account(10 * ONE_SOL));
    l.set(contributor, system_account(10 * ONE_SOL));
    l.set(mint, mint_account(&maker));
    l.set(contributor_ata, token_account(&mint, &contributor, 10_000_000));
    l.set(pdas.fundraiser, system_account(0));
    l.set(pdas.vault, system_account(0));
    l.set(contributor_pda, system_account(0));

    println!("\n== initialize / contribute / refund ==");

    // --- initialize -------------------------------------------------------
    let mut data = vec![0u8];
    data.extend_from_slice(&amount_to_raise.to_le_bytes());
    data.push(0); // duration
    data.push(pdas.fundraiser_bump);
    data.push(pdas.vault_bump);
    l.run(
        "initialize",
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    );

    // fundraiser account now exists with our state (122 bytes, program-owned).
    let fr = l.get(&pdas.fundraiser);
    assert_eq!(fr.owner, program_id);
    assert_eq!(fr.data.len(), 122);
    assert_eq!(&fr.data[0..32], maker.as_ref()); // maker
    assert_eq!(
        u64::from_le_bytes(fr.data[96..104].try_into().unwrap()),
        amount_to_raise
    );

    // --- contribute -------------------------------------------------------
    let mut data = vec![1u8];
    data.extend_from_slice(&contribution.to_le_bytes());
    data.push(contributor_bump);
    l.run(
        "contribute",
        vec![
            AccountMeta::new(contributor, true),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(contributor_pda, false),
            AccountMeta::new(contributor_ata, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    );

    // vault received the contribution; contributor record tracks it.
    let vault = l.get(&pdas.vault);
    assert_eq!(u64::from_le_bytes(vault.data[64..72].try_into().unwrap()), contribution);
    let crec = l.get(&contributor_pda);
    assert_eq!(crec.data.len(), 8);
    assert_eq!(u64::from_le_bytes(crec.data[0..8].try_into().unwrap()), contribution);

    // --- refund (campaign expired, target not met) ------------------------
    l.run(
        "refund",
        vec![
            AccountMeta::new(contributor, true),
            AccountMeta::new_readonly(maker, false),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(contributor_pda, false),
            AccountMeta::new(contributor_ata, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        vec![3u8, contributor_bump],
    );

    // contributor record closed, tokens returned to the contributor ATA.
    let crec = l.get(&contributor_pda);
    assert!(crec.data.is_empty() || crec.owner == system_program::id());
    let ata = l.get(&contributor_ata);
    assert_eq!(
        u64::from_le_bytes(ata.data[64..72].try_into().unwrap()),
        10_000_000,
        "contributor fully refunded"
    );
}

#[test]
fn initialize_then_check_releases_to_maker() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut l = Ledger::new(program_id);

    let maker = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let pdas = derive_pdas(&program_id, &maker);
    let maker_ata = Pubkey::new_unique();

    let amount_to_raise: u64 = 5_000_000;

    l.set(maker, system_account(10 * ONE_SOL));
    l.set(mint, mint_account(&maker));
    l.set(maker_ata, token_account(&mint, &maker, 0));
    l.set(pdas.fundraiser, system_account(0));
    l.set(pdas.vault, system_account(0));

    println!("\n== initialize / check_contributions (target met) ==");

    // initialize
    let mut data = vec![0u8];
    data.extend_from_slice(&amount_to_raise.to_le_bytes());
    data.push(0);
    data.push(pdas.fundraiser_bump);
    data.push(pdas.vault_bump);
    l.run(
        "initialize",
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    );

    // Simulate a fully-funded vault by funding the (real) vault token account.
    l.set(pdas.vault, token_account(&mint, &pdas.fundraiser, amount_to_raise));

    // check_contributions
    l.run(
        "check",
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new(maker_ata, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        vec![2u8],
    );

    // maker received the full amount; fundraiser closed.
    let ata = l.get(&maker_ata);
    assert_eq!(
        u64::from_le_bytes(ata.data[64..72].try_into().unwrap()),
        amount_to_raise
    );
    let fr = l.get(&pdas.fundraiser);
    assert!(fr.data.is_empty() || fr.owner == system_program::id());
}

#[test]
fn check_fails_when_target_not_met() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut l = Ledger::new(program_id);

    let maker = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let pdas = derive_pdas(&program_id, &maker);
    let maker_ata = Pubkey::new_unique();

    l.set(maker, system_account(10 * ONE_SOL));
    l.set(mint, mint_account(&maker));
    l.set(maker_ata, token_account(&mint, &maker, 0));
    l.set(pdas.fundraiser, system_account(0));
    l.set(pdas.vault, system_account(0));

    println!("\n== check_contributions (target NOT met -> error) ==");

    let mut data = vec![0u8];
    data.extend_from_slice(&5_000_000u64.to_le_bytes());
    data.push(0);
    data.push(pdas.fundraiser_bump);
    data.push(pdas.vault_bump);
    l.run(
        "initialize",
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new_readonly(token::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    );

    // vault holds only 1 token, target is 5 -> TargetNotMet.
    l.set(pdas.vault, token_account(&mint, &pdas.fundraiser, 1_000_000));
    l.run_expect_err(
        "check",
        vec![
            AccountMeta::new(maker, true),
            AccountMeta::new(pdas.fundraiser, false),
            AccountMeta::new(pdas.vault, false),
            AccountMeta::new(maker_ata, false),
            AccountMeta::new_readonly(token::ID, false),
        ],
        vec![2u8],
    );
}
