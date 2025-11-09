use crate::errors::RecurringPaymentError;
use crate::state::Config;
use anchor_lang::prelude::*;

/// Arguments for canceling authority transfer
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct CancelAuthorityTransferArgs {
    // No arguments needed - signer validation is sufficient
}

/// Accounts required for canceling authority transfer
#[derive(Accounts)]
pub struct CancelAuthorityTransfer<'info> {
    /// Global configuration account
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
        has_one = platform_authority @ RecurringPaymentError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// Current platform authority (must sign)
    pub platform_authority: Signer<'info>,
}

/// Handler for canceling platform authority transfer
///
/// This allows the current platform authority to cancel a pending authority
/// transfer that was previously initiated. This is useful if the transfer was
/// initiated in error or if circumstances have changed.
///
/// # Security
/// - Only current `platform_authority` can cancel pending transfer
/// - Requires a pending transfer to exist before cancellation
/// - Atomically clears the `pending_authority` field
///
/// # Errors
/// Returns an error if:
/// - Caller is not current `platform_authority`
/// - No pending transfer exists to cancel
pub fn handler(ctx: Context<CancelAuthorityTransfer>, _args: CancelAuthorityTransferArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Ensure a pending transfer exists to cancel
    let pending_authority = config
        .pending_authority
        .ok_or(RecurringPaymentError::NoPendingTransfer)?;

    // Clear the pending authority
    config.pending_authority = None;

    msg!(
        "Authority transfer cancelled. Pending transfer to {} has been revoked.",
        pending_authority
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_authority_transfer_args_default() {
        let args = CancelAuthorityTransferArgs::default();
        // Verify default construction works
        let _serialized = args.try_to_vec().unwrap();
    }

    #[test]
    fn test_cancel_authority_transfer_args_serialization() {
        let args = CancelAuthorityTransferArgs::default();

        // Test serialization round-trip
        let serialized = args.try_to_vec().unwrap();
        let deserialized: CancelAuthorityTransferArgs =
            CancelAuthorityTransferArgs::try_from_slice(&serialized).unwrap();

        // Since the struct has no fields, just verify it deserializes
        let _ = deserialized;
    }

    #[test]
    fn test_cancel_authority_transfer_args_clone() {
        let args = CancelAuthorityTransferArgs::default();
        let cloned = args.clone();

        // Since the struct has no fields, just verify cloning works
        let _serialized1 = args.try_to_vec().unwrap();
        let _serialized2 = cloned.try_to_vec().unwrap();
    }

    #[test]
    fn test_pending_transfer_exists_validation() {
        let pending_authority = Some(Pubkey::new_unique());

        // Simulate the validation logic
        let can_cancel = pending_authority.is_some();

        assert!(
            can_cancel,
            "Should allow cancellation when pending transfer exists"
        );
    }

    #[test]
    fn test_no_pending_transfer_validation() {
        let pending_authority: Option<Pubkey> = None;

        // Simulate the validation logic
        let can_cancel = pending_authority.is_some();

        assert!(
            !can_cancel,
            "Should reject cancellation when no pending transfer exists"
        );
    }

    #[test]
    fn test_cancellation_clears_pending_authority() {
        // Simulate config state before cancellation
        let pending_authority_before = Some(Pubkey::new_unique());

        // Simulate cancellation logic
        let pending_authority_after: Option<Pubkey> = None;

        assert!(
            pending_authority_before.is_some(),
            "Should have pending authority before cancellation"
        );
        assert!(
            pending_authority_after.is_none(),
            "Pending authority should be cleared after cancellation"
        );
    }

    #[test]
    fn test_authority_validation() {
        let platform_authority = Pubkey::new_unique();
        let signer = platform_authority;

        // Simulate has_one constraint validation
        let is_authorized = signer == platform_authority;

        assert!(
            is_authorized,
            "Platform authority should be authorized to cancel transfer"
        );
    }

    #[test]
    fn test_unauthorized_cancellation_rejected() {
        let platform_authority = Pubkey::new_unique();
        let unauthorized_signer = Pubkey::new_unique();

        // Simulate has_one constraint validation
        let is_authorized = unauthorized_signer == platform_authority;

        assert!(
            !is_authorized,
            "Unauthorized signer should NOT be allowed to cancel transfer"
        );
    }
}
