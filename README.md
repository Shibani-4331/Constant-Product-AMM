# Constant-Product AMM

A constant-product automated market maker on Solana built with Anchor, in the spirit of Uniswap v2.

## Architecture

### Account Model

- **Pool** (PDA): token mints, vault addresses, LP mint, fee in bps, bumps
- **Vault A / Vault B** (PDAs): SPL token accounts owned by pool authority
- **LP Mint** (PDA): SPL mint issued to liquidity providers on deposit

### PDA Seeds

| Account | Seeds |
|---------|-------|
| `pool` | `["pool", mint_a, mint_b]` |
| `pool_authority` | `["authority", pool]` |
| `vault_a` | `["vault_a", pool]` |
| `vault_b` | `["vault_b", pool]` |
| `lp_mint` | `["lp_mint", pool]` |

## Instructions

| Instruction | Args | Description |
|-------------|------|-------------|
| `init_pool` | `fee_bps: u16` | Creates a new pool, token vaults, and LP mint |
| `add_liquidity` | `amount_a, amount_b, min_lp_out` | Deposits tokens into the pool, mints LP shares |
| `swap` | `amount_in, min_amount_out` | Swaps one token for the other against x \* y = k |
| `remove_liquidity` | `lp_amount, min_amount_a, min_amount_b` | Burns LP tokens, withdraws proportional share of reserves |

## Math

- **Invariant**: `x * y >= k` — k increases only through swap fees, never decreases
- **First deposit**: `LP = sqrt(amount_a * amount_b)`
- **Subsequent deposits**: `LP = min(lp_from_a, lp_from_b)` with proportional token matching
- **Swap output**: `amount_out = (reserve_out * amount_in_after_fee) / (reserve_in + amount_in_after_fee)` — integer division rounds down, favoring the pool
- **Fee**: `fee_bps / 10_000` is deducted from input, stays in pool as LP revenue
- **Withdraw**: `amount_out = (lp_amount * reserve) / total_lp_supply`

All arithmetic uses checked `u128` operations.

## Security

- Reinitialization protection via Anchor `init` constraint
- PDA seed verification on all program-owned accounts
- Signer checks on all user operations
- Slippage protection via `min_amount_out` / `min_lp_out` / `min_amount_a` / `min_amount_b`
- Swap vault direction validated against pool state
- Swap rounding favors the pool, not the swapper
- First-depositor inflation attack tested and prevented

## Tests

Run all tests:

```
cargo test
```

| Test | File |
|------|------|
| Pool initialization | `test_init_pool` |
| First deposit mints sqrt LP | `test_add_liquidity` |
| Second deposit is proportional | `test_add_liquidity` |
| Swap A→B with expected output | `test_swap` |
| Swap fails on slippage | `test_swap` |
| Swap fails with wrong vault | `test_swap` |
| Remove liquidity returns proportional share | `test_remove_liquidity` |
| Remove liquidity fails on slippage | `test_remove_liquidity` |
| Cannot reinitialize pool | `test_security_reinit` |
| First-depositor inflation attack | `test_first_depositor` |
| Math: initial LP, proportional LP, swap, withdraw | `test_math` |

## Build

```
anchor build
```

