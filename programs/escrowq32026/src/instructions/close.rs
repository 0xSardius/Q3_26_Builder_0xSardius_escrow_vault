use anchor_lang::prelude::*;
use anchor_spl::token_interface::{close_account, CloseAccount, TokenAccount, TokenInterface};

pub fn close_vault<'info>(
    vault: &InterfaceAccount<'info, TokenAccount>,
    destination: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let cpi_accounts = CloseAccount {
        account: vault.to_account_info(),
        destination,
        authority,
    };

    let cpi_context =
        CpiContext::new_with_signer(token_program.key(), cpi_accounts, signer_seeds);

    close_account(cpi_context)
}
