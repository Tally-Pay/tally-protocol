//! Program Derived Address (PDA) computation utilities

use crate::{error::Result, program_id_string};
use anchor_lang::prelude::Pubkey;

/// Compute the Merchant PDA
///
/// # Arguments
/// * `authority` - The merchant's authority pubkey
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
///
/// # Errors
/// Returns an error if the program ID cannot be parsed or PDA computation fails
pub fn merchant(authority: &Pubkey) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(merchant_with_program_id(authority, &program_id))
}

/// Compute the Merchant PDA address only (without bump)
///
/// # Arguments
/// * `authority` - The merchant's authority pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn merchant_address(authority: &Pubkey) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(merchant_address_with_program_id(authority, &program_id))
}

/// Compute the Merchant PDA with custom program ID
///
/// # Arguments
/// * `authority` - The merchant's authority pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
#[must_use]
pub fn merchant_with_program_id(authority: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[b"merchant", authority.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Merchant PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `authority` - The merchant's authority pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn merchant_address_with_program_id(authority: &Pubkey, program_id: &Pubkey) -> Pubkey {
    merchant_with_program_id(authority, program_id).0
}

/// Compute the Plan PDA
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as bytes (uses raw bytes to match Anchor constraint)
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn plan(merchant: &Pubkey, plan_id: &[u8]) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(plan_with_program_id(merchant, plan_id, &program_id))
}

/// Compute the Plan PDA address only (without bump)
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as bytes
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn plan_address(merchant: &Pubkey, plan_id: &[u8]) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(plan_address_with_program_id(merchant, plan_id, &program_id))
}

/// Compute the Plan PDA with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as bytes
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn plan_with_program_id(
    merchant: &Pubkey,
    plan_id: &[u8],
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let seeds = &[b"plan", merchant.as_ref(), plan_id];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Plan PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as bytes
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn plan_address_with_program_id(
    merchant: &Pubkey,
    plan_id: &[u8],
    program_id: &Pubkey,
) -> Pubkey {
    plan_with_program_id(merchant, plan_id, program_id).0
}

/// Compute the Plan PDA from string identifier
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as string
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn plan_from_string(merchant: &Pubkey, plan_id: &str) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(plan_from_string_with_program_id(
        merchant,
        plan_id,
        &program_id,
    ))
}

/// Compute the Plan PDA address from string identifier
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as string
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn plan_address_from_string(merchant: &Pubkey, plan_id: &str) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(plan_address_from_string_with_program_id(
        merchant,
        plan_id,
        &program_id,
    ))
}

/// Compute the Plan PDA from string identifier with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as string
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn plan_from_string_with_program_id(
    merchant: &Pubkey,
    plan_id: &str,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    plan_with_program_id(merchant, plan_id.as_bytes(), program_id)
}

/// Compute the Plan PDA address from string identifier with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `plan_id` - The plan identifier as string
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn plan_address_from_string_with_program_id(
    merchant: &Pubkey,
    plan_id: &str,
    program_id: &Pubkey,
) -> Pubkey {
    plan_from_string_with_program_id(merchant, plan_id, program_id).0
}

/// Compute the Subscription PDA
///
/// # Arguments
/// * `plan` - The plan PDA pubkey
/// * `subscriber` - The subscriber's pubkey
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn subscription(plan: &Pubkey, subscriber: &Pubkey) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(subscription_with_program_id(plan, subscriber, &program_id))
}

/// Compute the Subscription PDA address only (without bump)
///
/// # Arguments
/// * `plan` - The plan PDA pubkey
/// * `subscriber` - The subscriber's pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn subscription_address(plan: &Pubkey, subscriber: &Pubkey) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(subscription_address_with_program_id(
        plan,
        subscriber,
        &program_id,
    ))
}

/// Compute the Subscription PDA with custom program ID
///
/// # Arguments
/// * `plan` - The plan PDA pubkey
/// * `subscriber` - The subscriber's pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn subscription_with_program_id(
    plan: &Pubkey,
    subscriber: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let seeds = &[b"subscription", plan.as_ref(), subscriber.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Subscription PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `plan` - The plan PDA pubkey
/// * `subscriber` - The subscriber's pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn subscription_address_with_program_id(
    plan: &Pubkey,
    subscriber: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    subscription_with_program_id(plan, subscriber, program_id).0
}

/// Compute the Config PDA
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn config() -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(config_with_program_id(&program_id))
}

/// Compute the Config PDA address only (without bump)
///
/// # Returns
/// * `Ok(Pubkey)` - The config PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn config_address() -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(config_address_with_program_id(&program_id))
}

/// Compute the Config PDA with custom program ID
///
/// # Arguments
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn config_with_program_id(program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[b"config" as &[u8]];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Config PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The config PDA address
#[must_use]
pub fn config_address_with_program_id(program_id: &Pubkey) -> Pubkey {
    config_with_program_id(program_id).0
}

/// Compute the Delegate PDA
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn delegate(merchant: &Pubkey) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(delegate_with_program_id(merchant, &program_id))
}

/// Compute the Delegate PDA address only (without bump)
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The delegate PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn delegate_address(merchant: &Pubkey) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(delegate_address_with_program_id(merchant, &program_id))
}

