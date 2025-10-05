use crate::errors::SubscriptionError;
use crate::events::ProgramPaused;
use crate::state::Config;
use anchor_lang::prelude::*;

/// Arguments for pausing the program
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PauseArgs {}

/// Accounts required for pausing the program
#[derive(Accounts)]
pub struct Pause<'info> {
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

/// Handler for pausing the program
///
/// This enables the emergency pause mechanism, disabling all user-facing operations
/// (`start_subscription`, `renew_subscription`, `create_plan`) while allowing admin
/// operations to continue for emergency fund recovery.
///
/// # Security
/// - Only `platform_authority` can pause the program
/// - Pause state is stored in the Config account
/// - Events are emitted for transparency and off-chain monitoring
///
/// # Errors
/// Returns an error if:
/// - Caller is not the platform authority
pub fn handler(ctx: Context<Pause>, _args: PauseArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Set paused state to true
    config.paused = true;

    // Get current timestamp for event
    let clock = Clock::get()?;

    // Emit ProgramPaused event
    emit!(ProgramPaused {
        authority: ctx.accounts.platform_authority.key(),
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Program paused by platform authority: {}",
        ctx.accounts.platform_authority.key()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pause_args_serialization() {
        let args = PauseArgs {};

        // Test serialization round-trip
        let serialized = args.try_to_vec().unwrap();
        let deserialized: PauseArgs = PauseArgs::try_from_slice(&serialized).unwrap();

        // PauseArgs has no fields, so just verify it deserializes successfully
        let _ = deserialized;
    }

    #[test]
    fn test_pause_args_clone() {
        let args = PauseArgs {};

        // Verify clone trait is implemented
        let _ = args.clone();
    }
}
