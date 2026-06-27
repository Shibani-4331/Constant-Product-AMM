use anchor_lang::prelude::*;
use crate::error::AmmError;

// Used only for the first deposit 
pub fn initial_lp_amount(amount_a: u64, amount_b: u64) -> Result<u64> {
    let product = (amount_a as u128)
        .checked_mul(amount_b as u128)
        .ok_or(AmmError::MathOverflow)?;

    let lp = isqrt(product);
    Ok(lp as u64)
}

fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// For deposits into a pool that already has liquidity.
pub fn proportional_lp_amount(deposit_amount: u64, existing_pool_amount: u64, existing_lp_supply: u64) -> Result<u64> {
    require!(existing_pool_amount > 0, AmmError::InsufficientLiquidity);

    let numerator = (deposit_amount as u128)
        .checked_mul(existing_lp_supply as u128)
        .ok_or(AmmError::MathOverflow)?;

    let result = numerator
        .checked_div(existing_pool_amount as u128)
        .ok_or(AmmError::MathOverflow)?;

    Ok(result as u64)
}

// Computes how much of the output token a user receives for a given input,
pub fn swap_output_amount(amount_in: u64, reserve_in: u64, reserve_out: u64, fee_bps: u16) -> Result<u64> {
    
    require!(amount_in > 0, AmmError::ZeroAmount);
    require!(reserve_in > 0 && reserve_out > 0, AmmError::InsufficientLiquidity);

    let fee_amount = (amount_in as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(AmmError::MathOverflow)?
        .checked_div(10_000)
        .ok_or(AmmError::MathOverflow)?;

    let amount_in_after_fee = (amount_in as u128)
        .checked_sub(fee_amount)
        .ok_or(AmmError::MathOverflow)?;

    let k = (reserve_in as u128)
        .checked_mul(reserve_out as u128)
        .ok_or(AmmError::MathOverflow)?;

    let new_reserve_in = (reserve_in as u128)
        .checked_add(amount_in_after_fee)
        .ok_or(AmmError::MathOverflow)?;

    let new_reserve_out = k
        .checked_div(new_reserve_in)
        .ok_or(AmmError::MathOverflow)?;

    let amount_out = (reserve_out as u128)
        .checked_sub(new_reserve_out)
        .ok_or(AmmError::MathOverflow)?;

    Ok(amount_out as u64)
}

// For removing liquidity: given how many LP tokens are being burned
pub fn withdraw_amount(lp_amount: u64, reserve_amount: u64, total_lp_supply: u64) -> Result<u64> {

    require!(total_lp_supply > 0, AmmError::InsufficientLiquidity);
    require!(lp_amount > 0, AmmError::ZeroAmount);

    let numerator = (lp_amount as u128)
        .checked_mul(reserve_amount as u128)
        .ok_or(AmmError::MathOverflow)?;

    let result = numerator
        .checked_div(total_lp_supply as u128)
        .ok_or(AmmError::MathOverflow)?;

    Ok(result as u64)
}