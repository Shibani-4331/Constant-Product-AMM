use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Insufficient liquidity in pool")]
    InsufficientLiquidity,

    #[msg("Provided token mint does not match pool")]
    InvalidMint,

    #[msg("Provided vault does not match pool")]
    InvalidVault,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Zero amount not allowed")]
    ZeroAmount,

    #[msg("Invalid fee value")]
    InvalidFee,
}