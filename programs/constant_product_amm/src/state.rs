use anchor_lang::prelude::*;

#[account]
pub struct Pool {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub lp_mint: Pubkey,
    pub fee_bps: u16,
    pub bump: u8,
    pub authority_bump: u8,
}

impl Pool{
    pub const LEN: usize = 8+32+32+32+32+32+2+1+1;
}