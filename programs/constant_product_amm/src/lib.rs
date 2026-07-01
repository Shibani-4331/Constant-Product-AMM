pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod math;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("7Hr5ygzGZYj3oaQBX6SQtacuGzXJfctv6R94FkXKStSx");

#[program]
pub mod constant_product_amm {
    use super::*;

    pub fn init_pool(ctx: Context<InitPool>, fee_bps: u16) -> Result<()> {
        instructions::init_pool::init_pool_handler(ctx, fee_bps)
    }

    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_a: u64, amount_b: u64, min_lp_out: u64) -> Result<()> {
        instructions::add_liquidity::add_liquidity_handler(ctx, amount_a, amount_b, min_lp_out)
    }

    pub fn swap(ctx: Context<Swap>, amount_in: u64, min_amount_out: u64) -> Result<()> {
        instructions::swap::swap_handler(ctx, amount_in, min_amount_out)
    }
    
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64, min_amount_a: u64, min_amount_b: u64) -> Result<()> {
        instructions::remove_liquidity::remove_liquidity_handler(ctx, lp_amount, min_amount_a, min_amount_b)
    }
}