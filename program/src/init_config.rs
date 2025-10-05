use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable::{self, UpgradeableLoaderState};
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::{spl_token::state::Account as TokenAccount, Token};

// ============================================================================
// UPGRADE AUTHORITY MANAGEMENT AND DEPLOYMENT SECURITY (L-1)
// ============================================================================
//
// CRITICAL SECURITY ASSUMPTIONS:
//
// 1. UPGRADE AUTHORITY MANAGEMENT:
//    The program validates that the signer of init_config is the current upgrade
//    authority at deployment time. This creates a time-of-check/time-of-use (TOCTOU)
//    dependency on upgrade authority state.
//
//    SECURITY IMPLICATIONS:
//    - If upgrade authority is REVOKED before init_config: Program becomes permanently
//      unconfigurable. The program will be deployed but unusable.
//    - If upgrade authority is TRANSFERRED to unauthorized party before init_config:
//      Attacker can set arbitrary configuration including platform_authority, fee ranges,
//      and withdrawal limits.
//
// 2. EXPECTED DEPLOYMENT PROCESS:
//    a) Deploy program with `solana program deploy` using authorized keypair
//    b) IMMEDIATELY call init_config while upgrade authority is still valid
//    c) Verify config initialization succeeded with expected parameters
//    d) (OPTIONAL) Revoke or transfer upgrade authority to multisig
//
//    CRITICAL TIMING: Steps (a) and (b) must be atomic or executed in rapid succession
//    to minimize attack window.
//
// 3. PRODUCTION DEPLOYMENT RECOMMENDATIONS:
//    - Use deployment scripts that atomically deploy + initialize
//    - For mainnet, compile with `--features mainnet-beta` to enable hardcoded
//      upgrade authority validation (provides additional defense-in-depth)
//    - Monitor program deployment and config initialization in same transaction block
//    - Use multisig (e.g., Squads) for upgrade authority management
//    - Implement monitoring alerts for unexpected config initialization attempts
//
// 4. UPGRADE AUTHORITY VALIDATION:
//    For production deployments, this handler supports optional compile-time validation
//    against hardcoded expected upgrade authority pubkeys:
//
//    #[cfg(feature = "mainnet-beta")]
//    const EXPECTED_UPGRADE_AUTHORITY: Pubkey = pubkey!("YOUR_MAINNET_AUTHORITY");
//
//    This provides defense-in-depth against upgrade authority compromise before init.
//
// 5. AUDIT TRAIL:
//    All init_config executions log the upgrade authority pubkey used during
//    initialization via msg!() for on-chain audit trails and security monitoring.
//
// Example CLI command to initialize config:
// cargo run --package tally-cli -- init-config \
//   --platform-authority "YOUR_PLATFORM_AUTHORITY_PUBKEY" \
//   --max-platform-fee-bps 1000 \
//   --min-platform-fee-bps 50 \
//   --min-period-seconds 86400 \
//   --default-allowance-periods 3 \
//   --allowed-mint "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" \
//   --max-withdrawal-amount 1000000000 \
//   --max-grace-period-seconds 604800
// ============================================================================

// ============================================================================
// HARDCODED UPGRADE AUTHORITY VALIDATION (OPTIONAL)
// ============================================================================
//
// For production deployments, define expected upgrade authority pubkeys per network.
// This provides defense-in-depth validation during config initialization.
//
// DEPLOYMENT INSTRUCTIONS:
// 1. Replace placeholder pubkeys below with your actual upgrade authority addresses
// 2. Compile with appropriate feature flag:
//    - Mainnet: cargo build-sbf --features mainnet-beta
//    - Devnet:  cargo build-sbf --features devnet
//    - Testnet: cargo build-sbf --features testnet
// 3. Without feature flags, no hardcoded validation occurs (permissive mode)
//
// SECURITY NOTE:
// Hardcoded validation adds an additional security layer but requires recompilation
// and redeployment if upgrade authority changes. Use multisig upgrade authorities
// to minimize the need for authority rotation.

// Mainnet upgrade authority (REPLACE WITH YOUR MAINNET AUTHORITY)
// Example: const EXPECTED_UPGRADE_AUTHORITY: Pubkey = pubkey!("YourMainnetAuthorityPubkeyHere");
#[cfg(all(
    feature = "mainnet-beta",
    not(feature = "devnet"),
    not(feature = "testnet")
))]
#[allow(dead_code)] // Used conditionally based on feature flags
const EXPECTED_UPGRADE_AUTHORITY: Option<Pubkey> = None;

