use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};
use crate::state::Pool;
use crate::error::AmmError;
use crate::math::withdraw_amount;

pub fn remove_liquidity_handler(
    ctx: Context<RemoveLiquidity>,
    lp_amount: u64,
    min_amount_a: u64,
    min_amount_b: u64,
) -> Result<()> {
    require!(lp_amount > 0, AmmError::ZeroAmount);

    let pool = &ctx.accounts.pool;
    let reserve_a = ctx.accounts.vault_a.amount;
    let reserve_b = ctx.accounts.vault_b.amount;
    let lp_supply = ctx.accounts.lp_mint.supply;

    let amount_a_out = withdraw_amount(lp_amount, reserve_a, lp_supply)?;
    let amount_b_out = withdraw_amount(lp_amount, reserve_b, lp_supply)?;

    require!(amount_a_out >= min_amount_a, AmmError::SlippageExceeded);
    require!(amount_b_out >= min_amount_b, AmmError::SlippageExceeded);
    require!(amount_a_out > 0 && amount_b_out > 0, AmmError::InsufficientLiquidity);

    
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.user_lp_token.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        lp_amount,
    )?;

    let pool_key = pool.key();
    let seeds = &[b"authority", pool_key.as_ref(), &[pool.authority_bump]];
    let signer_seeds = &[&seeds[..]];

    
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.vault_a.to_account_info(),
                to: ctx.accounts.user_token_a.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        amount_a_out,
    )?;

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.vault_b.to_account_info(),
                to: ctx.accounts.user_token_b.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        amount_b_out,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
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