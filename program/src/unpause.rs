use crate::errors::SubscriptionError;
use crate::events::ProgramUnpaused;
use crate::state::Config;
use anchor_lang::prelude::*;

/// Arguments for unpausing the program
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UnpauseArgs {}

/// Accounts required for unpausing the program
#[derive(Accounts)]
pub struct Unpause<'info> {
    /// Global configuration account
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
        has_one = platform_authority @ SubscriptionError::Unauthorized
    )]
    pub config: Account<'info, Config>,

    /// Platform authority (must sign)
    pub platform_authority: Signer<'info>,
}

/// Handler for unpausing the program
///
/// This disables the emergency pause mechanism, re-enabling all user-facing operations
/// (`start_subscription`, `renew_subscription`, `create_plan`).
///
/// # Security
/// - Only `platform_authority` can unpause the program
/// - Pause state is stored in the Config account
/// - Events are emitted for transparency and off-chain monitoring
///
/// # Errors
/// Returns an error if:
/// - Caller is not the platform authority
pub fn handler(ctx: Context<Unpause>, _args: UnpauseArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Set paused state to false
    config.paused = false;

    // Get current timestamp for event
    let clock = Clock::get()?;

    // Emit ProgramUnpaused event
    emit!(ProgramUnpaused {
        authority: ctx.accounts.platform_authority.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Program unpaused by platform authority: {}",
        ctx.accounts.platform_authority.key()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unpause_args_serialization() {
        let args = UnpauseArgs {};

        // Test serialization round-trip
        let serialized = args.try_to_vec().unwrap();
        let deserialized: UnpauseArgs = UnpauseArgs::try_from_slice(&serialized).unwrap();

        // UnpauseArgs has no fields, so just verify it deserializes successfully
        let _ = deserialized;
    }

    #[test]
    fn test_unpause_args_clone() {
        let args = UnpauseArgs {};

        // Verify clone trait is implemented
        #[allow(clippy::redundant_clone)]
        let cloned = args.clone();

        // Verify cloned instance can be serialized
        let _ = cloned.try_to_vec().unwrap();
    }
}