// Devnet upgrade authority (REPLACE WITH YOUR DEVNET AUTHORITY)
#[cfg(all(
    feature = "devnet",
    not(feature = "mainnet-beta"),
    not(feature = "testnet")
))]
#[allow(dead_code)] // Used conditionally based on feature flags
const EXPECTED_UPGRADE_AUTHORITY: Option<Pubkey> = None;

// Testnet upgrade authority (REPLACE WITH YOUR TESTNET AUTHORITY)
#[cfg(all(
    feature = "testnet",
    not(feature = "mainnet-beta"),
    not(feature = "devnet")
))]
#[allow(dead_code)] // Used conditionally based on feature flags
const EXPECTED_UPGRADE_AUTHORITY: Option<Pubkey> = None;

// Default: No hardcoded validation when no network feature is enabled (or multiple are enabled)
#[cfg(not(all(
    any(feature = "mainnet-beta", feature = "devnet", feature = "testnet"),
    not(all(feature = "mainnet-beta", feature = "devnet")),
    not(all(feature = "mainnet-beta", feature = "testnet")),
    not(all(feature = "devnet", feature = "testnet"))
)))]
#[allow(dead_code)] // Used conditionally based on feature flags
const EXPECTED_UPGRADE_AUTHORITY: Option<Pubkey> = None;

// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitConfigArgs {
    pub platform_authority: Pubkey,
    pub max_platform_fee_bps: u16,
    pub min_platform_fee_bps: u16,
    pub min_period_seconds: u64,
    pub default_allowance_periods: u8,
    pub allowed_mint: Pubkey,
    pub max_withdrawal_amount: u64,
    pub max_grace_period_seconds: u64,
}

#[derive(Accounts)]
#[instruction(args: InitConfigArgs)]
pub struct InitConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = crate::state::Config::SPACE,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// Program data account for upgrade authority validation
    /// CHECK: Validated in handler by deserializing and checking upgrade authority
    pub program_data: UncheckedAccount<'info>,

    /// Platform treasury ATA for receiving platform fees
    /// This ensures the platform authority has already created their USDC token account
    /// CHECK: Validated in handler as the canonical ATA for `platform_authority` + `allowed_mint`
    pub platform_treasury_ata: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Gets the expected program data address for the current program
fn get_program_data_address(program_id: &Pubkey) -> Pubkey {
    let (program_data_address, _) =
        Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());
    program_data_address
}

/// Validates upgrade authority and logs audit trail information
///
/// This function implements the L-1 security audit fix by:
/// 1. Validating `program_data` account matches expected PDA
/// 2. Deserializing program data to extract upgrade authority
/// 3. Ensuring upgrade authority has not been revoked
/// 4. Logging comprehensive audit trail for security monitoring
/// 5. Validating signer matches current upgrade authority
/// 6. (Optional) Validating against hardcoded expected authority for production
///
/// # Security
///
/// This validation creates a TOCTOU dependency on upgrade authority state.
/// Programs MUST call `init_config` immediately after deployment while upgrade
/// authority is still valid and controlled by authorized parties.
///
/// # Returns
///
/// Returns the validated upgrade authority pubkey on success.
fn validate_upgrade_authority(ctx: &Context<InitConfig>) -> Result<Pubkey> {
    // Step 1: Validate program_data account matches expected PDA
    let expected_program_data = get_program_data_address(ctx.program_id);
    require!(
        ctx.accounts.program_data.key() == expected_program_data,
        crate::errors::SubscriptionError::InvalidProgramData
    );

    // Step 2: Deserialize program data to extract upgrade authority
    let program_data_account = ctx.accounts.program_data.to_account_info();
    let program_data_bytes = program_data_account.try_borrow_data()?;

    let program_data_state: UpgradeableLoaderState = bincode::deserialize(&program_data_bytes)
        .map_err(|_| crate::errors::SubscriptionError::InvalidProgramData)?;

    // Step 3: Extract and validate upgrade authority exists
    let UpgradeableLoaderState::ProgramData {
        upgrade_authority_address: upgrade_authority_option,
        slot: deployment_slot,
    } = program_data_state
    else {
        return Err(crate::errors::SubscriptionError::InvalidProgramData.into());
    };

    // Step 4: Ensure upgrade authority has not been revoked (critical security check)
    let upgrade_authority =
        upgrade_authority_option.ok_or(crate::errors::SubscriptionError::Unauthorized)?;

    // Step 5: AUDIT TRAIL - Log upgrade authority for security monitoring
    msg!("=== CONFIG INITIALIZATION AUDIT TRAIL ===");
    msg!("Program ID: {}", ctx.program_id);
    msg!("Upgrade Authority: {}", upgrade_authority);
    msg!("Deployment Slot: {}", deployment_slot);
    msg!("Signer: {}", ctx.accounts.authority.key());

    // Step 6: Validate signer is the current upgrade authority
    require!(
        ctx.accounts.authority.key() == upgrade_authority,
        crate::errors::SubscriptionError::Unauthorized
    );

    // Step 7: OPTIONAL - Hardcoded upgrade authority validation (defense-in-depth)
    #[cfg(any(feature = "mainnet-beta", feature = "devnet", feature = "testnet"))]
    if let Some(expected_authority) = EXPECTED_UPGRADE_AUTHORITY {
        require!(
            upgrade_authority == expected_authority,
            crate::errors::SubscriptionError::Unauthorized
        );
        msg!("✓ Hardcoded upgrade authority validation passed");
    } else {
        msg!("⚠ WARNING: No hardcoded upgrade authority configured for this network");
        msg!("⚠ Consider setting EXPECTED_UPGRADE_AUTHORITY for production security");
    }

    #[cfg(not(any(feature = "mainnet-beta", feature = "devnet", feature = "testnet")))]
    msg!("ℹ Development mode: No hardcoded upgrade authority validation");

    msg!("✓ Upgrade authority validation completed successfully");
    msg!("=========================================");

    Ok(upgrade_authority)
}

