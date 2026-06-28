use solana_program_pack::Pack;
use {
    anchor_lang::{
        solana_program::{instruction::Instruction, pubkey::Pubkey, system_instruction, system_program, sysvar::SysvarId},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::token::spl_token,
    litesvm::LiteSVM,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_keypair::Keypair,
    solana_transaction::versioned::VersionedTransaction,
};
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

fn create_token_account_with_balance(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Keypair {
    let token_account_kp = Keypair::new();
    let rent_lamports = anchor_lang::prelude::Rent::default().minimum_balance(spl_token::state::Account::LEN);

    let mut ixs = vec![
        system_instruction::create_account(
            &payer.pubkey(),
            &token_account_kp.pubkey(),
            rent_lamports,
            spl_token::state::Account::LEN as u64,
            &spl_token::id(),
        ),
        spl_token::instruction::initialize_account(&spl_token::id(), &token_account_kp.pubkey(), mint, owner)
            .unwrap(),
    ];

    if amount > 0 {
        ixs.push(
            spl_token::instruction::mint_to(
                &spl_token::id(),
                mint,
                &token_account_kp.pubkey(),
                &payer.pubkey(),
                &[],
                amount,
            )
            .unwrap(),
        );
    }

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer, &token_account_kp]).unwrap();
    svm.send_transaction(tx).unwrap();

    token_account_kp
}

fn get_token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm.get_account(token_account).unwrap();
    let parsed = spl_token::state::Account::unpack(&account.data).unwrap();
    parsed.amount
}

struct PoolSetup {
    svm: LiteSVM,
    payer: Keypair,
    mint_a: Pubkey,
    mint_b: Pubkey,
    pool: Pubkey,
    pool_authority: Pubkey,
    vault_a: Pubkey,
    vault_b: Pubkey,
    lp_mint: Pubkey,
}

fn setup_pool(fee_bps: u16) -> PoolSetup {
    let program_id = constant_product_amm::id();
    let payer = Keypair::new();
    let mint_a_kp = Keypair::new();
    let mint_b_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    create_mint(&mut svm, &payer, &mint_a_kp, 6);
    create_mint(&mut svm, &payer, &mint_b_kp, 6);

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

    let init_accounts = constant_product_amm::accounts::InitPool {
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
    let init_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::InitPool { fee_bps }.data(),
        init_accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[init_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    PoolSetup { svm, payer, mint_a, mint_b, pool, pool_authority, vault_a, vault_b, lp_mint }
}

#[test]
fn test_first_deposit_mints_sqrt_lp_amount() {
    let mut s = setup_pool(30);
    let program_id = constant_product_amm::id();

    let user_token_a = create_token_account_with_balance(&mut s.svm, &s.payer, &s.mint_a, &s.payer.pubkey(), 1_000_000);
    let user_token_b = create_token_account_with_balance(&mut s.svm, &s.payer, &s.mint_b, &s.payer.pubkey(), 1_000_000);
    let user_lp_token = create_token_account_with_balance(&mut s.svm, &s.payer, &s.lp_mint, &s.payer.pubkey(), 0);

    let add_liq_accounts = constant_product_amm::accounts::AddLiquidity {
        user: s.payer.pubkey(),
        pool: s.pool,
        pool_authority: s.pool_authority,
        vault_a: s.vault_a,
        vault_b: s.vault_b,
        lp_mint: s.lp_mint,
        user_token_a: user_token_a.pubkey(),
        user_token_b: user_token_b.pubkey(),
        user_lp_token: user_lp_token.pubkey(),
        token_program: spl_token::id(),
    };
    let ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::AddLiquidity { amount_a: 100_000, amount_b: 400_000, min_lp_out: 1 }.data(),
        add_liq_accounts.to_account_metas(None),
    );
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    let res = s.svm.send_transaction(tx);
    assert!(res.is_ok(), "add_liquidity failed: {:?}", res);

    let lp_balance = get_token_balance(&s.svm, &user_lp_token.pubkey());
    // sqrt(100_000 * 400_000) = 200_000
    assert_eq!(lp_balance, 200_000);

    assert_eq!(get_token_balance(&s.svm, &s.vault_a), 100_000);
    assert_eq!(get_token_balance(&s.svm, &s.vault_b), 400_000);
}

#[test]
fn test_second_deposit_is_proportional() {
    let mut s = setup_pool(30);
    let program_id = constant_product_amm::id();

    let user_token_a = create_token_account_with_balance(&mut s.svm, &s.payer, &s.mint_a, &s.payer.pubkey(), 1_000_000);
    let user_token_b = create_token_account_with_balance(&mut s.svm, &s.payer, &s.mint_b, &s.payer.pubkey(), 1_000_000);
    let user_lp_token = create_token_account_with_balance(&mut s.svm, &s.payer, &s.lp_mint, &s.payer.pubkey(), 0);

    let make_add_liq_ix = |amount_a: u64, amount_b: u64, min_lp_out: u64| {
        let accounts = constant_product_amm::accounts::AddLiquidity {
            user: s.payer.pubkey(),
            pool: s.pool,
            pool_authority: s.pool_authority,
            vault_a: s.vault_a,
            vault_b: s.vault_b,
            lp_mint: s.lp_mint,
            user_token_a: user_token_a.pubkey(),
            user_token_b: user_token_b.pubkey(),
            user_lp_token: user_lp_token.pubkey(),
            token_program: spl_token::id(),
        };
        Instruction::new_with_bytes(
            program_id,
            &constant_product_amm::instruction::AddLiquidity { amount_a, amount_b, min_lp_out }.data(),
            accounts.to_account_metas(None),
        )
    };

    let ix1 = make_add_liq_ix(100_000, 400_000, 1);
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    s.svm.send_transaction(tx).unwrap();

    let ix2 = make_add_liq_ix(10_000, 40_000, 1);
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    let res = s.svm.send_transaction(tx);
    assert!(res.is_ok(), "second add_liquidity failed: {:?}", res);

    let lp_balance = get_token_balance(&s.svm, &user_lp_token.pubkey());
    assert_eq!(lp_balance, 220_000); // 200_000 + 20_000

    assert_eq!(get_token_balance(&s.svm, &s.vault_a), 110_000);
    assert_eq!(get_token_balance(&s.svm, &s.vault_b), 440_000);
}