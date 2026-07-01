use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::Pool;
use crate::error::AmmError;
use crate::math::swap_output_amount;

pub fn swap_handler(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64 ) -> Result<()> {
    require!(amount_in > 0, AmmError::ZeroAmount);

    let pool = &ctx.accounts.pool;

    let vault_in_key = ctx.accounts.vault_in.key();
    let vault_out_key = ctx.accounts.vault_out.key();

    let is_a_to_b = vault_in_key == pool.vault_a && vault_out_key == pool.vault_b;
    let is_b_to_a = vault_in_key == pool.vault_b && vault_out_key == pool.vault_a;
    require!(is_a_to_b || is_b_to_a, AmmError::InvalidVault);

    let reserve_in = ctx.accounts.vault_in.amount;
    let reserve_out = ctx.accounts.vault_out.amount;

    let amount_out = swap_output_amount(amount_in, reserve_in, reserve_out, pool.fee_bps)?;

    require!(amount_out >= min_amount_out, AmmError::SlippageExceeded);
    require!(amount_out > 0, AmmError::InsufficientLiquidity);

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.user_source.to_account_info(),
                to: ctx.accounts.vault_in.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_in,
    )?;

    
    let pool_key = pool.key();
    let seeds = &[b"authority", pool_key.as_ref(), &[pool.authority_bump]];
    let signer_seeds = &[&seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.vault_out.to_account_info(),
                to: ctx.accounts.user_destination.to_account_info(),
                authority: ctx.accounts.pool_authority.to_account_info(),
            },
            signer_seeds,
        ),
        amount_out,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct Swap<'info> {
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
        constraint = vault_in.key() == pool.vault_a || vault_in.key() == pool.vault_b @ AmmError::InvalidVault,
    )]
    pub vault_in: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = vault_out.key() == pool.vault_a || vault_out.key() == pool.vault_b @ AmmError::InvalidVault,
    )]
    pub vault_out: Box<Account<'info, TokenAccount>>,

    #[account(mut, constraint = user_source.owner == user.key() @ AmmError::InvalidOwner)]
    pub user_source: Box<Account<'info, TokenAccount>>,

    #[account(mut, constraint = user_destination.owner == user.key() @ AmmError::InvalidOwner)]
    pub user_destination: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}