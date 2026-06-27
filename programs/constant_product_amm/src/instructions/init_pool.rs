use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::state::Pool;
use crate::error::AmmError;

pub fn init_pool_handler(ctx: Context<InitPool>, fee_bps: u16) -> Result<()> {
    require!(fee_bps <= 10_000, AmmError::InvalidFee);

    let pool = &mut ctx.accounts.pool;
    pool.token_a_mint = ctx.accounts.token_a_mint.key();
    pool.token_b_mint = ctx.accounts.token_b_mint.key();
    pool.vault_a = ctx.accounts.vault_a.key();
    pool.vault_b = ctx.accounts.vault_b.key();
    pool.lp_mint = ctx.accounts.lp_mint.key();
    pool.fee_bps = fee_bps;
    pool.bump = ctx.bumps.pool;
    pool.authority_bump = ctx.bumps.pool_authority;

    Ok(())
}

#[derive(Accounts)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub token_a_mint: Account<'info, Mint>,
    pub token_b_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        space = Pool::LEN,
        seeds = [b"pool", token_a_mint.key().as_ref(), token_b_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    /// CHECK: PDA used only as a signing authority, holds no data itself
    #[account(
        seeds = [b"authority", pool.key().as_ref()],
        bump
    )]
    pub pool_authority: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        seeds = [b"vault_a", pool.key().as_ref()],
        bump,
        token::mint = token_a_mint,
        token::authority = pool_authority,
    )]
    pub vault_a: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"vault_b", pool.key().as_ref()],
        bump,
        token::mint = token_b_mint,
        token::authority = pool_authority,
    )]
    pub vault_b: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = payer,
        seeds = [b"lp_mint", pool.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = pool_authority,
    )]
    pub lp_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}