/// Compute the Delegate PDA with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn delegate_with_program_id(merchant: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[b"delegate", merchant.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Delegate PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `merchant` - The merchant PDA pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The delegate PDA address
#[must_use]
pub fn delegate_address_with_program_id(merchant: &Pubkey, program_id: &Pubkey) -> Pubkey {
    delegate_with_program_id(merchant, program_id).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::{Keypair, Signer};
    use std::str::FromStr;

    #[test]
    fn test_merchant_pda() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let (merchant_pda, _bump) = merchant(&authority).unwrap();

        // PDA should be different from authority
        assert_ne!(merchant_pda, authority);

        // Should be deterministic
        let (merchant_pda2, _) = merchant(&authority).unwrap();
        assert_eq!(merchant_pda, merchant_pda2);
    }

    #[test]
    fn test_plan_pda() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan_id = b"premium_plan";

        let (plan_pda, _bump) = plan(&merchant, plan_id).unwrap();

        // Should be deterministic
        let (plan_pda2, _) = plan(&merchant, plan_id).unwrap();
        assert_eq!(plan_pda, plan_pda2);

        // Different plan IDs should produce different PDAs
        let (plan_pda3, _) = plan(&merchant, b"basic_plan").unwrap();
        assert_ne!(plan_pda, plan_pda3);
    }

    #[test]
    fn test_subscription_pda() {
        let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let (sub_pda, _bump) = subscription(&plan, &subscriber).unwrap();

        // Should be deterministic
        let (sub_pda2, _) = subscription(&plan, &subscriber).unwrap();
        assert_eq!(sub_pda, sub_pda2);

        // Different subscribers should produce different PDAs
        let subscriber2 = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let (sub_pda3, _) = subscription(&plan, &subscriber2).unwrap();
        assert_ne!(sub_pda, sub_pda3);
    }

    #[test]
    fn test_plan_string_functions() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan_id = "premium_plan";

        let (pda1, bump1) = plan_from_string(&merchant, plan_id).unwrap();
        let (pda2, bump2) = plan(&merchant, plan_id.as_bytes()).unwrap();

        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);

        let addr1 = plan_address_from_string(&merchant, plan_id).unwrap();
        let addr2 = plan_address(&merchant, plan_id.as_bytes()).unwrap();

        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_address_only_functions() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant_addr = merchant_address(&authority).unwrap();
        let (merchant_pda, _) = merchant(&authority).unwrap();
        assert_eq!(merchant_addr, merchant_pda);

        let plan_id = b"test_plan";
        let plan_addr = plan_address(&merchant_addr, plan_id).unwrap();
        let (plan_pda, _) = plan(&merchant_addr, plan_id).unwrap();
        assert_eq!(plan_addr, plan_pda);

        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let sub_addr = subscription_address(&plan_addr, &subscriber).unwrap();
        let (sub_pda, _) = subscription(&plan_addr, &subscriber).unwrap();
        assert_eq!(sub_addr, sub_pda);
    }

    #[test]
    fn test_plan_id_raw_bytes() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test short plan ID (uses raw bytes directly)
        let short_id = b"short";
        let (pda1, _) = plan(&merchant, short_id).unwrap();

        // Test same plan ID should produce same PDA (deterministic)
        let (pda2, _) = plan(&merchant, b"short").unwrap();
        assert_eq!(pda1, pda2);

        // Test different plan IDs should produce different PDAs
        let basic_id = b"basic";
        let (pda3, _) = plan(&merchant, basic_id).unwrap();
        assert_ne!(pda1, pda3);

        let other_id = b"other";
        let (pda5, _) = plan(&merchant, other_id).unwrap();
        assert_ne!(pda1, pda5);
        assert_ne!(pda3, pda5);

        // Test longer plan ID (raw bytes, no truncation)
        let long_id = b"premium_monthly_subscription";
        let (pda4, _) = plan(&merchant, long_id).unwrap();

        // Should not panic and should produce a valid PDA
        assert_ne!(pda4, pda1);
        assert_ne!(pda4, pda3);
        assert_ne!(pda4, pda5);
    }

    #[test]
    fn test_config_pda() {
        let (config_pda, _bump) = config().unwrap();

        // Should be deterministic
        let (config_pda2, _) = config().unwrap();
        assert_eq!(config_pda, config_pda2);

        // Test address-only function
        let config_addr = config_address().unwrap();
        assert_eq!(config_pda, config_addr);
    }

    #[test]
    fn test_delegate_pda() {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let (delegate_pda, _bump) = delegate(&merchant).unwrap();

        // Should be deterministic
        let (delegate_pda2, _) = delegate(&merchant).unwrap();
        assert_eq!(delegate_pda, delegate_pda2);

        // Test address-only function
        let delegate_addr = delegate_address(&merchant).unwrap();
        assert_eq!(delegate_pda, delegate_addr);

        // Different merchants should produce different delegate PDAs
        let merchant2 = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let (delegate_pda3, _) = delegate(&merchant2).unwrap();
        assert_ne!(delegate_pda, delegate_pda3);
    }

    #[test]
    fn test_known_program_id() {
        // Test with the actual program ID to ensure consistency
        let program_id_str = "Fwrs8tRRtw8HwmQZFS3XRRVcKBQhe1nuZ5heB4FgySXV";
        let expected_program_id = Pubkey::from_str(program_id_str).unwrap();
        let actual_program_id = program_id_string().parse().unwrap();

        assert_eq!(expected_program_id, actual_program_id);
    }
}
