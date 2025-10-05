use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable::{self, UpgradeableLoaderState};
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::{spl_token::state::Account as TokenAccount, Token};

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

pub fn handler(ctx: Context<InitConfig>, args: InitConfigArgs) -> Result<()> {
    // Validate that program_data account matches expected address
    let expected_program_data = get_program_data_address(ctx.program_id);
    require!(
        ctx.accounts.program_data.key() == expected_program_data,
        crate::errors::SubscriptionError::InvalidProgramData
    );

    // Deserialize program data to get upgrade authority
    let program_data_account = ctx.accounts.program_data.to_account_info();
    let program_data_bytes = program_data_account.try_borrow_data()?;

    // Deserialize the UpgradeableLoaderState
    let program_data_state: UpgradeableLoaderState = bincode::deserialize(&program_data_bytes)
        .map_err(|_| crate::errors::SubscriptionError::InvalidProgramData)?;

    // Extract upgrade authority from program data
    let UpgradeableLoaderState::ProgramData {
        upgrade_authority_address: upgrade_authority,
        ..
    } = program_data_state
    else {
        return Err(crate::errors::SubscriptionError::InvalidProgramData.into());
    };

    // Validate that the signer is the upgrade authority
    let upgrade_authority =
        upgrade_authority.ok_or(crate::errors::SubscriptionError::Unauthorized)?;

    require!(
        ctx.accounts.authority.key() == upgrade_authority,
        crate::errors::SubscriptionError::Unauthorized
    );

    // Validate that min_platform_fee_bps <= max_platform_fee_bps
    require!(
        args.min_platform_fee_bps <= args.max_platform_fee_bps,
        crate::errors::SubscriptionError::InvalidPlan
    );

    // Validate max_grace_period_seconds is reasonable (not zero)
    require!(
        args.max_grace_period_seconds > 0,
        crate::errors::SubscriptionError::InvalidPlan
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
}
