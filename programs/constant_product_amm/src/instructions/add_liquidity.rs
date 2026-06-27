use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, MintTo, Transfer};
use crate::state::Pool;
use crate::error::AmmError;
use crate::math::{initial_lp_amount, proportional_lp_amount};

pub fn add_liquidity_handler(
    ctx: Context<AddLiquidity>,
    amount_a: u64,
    amount_b: u64,
    min_lp_out: u64,
) -> Result<()> {
    require!(amount_a > 0 && amount_b > 0, AmmError::ZeroAmount);

    let pool = &ctx.accounts.pool;
    let reserve_a = ctx.accounts.vault_a.amount;
    let reserve_b = ctx.accounts.vault_b.amount;
    let lp_supply = ctx.accounts.lp_mint.supply;

    // Decide how many LP tokens to mint, and how much of each token to actually pull in
    let (used_a, used_b, lp_to_mint) = if lp_supply == 0 {
        // First-ever deposit: use the full amounts supplied, mint via sqrt formula
        let lp = initial_lp_amount(amount_a, amount_b)?;
        (amount_a, amount_b, lp)
    } else {
        // Subsequent deposit: compute LP earned per side, take the smaller (don't let user game the ratio)
        let lp_from_a = proportional_lp_amount(amount_a, reserve_a, lp_supply)?;
        let lp_from_b = proportional_lp_amount(amount_b, reserve_b, lp_supply)?;

        if lp_from_a <= lp_from_b {
            // token_a is the limiting side; recalculate matching amount_b
            let matched_b = (amount_a as u128)
                .checked_mul(reserve_b as u128)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(reserve_a as u128)
                .ok_or(AmmError::MathOverflow)? as u64;
            (amount_a, matched_b, lp_from_a)
        } else {
            let matched_a = (amount_b as u128)
                .checked_mul(reserve_a as u128)
                .ok_or(AmmError::MathOverflow)?
                .checked_div(reserve_b as u128)
                .ok_or(AmmError::MathOverflow)? as u64;
            (matched_a, amount_b, lp_from_b)
        }
    };

    require!(lp_to_mint >= min_lp_out, AmmError::SlippageExceeded);
    require!(lp_to_mint > 0, AmmError::ZeroAmount);

    // Transfer token A: user -> vault_a
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.user_token_a.to_account_info(),
                to: ctx.accounts.vault_a.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        used_a,
    )?;

    // Transfer token B: user -> vault_b
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.user_token_b.to_account_info(),
                to: ctx.accounts.vault_b.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        used_b,
    )?;

    // Mint LP tokens to user, signed by the pool authority PDA
    let pool_key = pool.key();
    let seeds = &[b"authority", pool_key.as_ref(), &[pool.authority_bump]];
    let signer_seeds = &[&seeds[..]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.user_lp_token.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        lp_to_mint,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"pool", pool.token_a_mint.as_ref(), pool.token_b_mint.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// CHECK: signing authority only, no data
    #[account(
        seeds = [b"authority", pool.key().as_ref()],
        bump = pool.authority_bump,
    )]
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = vault_a.key() == pool.vault_a @ AmmError::InvalidVault,
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = vault_b.key() == pool.vault_b @ AmmError::InvalidVault,
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = lp_mint.key() == pool.lp_mint @ AmmError::InvalidMint,
    )]
    pub lp_mint: Box<Account<'info, Mint>>,

    #[account(mut, constraint = user_token_a.mint == pool.token_a_mint @ AmmError::InvalidMint)]
    pub user_token_a: Box<Account<'info, TokenAccount>>,

    #[account(mut, constraint = user_token_b.mint == pool.token_b_mint @ AmmError::InvalidMint)]
    pub user_token_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, constraint = user_lp_token.mint == pool.lp_mint @ AmmError::InvalidMint)]
    pub user_lp_token: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}