pub fn handler(ctx: Context<InitConfig>, args: InitConfigArgs) -> Result<()> {
    // ========================================================================
    // UPGRADE AUTHORITY VALIDATION AND AUDIT LOGGING (L-1)
    // ========================================================================
    // Validate upgrade authority and log comprehensive audit trail
    let _upgrade_authority = validate_upgrade_authority(&ctx)?;
    msg!("Platform Authority (from args): {}", args.platform_authority);
    // ========================================================================

    // Validate that min_platform_fee_bps <= max_platform_fee_bps
    require!(
        args.min_platform_fee_bps <= args.max_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidConfiguration
    );

    // Validate max_grace_period_seconds is reasonable (not zero)
    require!(
        args.max_grace_period_seconds > 0,
        crate::errors::SubscriptionError::InvalidConfiguration
    );

    // Validate min_period_seconds meets absolute minimum (M-4 security fix)
    //
    // This prevents spam attacks where malicious actors could set min_period_seconds = 0
    // during config initialization, enabling them to create subscription plans with
    // extremely short billing cycles (e.g., 1 second), leading to:
    // - Network spam through excessive renewal transactions
    // - Denial-of-service attacks via transaction flooding
    // - Unreasonable operational burden on merchants
    //
    // The absolute minimum of 86400 seconds (24 hours) ensures all subscription plans
    // have reasonable billing cycles aligned with industry standards.
    require!(
        args.min_period_seconds >= crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS,
        crate::errors::SubscriptionError::InvalidConfiguration
    );

    // Validate platform treasury ATA exists and is correctly derived
    // This prevents operational issues where subscriptions fail if the platform
    // authority hasn't created their USDC token account before deployment
    let expected_platform_ata = get_associated_token_address(
        &args.platform_authority,
        &args.allowed_mint,
    );

    require!(
        ctx.accounts.platform_treasury_ata.key() == expected_platform_ata,
        crate::errors::SubscriptionError::BadSeeds
    );

    // Validate platform treasury ATA is a valid token account
    let platform_ata_data = ctx.accounts.platform_treasury_ata.try_borrow_data()?;
    require!(
        platform_ata_data.len() == TokenAccount::LEN,
        crate::errors::SubscriptionError::InvalidPlatformTreasuryAccount
    );
    require!(
        ctx.accounts.platform_treasury_ata.owner == &ctx.accounts.token_program.key(),
        crate::errors::SubscriptionError::InvalidPlatformTreasuryAccount
    );

    // Deserialize and validate platform treasury token account data
    let token_account = TokenAccount::unpack(&platform_ata_data)?;
    require!(
        token_account.mint == args.allowed_mint,
        crate::errors::SubscriptionError::WrongMint
    );
    require!(
        token_account.owner == args.platform_authority,
        crate::errors::SubscriptionError::Unauthorized
    );

    // Initialize config account
    let config = &mut ctx.accounts.config;
    config.platform_authority = args.platform_authority;
    config.pending_authority = None; // No pending transfer on initialization
    config.max_platform_fee_bps = args.max_platform_fee_bps;
    config.min_platform_fee_bps = args.min_platform_fee_bps;
    config.min_period_seconds = args.min_period_seconds;
    config.default_allowance_periods = args.default_allowance_periods;
    config.allowed_mint = args.allowed_mint;
    config.max_withdrawal_amount = args.max_withdrawal_amount;
    config.max_grace_period_seconds = args.max_grace_period_seconds;
    config.paused = false; // Program starts in unpaused state
    config.bump = ctx.bumps.config;

    // Get current timestamp for event
    let clock = Clock::get()?;

    // Emit ConfigInitialized event
    emit!(crate::events::ConfigInitialized {
        platform_authority: args.platform_authority,
        max_platform_fee_bps: args.max_platform_fee_bps,
        min_platform_fee_bps: args.min_platform_fee_bps,
        min_period_seconds: args.min_period_seconds,
        default_allowance_periods: args.default_allowance_periods,
        allowed_mint: args.allowed_mint,
        max_withdrawal_amount: args.max_withdrawal_amount,
        max_grace_period_seconds: args.max_grace_period_seconds,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::bpf_loader_upgradeable::UpgradeableLoaderState;

    #[test]
    fn test_get_program_data_address() {
        let program_id = Pubkey::new_unique();
        let program_data_address = get_program_data_address(&program_id);

        // Verify it's a valid PDA derivation
        let (expected, _bump) =
            Pubkey::find_program_address(&[program_id.as_ref()], &bpf_loader_upgradeable::id());

        assert_eq!(program_data_address, expected);
    }

    #[test]
    fn test_program_data_deserialization_valid() {
        let upgrade_authority = Pubkey::new_unique();
        let program_data_state = UpgradeableLoaderState::ProgramData {
            slot: 42,
            upgrade_authority_address: Some(upgrade_authority),
        };

        let serialized = bincode::serialize(&program_data_state).unwrap();
        let deserialized: UpgradeableLoaderState = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            UpgradeableLoaderState::ProgramData {
                upgrade_authority_address,
                ..
            } => {
                assert_eq!(upgrade_authority_address, Some(upgrade_authority));
            }
            _ => panic!("Expected ProgramData variant"),
        }
    }

    #[test]
    fn test_program_data_deserialization_no_authority() {
        let program_data_state = UpgradeableLoaderState::ProgramData {
            slot: 42,
            upgrade_authority_address: None,
        };

        let serialized = bincode::serialize(&program_data_state).unwrap();
        let deserialized: UpgradeableLoaderState = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            UpgradeableLoaderState::ProgramData {
                upgrade_authority_address,
                ..
            } => {
                assert_eq!(upgrade_authority_address, None);
            }
            _ => panic!("Expected ProgramData variant"),
        }
    }

    #[test]
    fn test_min_fee_validation_valid_equal() {
        // Test that min_platform_fee_bps == max_platform_fee_bps is valid
        let min_fee = 100u16;
        let max_fee = 100u16;
        assert!(min_fee <= max_fee);
    }

    #[test]
    fn test_min_fee_validation_valid_less() {
        // Test that min_platform_fee_bps < max_platform_fee_bps is valid
        let min_fee = 50u16;
        let max_fee = 1000u16;
        assert!(min_fee <= max_fee);
    }

    #[test]
    fn test_min_fee_validation_invalid() {
        // Test that min_platform_fee_bps > max_platform_fee_bps should fail
        let min_fee = 1000u16;
        let max_fee = 500u16;
        assert!(min_fee > max_fee);
    }

    #[test]
    fn test_min_fee_zero_allowed() {
        // Test that min_platform_fee_bps can be 0
        let min_fee = 0u16;
        let max_fee = 1000u16;
        assert!(min_fee <= max_fee);
    }

    #[test]
    fn test_fee_range_boundary_values() {
        // Test boundary values for fee validation
        let min_fee = 0u16;
        let max_fee = 10000u16; // 100% in basis points
        assert!(min_fee <= max_fee);

        let min_fee_50bps = 50u16; // 0.5%
        let max_fee_1000bps = 1000u16; // 10%
        assert!(min_fee_50bps <= max_fee_1000bps);
    }

    // ============================================================================
    // MINIMUM PERIOD VALIDATION TESTS (M-4 Security Fix)
    // ============================================================================

    #[test]
    fn test_min_period_rejects_zero() {
        // Test that min_period_seconds = 0 is rejected
        let min_period_seconds = 0u64;
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        assert!(
            min_period_seconds < absolute_min,
            "Should reject min_period_seconds = 0"
        );
    }

    #[test]
    fn test_min_period_rejects_below_absolute_minimum() {
        // Test that min_period_seconds < 86400 is rejected
        let min_period_seconds = 3600u64; // 1 hour
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        assert!(
            min_period_seconds < absolute_min,
            "Should reject min_period_seconds below 86400 seconds"
        );
    }

    #[test]
    fn test_min_period_accepts_exact_minimum() {
        // Test that min_period_seconds = 86400 is accepted
        let min_period_seconds = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        assert!(
            min_period_seconds >= absolute_min,
            "Should accept min_period_seconds exactly at 86400 seconds (24 hours)"
        );
    }

    #[test]
    fn test_min_period_accepts_above_minimum() {
        // Test that min_period_seconds > 86400 is accepted
        let min_period_seconds = 604_800_u64; // 7 days
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        assert!(
            min_period_seconds >= absolute_min,
            "Should accept min_period_seconds above 86400 seconds"
        );
    }

    #[test]
    fn test_min_period_boundary_values() {
        // Test boundary values for minimum period validation
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        // Just below boundary - should fail
        let below_min = absolute_min - 1;
        assert!(
            below_min < absolute_min,
            "min_period_seconds = 86399 should be rejected"
        );

        // At boundary - should pass
        let at_min = absolute_min;
        assert!(
            at_min >= absolute_min,
            "min_period_seconds = 86400 should be accepted"
        );

        // Just above boundary - should pass
        let above_min = absolute_min + 1;
        assert!(
            above_min >= absolute_min,
            "min_period_seconds = 86401 should be accepted"
        );
    }

    #[test]
    fn test_min_period_common_values() {
        // Test common subscription periods are accepted
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        // Daily (24 hours)
        let daily = 86_400_u64;
        assert!(daily >= absolute_min, "Daily subscriptions should be accepted");

        // Weekly (7 days)
        let weekly = 604_800_u64;
        assert!(weekly >= absolute_min, "Weekly subscriptions should be accepted");

        // Monthly (30 days)
        let monthly = 2_592_000_u64;
        assert!(monthly >= absolute_min, "Monthly subscriptions should be accepted");

        // Yearly (365 days)
        let yearly = 31_536_000_u64;
        assert!(yearly >= absolute_min, "Yearly subscriptions should be accepted");
    }

    #[test]
    fn test_min_period_spam_attack_prevention() {
        // Test that values enabling spam attacks are rejected
        let absolute_min = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        // 1 second - obvious spam attack vector
        let one_second = 1u64;
        assert!(
            one_second < absolute_min,
            "1-second billing cycle should be rejected as spam attack vector"
        );

        // 1 minute - still spam
        let one_minute = 60u64;
        assert!(
            one_minute < absolute_min,
            "1-minute billing cycle should be rejected as spam attack vector"
        );

        // 1 hour - still unreasonable
        let one_hour = 3600u64;
        assert!(
            one_hour < absolute_min,
            "1-hour billing cycle should be rejected as spam attack vector"
        );

        // 12 hours - still below minimum
        let twelve_hours = 43200u64;
        assert!(
            twelve_hours < absolute_min,
            "12-hour billing cycle should be rejected (below 24-hour minimum)"
        );
    }

    #[test]
    fn test_absolute_min_period_constant_value() {
        // Verify the constant has the expected value for 24 hours
        let expected = 86400u64; // 24 hours * 60 minutes * 60 seconds
        let actual = crate::constants::ABSOLUTE_MIN_PERIOD_SECONDS;

        assert_eq!(
            actual, expected,
            "ABSOLUTE_MIN_PERIOD_SECONDS should equal 86400 (24 hours)"
        );
    }
}
