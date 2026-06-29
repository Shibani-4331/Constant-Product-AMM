use constant_product_amm::math::{initial_lp_amount, proportional_lp_amount, swap_output_amount, withdraw_amount};
use constant_product_amm::error::AmmError;

#[test]
fn test_initial_lp_sqrt_basic() {
    let lp = initial_lp_amount(100, 400).unwrap();
    assert_eq!(lp, 200);
}

#[test]
fn test_initial_lp_equal_amounts() {
    let lp = initial_lp_amount(1_000_000, 1_000_000).unwrap();
    assert_eq!(lp, 1_000_000);
}

#[test]
fn test_initial_lp_smallest() {
    let lp = initial_lp_amount(1, 1).unwrap();
    assert_eq!(lp, 1);
}

#[test]
fn test_initial_lp_zero() {
    let lp = initial_lp_amount(0, 100).unwrap();
    assert_eq!(lp, 0);
}

#[test]
fn test_proportional_lp_basic() {
    let lp = proportional_lp_amount(100, 1000, 5000).unwrap();
    assert_eq!(lp, 500);
}

#[test]
fn test_proportional_lp_no_liquidity() {
    let err = proportional_lp_amount(100, 0, 5000).unwrap_err();
    assert_eq!(err, AmmError::InsufficientLiquidity.into());
}

#[test]
fn test_proportional_lp_exact() {
    let lp = proportional_lp_amount(50, 100, 200).unwrap();
    assert_eq!(lp, 100);
}

#[test]
fn test_swap_pool_favored_rounding() {
    let out = swap_output_amount(100, 1000, 2000, 0).unwrap();
    assert_eq!(out, 181);
}

#[test]
fn test_swap_with_fee() {
    let out = swap_output_amount(100, 1000, 2000, 100).unwrap();
    assert_eq!(out, 180);
}

#[test]
fn test_swap_zero_input() {
    let err = swap_output_amount(0, 1000, 2000, 30).unwrap_err();
    assert_eq!(err, AmmError::ZeroAmount.into());
}

#[test]
fn test_swap_no_liquidity_in() {
    let err = swap_output_amount(100, 0, 2000, 30).unwrap_err();
    assert_eq!(err, AmmError::InsufficientLiquidity.into());
}

#[test]
fn test_swap_no_liquidity_out() {
    let err = swap_output_amount(100, 1000, 0, 30).unwrap_err();
    assert_eq!(err, AmmError::InsufficientLiquidity.into());
}

#[test]
fn test_withdraw_basic() {
    let out = withdraw_amount(100, 1000, 500).unwrap();
    assert_eq!(out, 200);
}

#[test]
fn test_withdraw_zero_lp() {
    let err = withdraw_amount(0, 1000, 500).unwrap_err();
    assert_eq!(err, AmmError::ZeroAmount.into());
}

#[test]
fn test_withdraw_no_supply() {
    let err = withdraw_amount(100, 1000, 0).unwrap_err();
    assert_eq!(err, AmmError::InsufficientLiquidity.into());
}