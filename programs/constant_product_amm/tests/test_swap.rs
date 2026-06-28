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

fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint_kp: &Keypair, decimals:u8){
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

fn setup_pool_with_liquidity(fee_bps: u16, deposit_a: u64, deposit_b: u64) -> (PoolSetup, Keypair, Keypair, Keypair) {
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

    let user_token_a = create_token_account_with_balance(&mut svm, &payer, &mint_a, &payer.pubkey(), 10_000_000);
    let user_token_b = create_token_account_with_balance(&mut svm, &payer, &mint_b, &payer.pubkey(), 10_000_000);
    let user_lp_token = create_token_account_with_balance(&mut svm, &payer, &lp_mint, &payer.pubkey(), 0);

    let add_liq_accounts = constant_product_amm::accounts::AddLiquidity {
        user: payer.pubkey(),
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        user_token_a: user_token_a.pubkey(),
        user_token_b: user_token_b.pubkey(),
        user_lp_token: user_lp_token.pubkey(),
        token_program: spl_token::id(),
    };
    let add_liq_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::AddLiquidity { amount_a: deposit_a, amount_b: deposit_b, min_lp_out: 1 }.data(),
        add_liq_accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[add_liq_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let setup = PoolSetup { svm, payer, mint_a, mint_b, pool, pool_authority, vault_a, vault_b, lp_mint };
    (setup, user_token_a, user_token_b, user_lp_token)
}

#[test]
fn test_swap_a_to_b_produces_expected_output() {
    let (mut s, user_token_a, user_token_b, _user_lp) = setup_pool_with_liquidity(30, 1_000_000, 1_000_000);
    let program_id = constant_product_amm::id();

    let vault_a_before = get_token_balance(&s.svm, &s.vault_a);
    let vault_b_before = get_token_balance(&s.svm, &s.vault_b);
    let user_b_before = get_token_balance(&s.svm, &user_token_b.pubkey());

    let swap_accounts = constant_product_amm::accounts::Swap {
        user: s.payer.pubkey(),
        pool: s.pool,
        pool_authority: s.pool_authority,
        vault_in: s.vault_a,
        vault_out: s.vault_b,
        user_source: user_token_a.pubkey(),
        user_destination: user_token_b.pubkey(),
        token_program: spl_token::id(),
    };
    let swap_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::Swap { amount_in: 50_000, min_amount_out: 1 }.data(),
        swap_accounts.to_account_metas(None),
    );
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[swap_ix], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    let res = s.svm.send_transaction(tx);
    assert!(res.is_ok(), "swap failed: {:?}", res);

    let vault_a_after = get_token_balance(&s.svm, &s.vault_a);
    let vault_b_after = get_token_balance(&s.svm, &s.vault_b);
    let user_b_after = get_token_balance(&s.svm, &user_token_b.pubkey());

    assert_eq!(vault_a_after, vault_a_before + 50_000);
    let received = user_b_after - user_b_before;
    assert!(received > 0 && received < 50_000, "output should be positive but less than input due to price impact + fee");
    assert_eq!(vault_b_after, vault_b_before - received);

    // k should grow slightly due to the fee
    let k_before = (vault_a_before as u128) * (vault_b_before as u128);
    let k_after = (vault_a_after as u128) * (vault_b_after as u128);
    assert!(k_after >= k_before, "k should not shrink after a swap");
}

#[test]
fn test_swap_fails_when_slippage_exceeded() {
    let (mut s, user_token_a, user_token_b, _user_lp) = setup_pool_with_liquidity(30, 1_000_000, 1_000_000);
    let program_id = constant_product_amm::id();

    let swap_accounts = constant_product_amm::accounts::Swap {
        user: s.payer.pubkey(),
        pool: s.pool,
        pool_authority: s.pool_authority,
        vault_in: s.vault_a,
        vault_out: s.vault_b,
        user_source: user_token_a.pubkey(),
        user_destination: user_token_b.pubkey(),
        token_program: spl_token::id(),
    };
    let swap_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::Swap { amount_in: 50_000, min_amount_out: 999_999_999 }.data(),
        swap_accounts.to_account_metas(None),
    );
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[swap_ix], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    let res = s.svm.send_transaction(tx);

    assert!(res.is_err(), "swap should fail when min_amount_out cannot be satisfied");
}

#[test]
fn test_swap_fails_with_wrong_vault() {
    // Security test: pass a vault that doesn't belong to this pool at all (here, just swap vault_in/vault_out
    // with the user's own token accounts pretending to be vaults) and confirm the program rejects it.
    let (mut s, user_token_a, user_token_b, _user_lp) = setup_pool_with_liquidity(30, 1_000_000, 1_000_000);
    let program_id = constant_product_amm::id();

    let fake_vault = Keypair::new();
    // an account that is NOT a real vault for this pool
    let fake_vault_account = create_token_account_with_balance(&mut s.svm, &s.payer, &s.mint_b, &s.payer.pubkey(), 0);
    let _ = fake_vault; // unused, just for clarity

    let swap_accounts = constant_product_amm::accounts::Swap {
        user: s.payer.pubkey(),
        pool: s.pool,
        pool_authority: s.pool_authority,
        vault_in: s.vault_a,
        vault_out: fake_vault_account.pubkey(), // not the pool's real vault_b
        user_source: user_token_a.pubkey(),
        user_destination: user_token_b.pubkey(),
        token_program: spl_token::id(),
    };
    let swap_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::Swap { amount_in: 50_000, min_amount_out: 1 }.data(),
        swap_accounts.to_account_metas(None),
    );
    let blockhash = s.svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[swap_ix], Some(&s.payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&s.payer]).unwrap();
    let res = s.svm.send_transaction(tx);

    assert!(res.is_err(), "swap should fail when vault_out is not the pool's real vault");
}
