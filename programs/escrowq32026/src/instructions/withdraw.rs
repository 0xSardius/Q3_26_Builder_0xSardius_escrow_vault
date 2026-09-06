use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub fn withdraw_from_vault<'info>(
    vault: &InterfaceAccount<'info, TokenAccount>,
    destination: &InterfaceAccount<'info, TokenAccount>,
    mint: &InterfaceAccount<'info, Mint>,
    authority: AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let cpi_accounts = TransferChecked {
        from: vault.to_account_info(),
        to: destination.to_account_info(),
        mint: mint.to_account_info(),
        authority,
    };

    let cpi_context =
        CpiContext::new_with_signer(token_program.key(), cpi_accounts, signer_seeds);

    transfer_checked(cpi_context, vault.amount, mint.decimals)
}
