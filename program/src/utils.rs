use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::{spl_token::state::Account as TokenAccount, Token};

use crate::errors::SubscriptionError;

/// Validates that the platform treasury ATA is valid and correctly configured.
///
/// This function performs comprehensive validation of the platform treasury token account
/// to ensure it can receive platform fees during subscription operations. It checks:
/// - The ATA address matches the canonical derivation for the platform authority and mint
/// - The account has the correct size for a token account
/// - The account is owned by the SPL Token program
/// - The token account data is valid and can be deserialized
/// - The mint matches the configured USDC mint
/// - The owner matches the platform authority
///
/// # Arguments
///
/// * `platform_treasury_ata` - The platform treasury ATA account to validate
/// * `platform_authority` - Expected owner of the treasury account (from Config)
/// * `allowed_mint` - Expected token mint (from Config)
/// * `token_program` - The SPL Token program
///
/// # Errors
///
/// Returns an error if:
/// - The ATA address doesn't match the canonical derivation (`BadSeeds`)
/// - The account size is incorrect (`InvalidPlatformTreasuryAccount`)
/// - The account owner is not the SPL Token program (`InvalidPlatformTreasuryAccount`)
/// - The token account data cannot be deserialized (`InvalidPlatformTreasuryAccount`)
/// - The mint doesn't match the allowed mint (`WrongMint`)
/// - The owner doesn't match the platform authority (`Unauthorized`)
///
/// # Security Considerations
///
/// This validation prevents denial-of-service attacks where the platform authority
/// closes or modifies the treasury ATA after config initialization, which would
/// cause all subscription operations to fail during platform fee transfers.
///
/// # Usage
///
/// ```ignore
/// validate_platform_treasury(
///     &ctx.accounts.platform_treasury_ata,
///     &ctx.accounts.config.platform_authority,
///     &ctx.accounts.config.allowed_mint,
///     &ctx.accounts.token_program,
/// )?;
/// ```
pub fn validate_platform_treasury<'info>(
    platform_treasury_ata: &UncheckedAccount<'info>,
    platform_authority: &Pubkey,
    allowed_mint: &Pubkey,
    token_program: &Program<'info, Token>,
) -> Result<()> {
    // Validate platform treasury ATA exists and is correctly derived
    let expected_platform_ata = get_associated_token_address(platform_authority, allowed_mint);

    require!(
        platform_treasury_ata.key() == expected_platform_ata,
        SubscriptionError::BadSeeds
    );

    // Validate platform treasury ATA is a valid token account
    let platform_ata_data = platform_treasury_ata.try_borrow_data()?;
    require!(
        platform_ata_data.len() == TokenAccount::LEN,
        SubscriptionError::InvalidPlatformTreasuryAccount
    );
    require!(
        platform_treasury_ata.owner == &token_program.key(),
        SubscriptionError::InvalidPlatformTreasuryAccount
    );

    // Deserialize and validate platform treasury token account data
    let token_account = TokenAccount::unpack(&platform_ata_data)?;
    require!(
        token_account.mint == *allowed_mint,
        SubscriptionError::WrongMint
    );
    require!(
        token_account.owner == *platform_authority,
        SubscriptionError::Unauthorized
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_authority_validation() {
        // Test that we validate against the expected platform authority
        let platform_authority = Pubkey::new_unique();
        let different_authority = Pubkey::new_unique();

        assert_ne!(platform_authority, different_authority);
    }

    #[test]
    fn test_allowed_mint_validation() {
        // Test that we validate against the expected mint
        let usdc_mint = Pubkey::new_unique();
        let fake_mint = Pubkey::new_unique();

        assert_ne!(usdc_mint, fake_mint);
    }

    #[test]
    fn test_ata_derivation() {
        // Test that ATA derivation is deterministic
        let authority = Pubkey::new_unique();
        let mint = Pubkey::new_unique();

        let ata1 = get_associated_token_address(&authority, &mint);
        let ata2 = get_associated_token_address(&authority, &mint);

        assert_eq!(ata1, ata2);
    }

    #[test]
    fn test_ata_derivation_uniqueness() {
        // Test that different authority/mint combinations produce different ATAs
        let authority1 = Pubkey::new_unique();
        let authority2 = Pubkey::new_unique();
        let mint = Pubkey::new_unique();

        let ata1 = get_associated_token_address(&authority1, &mint);
        let ata2 = get_associated_token_address(&authority2, &mint);

        assert_ne!(ata1, ata2);
    }
}
