use solana_program_pack::Pack;
use anchor_lang::solana_program::sysvar::SysvarId;
use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_lang::solana_program::{instruction::Instruction, pubkey::Pubkey, system_instruction, system_program};
use anchor_spl::token::spl_token;
use litesvm::LiteSVM;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_keypair::Keypair;
use solana_transaction::versioned::VersionedTransaction;

fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint_kp: &Keypair, decimals: u8) {
    let rent_lamports = anchor_lang::prelude::Rent::default().minimum_balance(spl_token::state::Mint::LEN);
    let ixs = vec![
        system_instruction::create_account(
            &payer.pubkey(),
            &mint_kp.pubkey(),
            rent_lamports,
            spl_token::state::Mint::LEN as u64,
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_mint(&spl_token::id(), &mint_kp.pubkey(), &payer.pubkey(), None, decimals)
            .unwrap(),
    ];
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, mint_kp]).unwrap();
    svm.send_transaction(tx).unwrap();
}

fn build_init_pool_ix(
    program_id: &Pubkey,
    payer: &Pubkey,
    mint_a: &Pubkey,
    mint_b: &Pubkey,
    fee_bps: u16,
) -> Instruction {
    let (pool, _) = Pubkey::find_program_address(&[b"pool", mint_a.as_ref(), mint_b.as_ref()], program_id);
    let (pool_authority, _) = Pubkey::find_program_address(&[b"authority", pool.as_ref()], program_id);
    let (vault_a, _) = Pubkey::find_program_address(&[b"vault_a", pool.as_ref()], program_id);
    let (vault_b, _) = Pubkey::find_program_address(&[b"vault_b", pool.as_ref()], program_id);
    let (lp_mint, _) = Pubkey::find_program_address(&[b"lp_mint", pool.as_ref()], program_id);

    let accounts = constant_product_amm::accounts::InitPool {
        payer: *payer,
        token_a_mint: *mint_a,
        token_b_mint: *mint_b,
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        token_program: spl_token::id(),
        system_program: system_program::id(),
        rent: anchor_lang::prelude::Rent::id(),
    };

    Instruction::new_with_bytes(
        *program_id,
        &constant_product_amm::instruction::InitPool { fee_bps }.data(),
        accounts.to_account_metas(None),
    )
}

#[test]
fn test_cannot_reinitialize_existing_pool() {
    let program_id = constant_product_amm::id();
    let payer = Keypair::new();
    let mint_a_kp = Keypair::new();
    let mint_b_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 5_000_000_000).unwrap();

    create_mint(&mut svm, &payer, &mint_a_kp, 6);
    create_mint(&mut svm, &payer, &mint_b_kp, 6);

    let (mint_a, mint_b) = {
        let a = mint_a_kp.pubkey();
        let b = mint_b_kp.pubkey();
        if a < b { (a, b) } else { (b, a) }
    };

    let ix1 = build_init_pool_ix(&program_id, &payer.pubkey(), &mint_a, &mint_b, 30);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res1 = svm.send_transaction(tx);
    assert!(res1.is_ok(), "first init_pool should succeed: {:?}", res1);

    let ix2 = build_init_pool_ix(&program_id, &payer.pubkey(), &mint_a, &mint_b, 30);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res2 = svm.send_transaction(tx);

    assert!(
        res2.is_err(),
        "reinitializing an existing pool should fail, but it succeeded: {:?}",
        res2
    );
}