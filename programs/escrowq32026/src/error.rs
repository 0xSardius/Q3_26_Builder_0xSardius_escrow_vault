use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The escrow has expired")]
    EscrowExpired,
    #[msg("The escrow has not expired yet")]
    EscrowNotExpired,
    #[msg("Expiration must be in the future")]
    EscrowExpirationInPast,
    #[msg("Holder must own the position receipt")]
    MissingPosition,
}
