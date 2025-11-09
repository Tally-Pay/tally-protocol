use crate::errors::RecurringPaymentError;
use crate::state::Config;
use anchor_lang::prelude::*;

/// Arguments for accepting authority transfer
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct AcceptAuthorityArgs {
    // No arguments needed - signer validation is sufficient
}

/// Accounts required for accepting authority transfer
#[derive(Accounts)]
pub struct AcceptAuthority<'info> {
    /// Global configuration account
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    /// New authority accepting the transfer (must sign)
    pub new_authority: Signer<'info>,
}

/// Handler for accepting platform authority transfer
///
/// This completes a two-step authority transfer process. The new authority
/// must sign to accept the transfer that was initiated by the current authority.
///
/// # Security
/// - Only the `pending_authority` can accept the transfer
/// - Atomically updates `platform_authority` and clears `pending_authority`
/// - Prevents unauthorized authority takeover
///
/// # Errors
/// Returns an error if:
/// - No pending transfer exists
/// - Signer is not the `pending_authority`
pub fn handler(ctx: Context<AcceptAuthority>, _args: AcceptAuthorityArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Ensure a pending transfer exists
    let pending_authority = config
        .pending_authority
        .ok_or(RecurringPaymentError::NoPendingTransfer)?;

    // Ensure signer is the pending authority
    require!(
        ctx.accounts.new_authority.key() == pending_authority,
        RecurringPaymentError::Unauthorized
    );

    let old_authority = config.platform_authority;

    // Complete the transfer
    config.platform_authority = pending_authority;
    config.pending_authority = None;

    msg!(
        "Authority transfer completed from {} to {}",
        old_authority,
        pending_authority
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accept_authority_args_default() {
        let args = AcceptAuthorityArgs::default();
        // Verify default construction works
        let _serialized = args.try_to_vec().unwrap();
    }

    #[test]
    fn test_accept_authority_args_serialization() {
        let args = AcceptAuthorityArgs::default();

        // Test serialization round-trip
        let serialized = args.try_to_vec().unwrap();
        let deserialized: AcceptAuthorityArgs =
            AcceptAuthorityArgs::try_from_slice(&serialized).unwrap();

        // Since the struct has no fields, just verify it deserializes
        let _ = deserialized;
    }

    #[test]
    fn test_accept_authority_args_clone() {
        let args = AcceptAuthorityArgs::default();
        let cloned = args.clone();

        // Since the struct has no fields, just verify cloning works
        let _serialized1 = args.try_to_vec().unwrap();
        let _serialized2 = cloned.try_to_vec().unwrap();
    }
}
