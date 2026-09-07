use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Escrow, ESCROW_SEED};

#[derive(Accounts)]
pub struct Update<'info> {
    pub maker: Signer<'info>,
    #[account(
        mut,
        has_one = maker,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

impl<'info> Update<'info> {
    pub fn assert_live(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now < self.escrow.expiration, ErrorCode::EscrowExpired);
        Ok(())
    }

    pub fn update(&mut self, receive: u64) -> Result<()> {
        self.escrow.receive = receive;
        Ok(())
    }
}
