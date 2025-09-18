//! Validation utilities for Tally operations

use crate::{
    ata::{get_associated_token_address_with_program, get_token_account_info, TokenProgram},
    error::{Result, TallyError},
    SimpleTallyClient,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Get and validate USDC mint address
///
/// # Errors
/// Returns an error if the USDC mint address is invalid
pub fn get_usdc_mint(usdc_mint_str: Option<&str>) -> Result<Pubkey> {
    let mint_str = usdc_mint_str.unwrap_or("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"); // Mainnet USDC
    Pubkey::from_str(mint_str)
        .map_err(|e| TallyError::Generic(format!("Invalid USDC mint address '{mint_str}': {e}")))
}

/// Validate plan parameters according to program constraints
///
/// # Errors
/// Returns an error if plan parameters are invalid
pub fn validate_plan_parameters(price_usdc: u64, period_secs: i64, grace_secs: i64) -> Result<()> {
    // Price must be greater than 0
    if price_usdc == 0 {
        return Err(TallyError::Generic(
            "Plan price must be greater than 0".to_string(),
        ));
    }

    // Period must be at least 24 hours (86400 seconds)
    if period_secs < 86400 {
        return Err(TallyError::Generic(format!(
            "Plan period must be at least 24 hours (86400 seconds), got: {period_secs}"
        )));
    }

    // Period must be positive
    if period_secs <= 0 {
        return Err(TallyError::Generic(format!(
            "Plan period must be positive, got: {period_secs}"
        )));
    }

    // Grace period must be non-negative
    if grace_secs < 0 {
        return Err(TallyError::Generic(format!(
            "Grace period must be non-negative, got: {grace_secs}"
        )));
    }

    // Grace period cannot exceed 2x the billing period
    let max_grace_secs = 2_i64.saturating_mul(period_secs);
    if grace_secs > max_grace_secs {
        return Err(TallyError::Generic(format!(
            "Grace period ({grace_secs} seconds) cannot exceed 2x the billing period ({period_secs} seconds). Maximum allowed: {max_grace_secs}"
        )));
    }

    Ok(())
}

/// Validate platform fee basis points
///
/// # Errors
/// Returns an error if fee basis points are invalid
pub fn validate_platform_fee_bps(fee_bps: u16) -> Result<()> {
    if fee_bps > 1000 {
        return Err(TallyError::Generic(format!(
            "Platform fee basis points must be between 0-1000 (0-10%), got: {fee_bps}"
        )));
    }
    Ok(())
}

/// Validate withdrawal amount
///
/// # Errors
/// Returns an error if withdrawal amount is invalid
pub fn validate_withdrawal_amount(amount: u64) -> Result<()> {
    if amount == 0 {
        return Err(TallyError::Generic(
            "Withdrawal amount must be greater than 0".to_string(),
        ));
    }
    Ok(())
}

/// Validate that a token account is a valid USDC token account owned by the specified authority
///
/// # Errors
/// Returns an error if validation fails
pub fn validate_usdc_token_account(
    tally_client: &SimpleTallyClient,
    token_account: &Pubkey,
    usdc_mint: &Pubkey,
    expected_owner: &Pubkey,
    account_type: &str, // "treasury" or "destination" for error messages
) -> Result<()> {
    // Check if the account exists and get its information
    let token_account_info = get_token_account_info(tally_client.rpc(), token_account)?
        .ok_or_else(|| {
            TallyError::Generic(format!("{account_type} ATA {token_account} does not exist"))
        })?;

    let (token_account_data, _token_program) = token_account_info;

    // Validate mint matches
    if token_account_data.mint != *usdc_mint {
        return Err(TallyError::Generic(format!(
            "{} ATA mint mismatch: expected {}, found {}",
            account_type, usdc_mint, token_account_data.mint
        )));
    }

    // Validate owner matches authority
    if token_account_data.owner != *expected_owner {
        return Err(TallyError::Generic(format!(
            "{} ATA owner mismatch: expected {}, found {}",
            account_type, expected_owner, token_account_data.owner
        )));
    }

    // Verify it's the correct ATA for this authority and mint
    let expected_ata =
        get_associated_token_address_with_program(expected_owner, usdc_mint, TokenProgram::Token)?;
    if *token_account != expected_ata {
        return Err(TallyError::Generic(format!(
            "{account_type} ATA address mismatch: expected {expected_ata}, provided {token_account}"
        )));
    }

    Ok(())
}

/// Validate that an authority matches the expected merchant for a given plan
///
/// # Errors
/// Returns an error if authority validation fails
pub fn validate_merchant_authority(authority: &Pubkey, expected_merchant: &Pubkey) -> Result<()> {
    let computed_merchant = crate::pda::merchant_address(authority)?;
    if *expected_merchant != computed_merchant {
        return Err(TallyError::Generic(format!(
            "Authority mismatch: expected merchant PDA {computed_merchant} for authority {authority}, but got {expected_merchant}"
        )));
    }
    Ok(())
}

/// Validate that an authority matches the expected merchant for a given plan with custom program ID
///
/// # Errors
/// Returns an error if authority validation fails
pub fn validate_merchant_authority_with_program_id(
    authority: &Pubkey,
    expected_merchant: &Pubkey,
    program_id: &Pubkey,
) -> Result<()> {
    let computed_merchant = crate::pda::merchant_address_with_program_id(authority, program_id);
    if *expected_merchant != computed_merchant {
        return Err(TallyError::Generic(format!(
            "Authority mismatch: expected merchant PDA {computed_merchant} for authority {authority}, but got {expected_merchant}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_plan_parameters() {
        // Valid parameters
        assert!(validate_plan_parameters(5_000_000, 2_592_000, 432_000).is_ok());

        // Zero price
        assert!(validate_plan_parameters(0, 2_592_000, 432_000).is_err());

        // Period too short
        assert!(validate_plan_parameters(5_000_000, 3600, 432_000).is_err());

        // Negative period
        assert!(validate_plan_parameters(5_000_000, -1, 432_000).is_err());

        // Negative grace period
        assert!(validate_plan_parameters(5_000_000, 2_592_000, -1).is_err());

        // Grace period too long
        assert!(validate_plan_parameters(5_000_000, 2_592_000, 6_000_000).is_err());
    }

    #[test]
    fn test_validate_platform_fee_bps() {
        // Valid fee
        assert!(validate_platform_fee_bps(50).is_ok());
        assert!(validate_platform_fee_bps(0).is_ok());
        assert!(validate_platform_fee_bps(1000).is_ok());

        // Invalid fee
        assert!(validate_platform_fee_bps(1001).is_err());
    }

    #[test]
    fn test_validate_withdrawal_amount() {
        // Valid amount
        assert!(validate_withdrawal_amount(1000).is_ok());

        // Invalid amount
        assert!(validate_withdrawal_amount(0).is_err());
    }

    #[test]
    fn test_get_usdc_mint() {
        // Default mainnet USDC
        let mint = get_usdc_mint(None).unwrap();
        assert_eq!(
            mint.to_string(),
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        );

        // Custom mint
        let custom_mint = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
        let mint = get_usdc_mint(Some(custom_mint)).unwrap();
        assert_eq!(mint.to_string(), custom_mint);

        // Invalid mint
        assert!(get_usdc_mint(Some("invalid")).is_err());
    }
}
