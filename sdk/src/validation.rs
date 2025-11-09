//! Validation utilities for Tally operations

use crate::{
    ata::{get_associated_token_address_with_program, get_token_account_info, TokenProgram},
    error::{Result, TallyError},
    SimpleTallyClient,
};
use anchor_client::solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Calculate the maximum allowed grace period for a given payment period
///
/// The program enforces that grace periods cannot exceed 30% of the payment period.
/// This formula matches the program's validation logic exactly.
///
/// # Arguments
/// * `period_secs` - The payment period in seconds
///
/// # Returns
/// The maximum allowed grace period in seconds (30% of payment period)
///
/// # Example
/// ```
/// # use tally_sdk::validation::calculate_max_grace_period;
/// // For a 30-day payment period (2,592,000 seconds)
/// let max_grace = calculate_max_grace_period(2_592_000);
/// assert_eq!(max_grace, 777_600); // 9 days (30% of 30 days)
/// ```
#[must_use]
pub const fn calculate_max_grace_period(period_secs: i64) -> i64 {
    // Grace period cannot exceed 30% of payment period
    // Formula: period_secs * 3 / 10 (equivalent to 30%)
    period_secs
        .saturating_mul(3)
        .saturating_div(10)
}

/// Calculate the maximum allowed grace period for a given payment period (u64 version)
///
/// This is a convenience wrapper for `calculate_max_grace_period` that works with u64 values.
///
/// # Arguments
/// * `period_secs` - The payment period in seconds
///
/// # Returns
/// The maximum allowed grace period in seconds (30% of payment period)
#[must_use]
pub const fn calculate_max_grace_period_u64(period_secs: u64) -> u64 {
    // Grace period cannot exceed 30% of payment period
    // Formula: period_secs * 3 / 10 (equivalent to 30%)
    period_secs
        .saturating_mul(3)
        .saturating_div(10)
}

/// Get and validate USDC mint address
///
/// # Errors
/// Returns an error if the USDC mint address is invalid
pub fn get_usdc_mint(usdc_mint_str: Option<&str>) -> Result<Pubkey> {
    let mint_str = usdc_mint_str.unwrap_or("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"); // Mainnet USDC
    Pubkey::from_str(mint_str)
        .map_err(|e| TallyError::Generic(format!("Invalid USDC mint address '{mint_str}': {e}")))
}

/// Validate payment terms parameters according to program constraints
///
/// # Errors
/// Returns an error if payment terms parameters are invalid
pub fn validate_payment_terms_parameters(amount_usdc: u64, period_secs: i64) -> Result<()> {
    // Price must be greater than 0
    if amount_usdc == 0 {
        return Err(TallyError::Generic(
            "Payment amount must be greater than 0".to_string(),
        ));
    }

    // Period must be at least 24 hours (86400 seconds)
    if period_secs < 86400 {
        return Err(TallyError::Generic(format!(
            "Payment period must be at least 24 hours (86400 seconds), got: {period_secs}"
        )));
    }

    // Period must be positive
    if period_secs <= 0 {
        return Err(TallyError::Generic(format!(
            "Payment period must be positive, got: {period_secs}"
        )));
    }

    // Grace period validation removed - grace periods are payment agreement-specific
    // and handled by extension layers, not core protocol

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

/// Validate that an authority matches the expected payee for a given payment terms
///
/// # Errors
/// Returns an error if authority validation fails
pub fn validate_payee_authority(authority: &Pubkey, expected_payee: &Pubkey) -> Result<()> {
    let computed_payee = crate::pda::payee_address(authority)?;
    if *expected_payee != computed_payee {
        return Err(TallyError::Generic(format!(
            "Authority mismatch: expected payee PDA {computed_payee} for authority {authority}, but got {expected_payee}"
        )));
    }
    Ok(())
}

/// Validate that an authority matches the expected payee for a given payment terms with custom program ID
///
/// # Errors
/// Returns an error if authority validation fails
pub fn validate_payee_authority_with_program_id(
    authority: &Pubkey,
    expected_payee: &Pubkey,
    program_id: &Pubkey,
) -> Result<()> {
    let computed_payee = crate::pda::payee_address_with_program_id(authority, program_id);
    if *expected_payee != computed_payee {
        return Err(TallyError::Generic(format!(
            "Authority mismatch: expected payee PDA {computed_payee} for authority {authority}, but got {expected_payee}"
        )));
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_payment_terms_parameters() {
        // Valid parameters
        assert!(validate_payment_terms_parameters(5_000_000, 2_592_000).is_ok());

        // Zero price
        assert!(validate_payment_terms_parameters(0, 2_592_000).is_err());

        // Period too short
        assert!(validate_payment_terms_parameters(5_000_000, 3600).is_err());

        // Negative period
        assert!(validate_payment_terms_parameters(5_000_000, -1).is_err());

        // Grace period validation removed - payment agreement extension layer handles this
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
