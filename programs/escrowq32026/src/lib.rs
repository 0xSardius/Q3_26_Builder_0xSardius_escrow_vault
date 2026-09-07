pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("5Y6HMSgNYbkcBiQCukYvTK56aQarSpq1Nk9aiSsjws2o");

// Two parties — a maker and a taker — can swap tokens without trusting each other or a third party.
// The maker deposits token A into a program-controlled vault and specifies how much of token B they want in return.
// Any taker who holds token B can complete the swap atomically while the offer is live.
// After expiration, take and update are closed; the maker can refund.

// Maker deposits token A  →  vault (PDA-owned)
//                                       ↓  taker sends token B to maker
//                                       ↓  vault releases token A to taker
//                                       ↓  escrow + vault accounts closed, rent returned

#[program]
pub mod escrowq32026 {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn make(
        ctx: Context<Make>,
        seed: u64,
        deposit: u64,
        receive: u64,
        expiration: i64,
    ) -> Result<()> {
        ctx.accounts
            .init_escrow(seed, receive, &ctx.bumps, expiration)?;
        ctx.accounts.deposit(deposit)?;
        ctx.accounts.mint_position()
    }

    #[instruction(discriminator = 1)]
    pub fn take(ctx: Context<Take>) -> Result<()> {
        ctx.accounts.assert_live()?;
        ctx.accounts.deposit()?;
        ctx.accounts.withdraw()?;
        ctx.accounts.close_vault()
    }

    #[instruction(discriminator = 2)]
    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        ctx.accounts.assert_expired()?;
        ctx.accounts.withdraw()?;
        ctx.accounts.close_vault()
    }

    #[instruction(discriminator = 3)]
    pub fn update(ctx: Context<Update>, receive: u64) -> Result<()> {
        ctx.accounts.assert_live()?;
        ctx.accounts.update(receive)
    }

    #[instruction(discriminator = 4)]
    pub fn redeem(ctx: Context<Redeem>) -> Result<()> {
        ctx.accounts.assert_expired()?;
        ctx.accounts.burn_position()?;
        ctx.accounts.withdraw()?;
        ctx.accounts.close_vault()
    }
}