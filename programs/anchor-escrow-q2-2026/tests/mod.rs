#![cfg(not(feature = "idl-build"))]

mod tests {

    use {
        anchor_lang::{
            prelude::msg,
            solana_program::pubkey::Pubkey as AnchorPubkey,
            AccountDeserialize, InstructionData,
        },
        anchor_spl::{
            associated_token::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
        },
        litesvm::LiteSVM,
        litesvm_token::{
            spl_token::{self, ID as TOKEN_PROGRAM_ID},
            CreateAssociatedTokenAccount, CreateMint, MintTo,
        },
        solana_address::Address,
        solana_keypair::Keypair,
        solana_instruction::{AccountMeta, Instruction},
        solana_message::Message,
        solana_program_pack::Pack,
        solana_signer::Signer,
        solana_transaction::Transaction,
    };

    fn log_account(program: &LiteSVM, label: &str, address: &Address) {
        match program.get_account(address) {
            Some(account) => {
                msg!("[acct] {} exists (lamports={})", label, account.lamports);
            }
            None => {
                msg!("[acct] {} missing", label);
            }
        }
    }

    fn log_token_account(program: &LiteSVM, label: &str, address: &Address) {
        match program.get_account(address) {
            Some(account) => {
                let token = spl_token::state::Account::unpack(&account.data).unwrap();
                msg!("[token] {} amount={}", label, token.amount);
            }
            None => {
                msg!("[token] {} missing", label);
            }
        }
    }

    fn address_to_anchor_pubkey(address: &Address) -> AnchorPubkey {
        let bytes: [u8; 32] = address.as_ref().try_into().unwrap();
        AnchorPubkey::new_from_array(bytes)
    }

    fn get_associated_token_address(owner: &Address, mint: &Address) -> Address {
        let associated_token_program_id = Address::from(ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes());
        let seeds = [owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()];
        Address::find_program_address(&seeds, &associated_token_program_id).0
    }

    // Setup function to initialize LiteSVM and create a payer keypair
    fn setup() -> (LiteSVM, Keypair) {
        let program_id = Address::from(anchor_escrow_q2_2026::id().to_bytes());
        let payer = Keypair::new();
        let mut svm = LiteSVM::new();
        let bytes = include_bytes!("../../../target/deploy/anchor_escrow_q2_2026.so");
        msg!("\n\n[setup] program_id={} payer={}", program_id, payer.pubkey());
        svm.add_program(program_id, bytes).unwrap();
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

        // Return the LiteSVM instance and payer keypair
        (svm, payer)
    }

    #[test]
    fn test_make() {
        // Setup the test environment by initializing LiteSVM and creating a payer keypair
        let (mut program, payer) = setup();

        let program_id = Address::from(anchor_escrow_q2_2026::id().to_bytes());
        msg!("[test_make] start");

        // Get the maker's public key from the payer keypair
        let maker = payer.pubkey();
        let maker_address = maker;
        msg!("[test_make] maker={}", maker_address);

        // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
        // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker_address)
            .send()
            .unwrap();
        let mint_a_pubkey = mint_a;

        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker_address)
            .send()
            .unwrap();
        let mint_b_pubkey = mint_b;

