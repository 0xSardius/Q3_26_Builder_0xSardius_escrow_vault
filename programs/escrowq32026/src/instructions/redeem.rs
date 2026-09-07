use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{burn, Burn, Mint, TokenAccount, TokenInterface},
};

use crate::{
    error::ErrorCode,
    instructions::{close_vault as close_token_vault, withdraw_from_vault},
    state::Escrow,
    ESCROW_SEED, POSITION_SEED,
};

#[derive(Accounts)]
pub struct Redeem<'info> {
    #[account(mut)]
    pub holder: Signer<'info>,
    #[account(mut)]
    pub maker: SystemAccount<'info>,
    #[account(mint::token_program = token_program)]
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut,
        seeds = [POSITION_SEED, escrow.key().as_ref()],
        bump,
        address = escrow.position_mint
    )]
    pub position_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = position_mint,
        associated_token::authority = holder,
        associated_token::token_program = token_program,
        constraint = holder_position_ata.amount == 1 @ ErrorCode::MissingPosition
    )]
    pub holder_position_ata: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = holder,
        associated_token::mint = mint_a,
        associated_token::authority = holder,
        associated_token::token_program = token_program
    )]
    pub holder_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        close = maker,
        has_one = maker,
        has_one = mint_a,
        seeds = [ESCROW_SEED, maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
    )]
    pub escrow: Box<Account<'info, Escrow>>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Redeem<'info> {
    pub fn assert_expired(&self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= self.escrow.expiration, ErrorCode::EscrowNotExpired);
        Ok(())
    }

    pub fn burn_position(&self) -> Result<()> {
        let cpi_accounts = Burn {
            mint: self.position_mint.to_account_info(),
            from: self.holder_position_ata.to_account_info(),
            authority: self.holder.to_account_info(),
        };

        burn(
            CpiContext::new(self.token_program.key(), cpi_accounts),
            1,
        )
    }

    pub fn withdraw(&mut self) -> Result<()> {
        let signer_seeds: [&[&[u8]]; 1] = [&[
            ESCROW_SEED,
            self.maker.key.as_ref(),
            &self.escrow.seed.to_le_bytes()[..],
            &[self.escrow.bump],
        ]];

        withdraw_from_vault(
            &self.vault,
            &self.holder_ata_a,
            &self.mint_a,
            self.escrow.to_account_info(),
            &self.token_program,
            &signer_seeds,
        )
    }

    pub fn close_vault(&mut self) -> Result<()> {
        let signer_seeds: [&[&[u8]]; 1] = [&[
            ESCROW_SEED,
            self.maker.key.as_ref(),
            &self.escrow.seed.to_le_bytes()[..],
            &[self.escrow.bump],
        ]];

        close_token_vault(
            &self.vault,
            self.holder.to_account_info(),
            self.escrow.to_account_info(),
            &self.token_program,
            &signer_seeds,
        )
    }
}
