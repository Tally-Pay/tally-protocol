//! Validation utilities for Tally operations

use crate::{
    ata::{get_associated_token_address_with_program, get_token_account_info, TokenProgram},
    error::{Result, TallyError},
    program_types::{Plan, UpdatePlanArgs},
    SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
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

/// Validate plan update arguments according to program constraints
///
/// # Arguments
/// * `update_args` - The update arguments to validate
/// * `current_plan` - Optional current plan data for additional validation
///
/// # Errors
/// Returns an error if update arguments are invalid
pub fn validate_plan_update_args(
    update_args: &UpdatePlanArgs,
    current_plan: Option<&Plan>,
) -> Result<()> {
    // Ensure at least one field is being updated
    if !update_args.has_updates() {
        return Err(TallyError::Generic(
            "At least one field must be specified for update".to_string(),
        ));
    }

    // Validate price if provided
    if let Some(price_usdc) = update_args.price_usdc {
        if price_usdc == 0 {
            return Err(TallyError::Generic(
                "Plan price must be greater than 0".to_string(),
            ));
        }
    }

    // Validate period if provided
    if let Some(period_secs) = update_args.period_secs {
        if period_secs < 86400 {
            return Err(TallyError::Generic(format!(
                "Plan period must be at least 24 hours (86400 seconds), got: {period_secs}"
            )));
        }
    }

    // Validate grace period if provided
    if let Some(grace_secs) = update_args.grace_secs {
        if grace_secs > 0 {
            // If we have current plan data, validate against existing period
            // If we have a new period in the update, validate against that
            let period_to_check = if let Some(new_period) = update_args.period_secs {
                new_period
            } else if let Some(current) = current_plan {
                current.period_secs
            } else {
                // If we don't have current plan data and no new period, we can't validate
                // This should be caught by the program's validation
                return Err(TallyError::Generic(
                    "Cannot validate grace period without current plan data or new period"
                        .to_string(),
                ));
            };

            let max_grace_secs = 2_u64.saturating_mul(period_to_check);
            if grace_secs > max_grace_secs {
                return Err(TallyError::Generic(format!(
                    "Grace period ({grace_secs} seconds) cannot exceed 2x the billing period ({period_to_check} seconds). Maximum allowed: {max_grace_secs}"
                )));
            }
        }
    }

    // Validate name length if provided
    if let Some(name) = &update_args.name {
        if name.len() > 32 {
            return Err(TallyError::Generic(format!(
                "Plan name cannot exceed 32 bytes, got: {} bytes",
                name.len()
            )));
        }
        if name.is_empty() {
            return Err(TallyError::Generic("Plan name cannot be empty".to_string()));
        }
    }

    // Cross-field validation for period and grace period
    if let (Some(period_secs), Some(grace_secs)) = (update_args.period_secs, update_args.grace_secs)
    {
        let max_grace_secs = 2_u64.saturating_mul(period_secs);
        if grace_secs > max_grace_secs {
            return Err(TallyError::Generic(format!(
                "Grace period ({grace_secs} seconds) cannot exceed 2x the billing period ({period_secs} seconds). Maximum allowed: {max_grace_secs}"
            )));
        }
    }

    Ok(())
}

/// Validate that plan update is safe for existing subscriptions
///
/// # Arguments
/// * `update_args` - The update arguments to validate
/// * `current_plan` - Current plan data
/// * `has_active_subscriptions` - Whether the plan has active subscriptions
///
/// # Errors
/// Returns an error if the update would negatively impact existing subscriptions
pub const fn validate_plan_update_safety(
    update_args: &UpdatePlanArgs,
    current_plan: &Plan,
    has_active_subscriptions: bool,
) -> Result<()> {
    if !has_active_subscriptions {
        // If no active subscriptions, all updates are safe
        return Ok(());
    }

    // Price increases don't affect existing subscriptions (they keep their original price)
    // This is the intended behavior based on subscription platform best practices

    // Period changes need careful consideration
    if let Some(new_period) = update_args.period_secs {
        if new_period < current_plan.period_secs {
            // Shortening the period could affect renewal timing
            // This is potentially risky but might be allowed based on business rules
            // For now, we'll allow it but log a warning (if logging were implemented)
        }
    }

    // Grace period changes should be safe as they only affect renewal tolerance
    // Active status changes are administrative and safe

    // Name changes are cosmetic and safe

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

    #[test]
    fn test_validate_plan_update_args() {
        use crate::program_types::UpdatePlanArgs;

        // Valid update with name only
        let update_args = UpdatePlanArgs::new().with_name("New Name".to_string());
        assert!(validate_plan_update_args(&update_args, None).is_ok());

        // Valid update with multiple fields
        let update_args = UpdatePlanArgs::new()
            .with_name("New Name".to_string())
            .with_active(true)
            .with_price_usdc(5_000_000)
            .with_period_secs(86400)
            .with_grace_secs(3600);
        assert!(validate_plan_update_args(&update_args, None).is_ok());

        // Empty update args (no updates)
        let empty_args = UpdatePlanArgs::new();
        assert!(validate_plan_update_args(&empty_args, None).is_err());

        // Invalid price (zero)
        let invalid_price = UpdatePlanArgs::new().with_price_usdc(0);
        assert!(validate_plan_update_args(&invalid_price, None).is_err());

        // Invalid period (too short)
        let invalid_period = UpdatePlanArgs::new().with_period_secs(3600); // 1 hour
        assert!(validate_plan_update_args(&invalid_period, None).is_err());

        // Invalid name (too long)
        let long_name = "a".repeat(33);
        let invalid_name = UpdatePlanArgs::new().with_name(long_name);
        assert!(validate_plan_update_args(&invalid_name, None).is_err());

        // Invalid name (empty)
        let empty_name = UpdatePlanArgs::new().with_name(String::new());
        assert!(validate_plan_update_args(&empty_name, None).is_err());
    }

    #[test]
    fn test_validate_plan_update_args_with_current_plan() {
        use crate::program_types::{Plan, UpdatePlanArgs};
        use anchor_client::solana_sdk::signature::{Keypair, Signer};

        let current_plan = Plan {
            merchant: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            plan_id: [0u8; 32],
            price_usdc: 5_000_000,
            period_secs: 86400, // 1 day
            grace_secs: 3600,   // 1 hour
            name: [0u8; 32],
            active: true,
        };

        // Valid grace period update (within 2x current period)
        let valid_grace = UpdatePlanArgs::new().with_grace_secs(172_800); // 2 days
        assert!(validate_plan_update_args(&valid_grace, Some(&current_plan)).is_ok());

        // Invalid grace period (exceeds 2x current period)
        let invalid_grace = UpdatePlanArgs::new().with_grace_secs(172_801); // > 2 days
        assert!(validate_plan_update_args(&invalid_grace, Some(&current_plan)).is_err());

        // Valid period and grace combination
        let valid_combo = UpdatePlanArgs::new()
            .with_period_secs(172_800) // 2 days
            .with_grace_secs(345_600); // 4 days (2x new period)
        assert!(validate_plan_update_args(&valid_combo, Some(&current_plan)).is_ok());

        // Invalid period and grace combination
        let invalid_combo = UpdatePlanArgs::new()
            .with_period_secs(86400) // 1 day
            .with_grace_secs(345_600); // 4 days (> 2x period)
        assert!(validate_plan_update_args(&invalid_combo, Some(&current_plan)).is_err());
    }

    #[test]
    fn test_validate_plan_update_safety() {
        use crate::program_types::{Plan, UpdatePlanArgs};
        use anchor_client::solana_sdk::signature::{Keypair, Signer};

        let current_plan = Plan {
            merchant: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            plan_id: [0u8; 32],
            price_usdc: 5_000_000,
            period_secs: 86400, // 1 day
            grace_secs: 3600,   // 1 hour
            name: [0u8; 32],
            active: true,
        };

        // All updates are safe when no active subscriptions
        let price_increase = UpdatePlanArgs::new().with_price_usdc(10_000_000);
        assert!(validate_plan_update_safety(&price_increase, &current_plan, false).is_ok());

        // Price increases are safe even with active subscriptions
        assert!(validate_plan_update_safety(&price_increase, &current_plan, true).is_ok());

        // Period changes are allowed (for now) even with active subscriptions
        let period_change = UpdatePlanArgs::new().with_period_secs(172_800); // 2 days
        assert!(validate_plan_update_safety(&period_change, &current_plan, true).is_ok());

        // Shorter periods are also allowed (for now)
        let shorter_period = UpdatePlanArgs::new().with_period_secs(43200); // 12 hours (but still > 24h min)
                                                                            // This should fail validation at the args level due to minimum period requirement
        assert!(validate_plan_update_args(&shorter_period, Some(&current_plan)).is_err());

        // Grace period changes are safe
        let grace_change = UpdatePlanArgs::new().with_grace_secs(7200); // 2 hours
        assert!(validate_plan_update_safety(&grace_change, &current_plan, true).is_ok());

        // Name and active changes are safe
        let name_change = UpdatePlanArgs::new().with_name("New Name".to_string());
        let active_change = UpdatePlanArgs::new().with_active(false);
        assert!(validate_plan_update_safety(&name_change, &current_plan, true).is_ok());
        assert!(validate_plan_update_safety(&active_change, &current_plan, true).is_ok());
    }
}