        // Create the maker's associated token account for Mint A
        // This is done using litesvm-token's CreateAssociatedTokenAccount utility
        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker_address)
            .send()
            .unwrap();
        let maker_ata_a_pubkey = maker_ata_a;

        // Derive the PDA for the escrow account using the maker's public key and a seed value
        let (escrow, _bump) = Address::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &program_id,
        );

        // Derive the PDA for the vault associated token account using the escrow PDA and Mint A
        let vault = get_associated_token_address(&escrow, &mint_a);
        let vault_address = vault;

        // Mint 1,000 tokens (with 6 decimal places) of Mint A to the maker's associated token account
        MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000000000)
            .send()
            .unwrap();
        log_token_account(&program, "maker_ata_a (after mint)", &maker_ata_a_pubkey);

        // Create the "Make" instruction to deposit tokens into the escrow
        let make_ix = Instruction::new_with_bytes(
            program_id,
            &anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
            vec![
                AccountMeta::new(maker, true),
                AccountMeta::new_readonly(mint_a_pubkey, false),
                AccountMeta::new_readonly(mint_b_pubkey, false),
                AccountMeta::new(maker_ata_a_pubkey, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(
                    Address::from(ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes()),
                    false,
                ),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(
                    Address::from(anchor_lang::system_program::ID.to_bytes()),
                    false,
                ),
            ],
        );

        // Create and send the transaction containing the "Make" instruction
        let message = Message::new(&[make_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();

        let transaction = Transaction::new(&[&payer], message, recent_blockhash);

        // Send the transaction and capture the result
        let tx = program.send_transaction(transaction).unwrap();

        // Log transaction details
        msg!("[test_make] make tx ok sig={} cu={}", tx.signature, tx.compute_units_consumed);

        // Verify the vault account and escrow account data after the "Make" instruction
        log_token_account(&program, "vault (after make)", &vault_address);
        log_account(&program, "escrow (after make)", &escrow);
        let vault_account = program.get_account(&vault_address).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);
        assert_eq!(vault_data.owner, escrow);
        assert_eq!(vault_data.mint, mint_a_pubkey);

        let escrow_account = program.get_account(&escrow).unwrap();
        let escrow_data = anchor_escrow_q2_2026::state::Escrow::try_deserialize(
            &mut escrow_account.data.as_ref(),
        )
        .unwrap();
        assert_eq!(escrow_data.seed, 123u64);
        assert_eq!(escrow_data.maker, address_to_anchor_pubkey(&maker));
        assert_eq!(escrow_data.mint_a, address_to_anchor_pubkey(&mint_a_pubkey));
        assert_eq!(escrow_data.mint_b, address_to_anchor_pubkey(&mint_b_pubkey));
        assert_eq!(escrow_data.receive, 10);
        msg!("[test_make] done\n\n");
    }

    #[test]
    fn test_take() {
        let (mut program, maker) = setup();
        let program_id = Address::from(anchor_escrow_q2_2026::id().to_bytes());

        msg!("\n\n[test_take] start");

        let taker = Keypair::new();
        program.airdrop(&taker.pubkey(), 1_000_000_000).unwrap();

        let maker_address = maker.pubkey();
        let taker_address = taker.pubkey();
        msg!("[test_take] maker={} taker={}", maker_address, taker_address);

        let mint_a = CreateMint::new(&mut program, &maker)
            .decimals(6)
            .authority(&maker_address)
            .send()
            .unwrap();
        let mint_b = CreateMint::new(&mut program, &maker)
            .decimals(6)
            .authority(&maker_address)
            .send()
            .unwrap();

        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &maker, &mint_a)
            .owner(&maker_address)
            .send()
            .unwrap();

        let taker_ata_b = CreateAssociatedTokenAccount::new(&mut program, &taker, &mint_b)
            .owner(&taker_address)
            .send()
            .unwrap();

        let (escrow, _bump) = Address::find_program_address(
            &[b"escrow", maker_address.as_ref(), &123u64.to_le_bytes()],
            &program_id,
        );
        let vault = get_associated_token_address(&escrow, &mint_a);

        MintTo::new(&mut program, &maker, &mint_a, &maker_ata_a, 1_000_000_000)
            .send()
            .unwrap();
        log_token_account(&program, "maker_ata_a (after mint)", &maker_ata_a);

        let make_ix = Instruction::new_with_bytes(
            program_id,
            &anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
            vec![
                AccountMeta::new(maker_address, true),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(maker_ata_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(Address::from(ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes()), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(Address::from(anchor_lang::system_program::ID.to_bytes()), false),
            ],
        );
        let message = Message::new(&[make_ix], Some(&maker_address));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&maker], message, recent_blockhash);
        let make_tx = program.send_transaction(transaction).unwrap();
        msg!("[test_take] make tx ok sig={} cu={}", make_tx.signature, make_tx.compute_units_consumed);
        log_token_account(&program, "vault (after make)", &vault);

        MintTo::new(&mut program, &maker, &mint_b, &taker_ata_b, 10)
            .send()
            .unwrap();
        log_token_account(&program, "taker_ata_b (after mint)", &taker_ata_b);

        let taker_ata_a = get_associated_token_address(&taker_address, &mint_a);
        let maker_ata_b = get_associated_token_address(&maker_address, &mint_b);

        let take_ix = Instruction::new_with_bytes(
            program_id,
            &anchor_escrow_q2_2026::instruction::Take {}.data(),
            vec![
                AccountMeta::new(taker_address, true),
                AccountMeta::new(maker_address, false),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(taker_ata_a, false),
                AccountMeta::new(taker_ata_b, false),
                AccountMeta::new(maker_ata_b, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(Address::from(ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes()), false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(Address::from(anchor_lang::system_program::ID.to_bytes()), false),
            ],
        );

        let message = Message::new(&[take_ix], Some(&taker_address));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&taker], message, recent_blockhash);
        let take_tx = program.send_transaction(transaction).unwrap();
        msg!("[test_take] take tx ok sig={} cu={}", take_tx.signature, take_tx.compute_units_consumed);

        log_account(&program, "escrow (after take)", &escrow);
        log_account(&program, "vault (after take)", &vault);
        assert!(program.get_account(&vault).is_none());
        assert!(program.get_account(&escrow).is_none());

        let taker_ata_a_account = program.get_account(&taker_ata_a).unwrap();
        let taker_ata_a_data = spl_token::state::Account::unpack(&taker_ata_a_account.data).unwrap();
        assert_eq!(taker_ata_a_data.amount, 10);
        assert_eq!(taker_ata_a_data.owner, taker_address);
        assert_eq!(taker_ata_a_data.mint, mint_a);

        let taker_ata_b_account = program.get_account(&taker_ata_b).unwrap();
        let taker_ata_b_data = spl_token::state::Account::unpack(&taker_ata_b_account.data).unwrap();
        assert_eq!(taker_ata_b_data.amount, 0);

        let maker_ata_b_account = program.get_account(&maker_ata_b).unwrap();
        let maker_ata_b_data = spl_token::state::Account::unpack(&maker_ata_b_account.data).unwrap();
        assert_eq!(maker_ata_b_data.amount, 10);
        assert_eq!(maker_ata_b_data.owner, maker_address);
        assert_eq!(maker_ata_b_data.mint, mint_b);
        msg!(
            "[test_take] balances: taker_ata_a={} taker_ata_b={} maker_ata_b={}",
            taker_ata_a_data.amount,
            taker_ata_b_data.amount,
            maker_ata_b_data.amount
        );
        msg!("[test_take] done\n\n");
    }

    #[test]
    fn test_refund() {
        let (mut program, payer) = setup();
        let program_id = Address::from(anchor_escrow_q2_2026::id().to_bytes());

        msg!("\n\n[test_refund] start");

        let maker = payer.pubkey();
        msg!("[test_refund] maker={}", maker);

        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();
        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6)
            .authority(&maker)
            .send()
            .unwrap();

        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker)
            .send()
            .unwrap();

        let initial_maker_ata_a_amount = 1_000_000_000u64;
        MintTo::new(
            &mut program,
            &payer,
            &mint_a,
            &maker_ata_a,
            initial_maker_ata_a_amount,
        )
        .send()
        .unwrap();
        log_token_account(&program, "maker_ata_a (after mint)", &maker_ata_a);

        let (escrow, _bump) = Address::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()],
            &program_id,
        );
        let vault = get_associated_token_address(&escrow, &mint_a);

        let make_ix = Instruction::new_with_bytes(
            program_id,
            &anchor_escrow_q2_2026::instruction::Make {
                deposit: 10,
                seed: 123u64,
                receive: 10,
            }
            .data(),
            vec![
                AccountMeta::new(maker, true),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new_readonly(mint_b, false),
                AccountMeta::new(maker_ata_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(
                    Address::from(ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes()),
                    false,
                ),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(
                    Address::from(anchor_lang::system_program::ID.to_bytes()),
                    false,
                ),
            ],
        );
        let message = Message::new(&[make_ix], Some(&maker));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&payer], message, recent_blockhash);
        let make_tx = program.send_transaction(transaction).unwrap();
        msg!("[test_refund] make tx ok sig={} cu={}", make_tx.signature, make_tx.compute_units_consumed);
        log_token_account(&program, "vault (after make)", &vault);

        let maker_ata_a_after_make_account = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_after_make_data =
            spl_token::state::Account::unpack(&maker_ata_a_after_make_account.data).unwrap();
        msg!("[test_refund] maker_ata_a after make amount={}", maker_ata_a_after_make_data.amount);
        assert_eq!(maker_ata_a_after_make_data.amount, initial_maker_ata_a_amount - 10);

        let refund_ix = Instruction::new_with_bytes(
            program_id,
            &anchor_escrow_q2_2026::instruction::Refund {}.data(),
            vec![
                AccountMeta::new(maker, true),
                AccountMeta::new_readonly(mint_a, false),
                AccountMeta::new(maker_ata_a, false),
                AccountMeta::new(escrow, false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(
                    Address::from(anchor_lang::system_program::ID.to_bytes()),
                    false,
                ),
            ],
        );
        let message = Message::new(&[refund_ix], Some(&maker));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&payer], message, recent_blockhash);
        let refund_tx = program.send_transaction(transaction).unwrap();
        msg!("[test_refund] refund tx ok sig={} cu={}", refund_tx.signature, refund_tx.compute_units_consumed);

        log_account(&program, "escrow (after refund)", &escrow);
        log_account(&program, "vault (after refund)", &vault);
        assert!(program.get_account(&vault).is_none());
        assert!(program.get_account(&escrow).is_none());

        let maker_ata_a_after_refund_account = program.get_account(&maker_ata_a).unwrap();
        let maker_ata_a_after_refund_data =
            spl_token::state::Account::unpack(&maker_ata_a_after_refund_account.data).unwrap();
        msg!("[test_refund] maker_ata_a after refund amount={}", maker_ata_a_after_refund_data.amount);
        assert_eq!(maker_ata_a_after_refund_data.amount, initial_maker_ata_a_amount);

        msg!("[test_refund] done\n\n");
    }
}
