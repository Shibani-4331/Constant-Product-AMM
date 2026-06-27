pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod math;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("5PggfhjuVtYnC8KoAKJKchVXDWssRTvLnsHb3WHzGygQ");

#[program]
pub mod constant_product_amm {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }
}
