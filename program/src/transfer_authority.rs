use crate::errors::SubscriptionError;
use crate::state::Config;
use anchor_lang::prelude::*;

/// Arguments for initiating authority transfer
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TransferAuthorityArgs {
    /// The new authority to transfer to
    pub new_authority: Pubkey,
}

/// Accounts required for initiating authority transfer
#[derive(Accounts)]
pub struct TransferAuthority<'info> {
    /// Global configuration account
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
        has_one = platform_authority @ SubscriptionError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// Current platform authority (must sign)
    pub platform_authority: Signer<'info>,
}

/// Handler for initiating platform authority transfer
///
/// This initiates a two-step authority transfer process. The current authority
/// proposes a new authority, which must then accept the transfer.
///
/// # Security
/// - Only current `platform_authority` can initiate transfer
/// - Cannot overwrite existing pending transfer
/// - New authority must be different from current authority
///
/// # Errors
/// Returns an error if:
/// - Caller is not current `platform_authority`
/// - A pending transfer already exists
/// - New authority is same as current authority
pub fn handler(ctx: Context<TransferAuthority>, args: TransferAuthorityArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Ensure no pending transfer exists
    require!(
        config.pending_authority.is_none(),
        SubscriptionError::TransferAlreadyPending
    );

    // Ensure new authority is different from current
    require!(
        args.new_authority != config.platform_authority,
        SubscriptionError::InvalidTransferTarget
    );

    // Set pending authority
    config.pending_authority = Some(args.new_authority);

    msg!(
        "Authority transfer initiated from {} to {}",
        config.platform_authority,
        args.new_authority
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_authority_args_serialization() {
        let new_authority = Pubkey::new_unique();
        let args = TransferAuthorityArgs { new_authority };

        // Test serialization round-trip
        let serialized = args.try_to_vec().unwrap();
        let deserialized: TransferAuthorityArgs =
            TransferAuthorityArgs::try_from_slice(&serialized).unwrap();

        assert_eq!(deserialized.new_authority, new_authority);
    }

    #[test]
    fn test_transfer_authority_args_clone() {
        let new_authority = Pubkey::new_unique();
        let args = TransferAuthorityArgs { new_authority };
        let cloned = args.clone();

        assert_eq!(cloned.new_authority, args.new_authority);
    }
}
