use {
    anchor_lang::{
        solana_program::{instruction::Instruction, pubkey::Pubkey, system_instruction, rent::Rent, system_program, sysvar::SysvarId},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token,
    litesvm::LiteSVM,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_keypair::Keypair,
    solana_transaction::versioned::VersionedTransaction,
};
// creating token
fn create_mint_instructions(
    payer: &Pubkey,
    mint: &Pubkey,
    decimals: u8,
    rent_lamports: u64,
) -> Vec<Instruction> {
    vec![
        system_instruction::create_account(
            payer,
            mint,
            rent_lamports,
            82, // standard size of an SPL Mint account
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_mint(
            &spl_token::id(),
            mint,
            payer, 
            None,
            decimals,
        )
        .unwrap(),
    ]
}

#[test]
fn test_init_pool_fails_with_same_mint() {
    let program_id = constant_product_amm::id();
    let payer = Keypair::new();
    let mint_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();

    let rent_lamports = Rent::default().minimum_balance(82);
    let ixs = create_mint_instructions(&payer.pubkey(), &mint_kp.pubkey(), 6, rent_lamports);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &mint_kp]).unwrap();
    svm.send_transaction(tx).unwrap();

    let mint = mint_kp.pubkey();
    let (pool, _) = Pubkey::find_program_address(&[b"pool", mint.as_ref(), mint.as_ref()], &program_id);
    let (pool_authority, _) = Pubkey::find_program_address(&[b"authority", pool.as_ref()], &program_id);
    let (vault_a, _) = Pubkey::find_program_address(&[b"vault_a", pool.as_ref()], &program_id);
    let (vault_b, _) = Pubkey::find_program_address(&[b"vault_b", pool.as_ref()], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"lp_mint", pool.as_ref()], &program_id);

    let accounts = constant_product_amm::accounts::InitPool {
        payer: payer.pubkey(),
        token_a_mint: mint,
        token_b_mint: mint,
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        token_program: spl_token::id(),
        system_program: system_program::id(),
        rent: anchor_lang::prelude::Rent::id(),
    };

    let ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::InitPool { fee_bps: 30 }.data(),
        accounts.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "init_pool should fail when both mints are the same");
}

#[test]
fn test_init_pool_fails_with_invalid_fee() {
    let program_id = constant_product_amm::id();
    let payer = Keypair::new();
    let mint_a_kp = Keypair::new();
    let mint_b_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();

    let rent_lamports = Rent::default().minimum_balance(82);
    for mint_kp in [&mint_a_kp, &mint_b_kp] {
        let ixs = create_mint_instructions(&payer.pubkey(), &mint_kp.pubkey(), 6, rent_lamports);
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, mint_kp]).unwrap();
        svm.send_transaction(tx).unwrap();
    }

    let (mint_a, mint_b) = {
        let a = mint_a_kp.pubkey();
        let b = mint_b_kp.pubkey();
        if a < b { (a, b) } else { (b, a) }
    };

    let (pool, _) = Pubkey::find_program_address(&[b"pool", mint_a.as_ref(), mint_b.as_ref()], &program_id);
    let (pool_authority, _) = Pubkey::find_program_address(&[b"authority", pool.as_ref()], &program_id);
    let (vault_a, _) = Pubkey::find_program_address(&[b"vault_a", pool.as_ref()], &program_id);
    let (vault_b, _) = Pubkey::find_program_address(&[b"vault_b", pool.as_ref()], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"lp_mint", pool.as_ref()], &program_id);

    let accounts = constant_product_amm::accounts::InitPool {
        payer: payer.pubkey(),
        token_a_mint: mint_a,
        token_b_mint: mint_b,
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        token_program: spl_token::id(),
        system_program: system_program::id(),
        rent: anchor_lang::prelude::Rent::id(),
    };

    let ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::InitPool { fee_bps: 10_001 }.data(),
        accounts.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_err(), "init_pool should fail when fee > 10000");
}

#[test]
fn test_init_pool_creates_pool_correctly() {
    let program_id = constant_product_amm::id();
    let payer = Keypair::new();
    let mint_a_kp = Keypair::new();
    let mint_b_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();

    let rent_lamports = Rent::default().minimum_balance(82);

    // Create the two token mints first via two separate transactions
    for mint_kp in [&mint_a_kp, &mint_b_kp] {
        let ixs = create_mint_instructions(&payer.pubkey(), &mint_kp.pubkey(), 6, rent_lamports);
        let blockhash = svm.latest_blockhash();
        let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
        let tx = VersionedTransaction::try_new(
            VersionedMessage::Legacy(msg),
            &[&payer, mint_kp],
        )
        .unwrap();
        svm.send_transaction(tx).unwrap();
    }

    // Sort mint pubkeys the same way our seed scheme expects (smaller pubkey first)
    let (mint_a, mint_b) = {
        let a = mint_a_kp.pubkey();
        let b = mint_b_kp.pubkey();
        if a < b { (a, b) } else { (b, a) }
    };

    // Derive every PDA exactly the way the program does
    let (pool, _pool_bump) = Pubkey::find_program_address(&[b"pool", mint_a.as_ref(), mint_b.as_ref()], &program_id);
    let (pool_authority, _auth_bump) = Pubkey::find_program_address(&[b"authority", pool.as_ref()], &program_id);
    let (vault_a, _) = Pubkey::find_program_address(&[b"vault_a", pool.as_ref()], &program_id);
    let (vault_b, _) = Pubkey::find_program_address(&[b"vault_b", pool.as_ref()], &program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"lp_mint", pool.as_ref()], &program_id);

    let accounts = constant_product_amm::accounts::InitPool {
        payer: payer.pubkey(),
        token_a_mint: mint_a,
        token_b_mint: mint_b,
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        token_program: spl_token::id(),
        system_program: system_program::id(),
        rent: anchor_lang::prelude::Rent::id(),
    };

    let fee_bps: u16 = 30;
    let instruction = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::InitPool { fee_bps }.data(),
        accounts.to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "init_pool transaction failed: {:?}", res);

    // Fetch the Pool account back and verify its stored data matches what we expect
    let pool_account = svm.get_account(&pool).expect("pool account should exist");
    let pool_data = constant_product_amm::Pool::try_deserialize(&mut pool_account.data.as_slice())
    .expect("should deserialize Pool account");

    assert_eq!(pool_data.token_a_mint, mint_a);
    assert_eq!(pool_data.token_b_mint, mint_b);
    assert_eq!(pool_data.vault_a, vault_a);
    assert_eq!(pool_data.vault_b, vault_b);
    assert_eq!(pool_data.lp_mint, lp_mint);
    assert_eq!(pool_data.fee_bps, fee_bps);
}