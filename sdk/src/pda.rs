//! Program Derived Address (PDA) computation utilities

use crate::{error::Result, program_id_string};
use anchor_client::solana_sdk::pubkey::Pubkey;

/// Compute the Payee PDA
///
/// # Arguments
/// * `authority` - The payee's authority pubkey
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
///
/// # Errors
/// Returns an error if the program ID cannot be parsed or PDA computation fails
pub fn payee(authority: &Pubkey) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(payee_with_program_id(authority, &program_id))
}

/// Compute the Payee PDA address only (without bump)
///
/// # Arguments
/// * `authority` - The payee's authority pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn payee_address(authority: &Pubkey) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(payee_address_with_program_id(authority, &program_id))
}

/// Compute the Payee PDA with custom program ID
///
/// # Arguments
/// * `authority` - The payee's authority pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn payee_with_program_id(authority: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[b"payee", authority.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the Payee PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `authority` - The payee's authority pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn payee_address_with_program_id(authority: &Pubkey, program_id: &Pubkey) -> Pubkey {
    payee_with_program_id(authority, program_id).0
}

/// Compute the `PaymentTerms` PDA
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as bytes
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_terms(payee: &Pubkey, terms_id: &[u8]) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(payment_terms_with_program_id(payee, terms_id, &program_id))
}

/// Compute the `PaymentTerms` PDA address only (without bump)
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as bytes
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_terms_address(payee: &Pubkey, terms_id: &[u8]) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(payment_terms_address_with_program_id(payee, terms_id, &program_id))
}

/// Compute the `PaymentTerms` PDA with custom program ID
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as bytes
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn payment_terms_with_program_id(
    payee: &Pubkey,
    terms_id: &[u8],
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let seeds = &[b"payment_terms", payee.as_ref(), terms_id];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the `PaymentTerms` PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as bytes
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn payment_terms_address_with_program_id(
    payee: &Pubkey,
    terms_id: &[u8],
    program_id: &Pubkey,
) -> Pubkey {
    payment_terms_with_program_id(payee, terms_id, program_id).0
}

/// Compute the `PaymentTerms` PDA from string identifier
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as string
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_terms_from_string(payee: &Pubkey, terms_id: &str) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(payment_terms_from_string_with_program_id(
        payee,
        terms_id,
        &program_id,
    ))
}

/// Compute the `PaymentTerms` PDA address from string identifier
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as string
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_terms_address_from_string(payee: &Pubkey, terms_id: &str) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(payment_terms_address_from_string_with_program_id(
        payee,
        terms_id,
        &program_id,
    ))
}

/// Compute the `PaymentTerms` PDA from string identifier with custom program ID
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as string
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn payment_terms_from_string_with_program_id(
    payee: &Pubkey,
    terms_id: &str,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    payment_terms_with_program_id(payee, terms_id.as_bytes(), program_id)
}

/// Compute the `PaymentTerms` PDA address from string identifier with custom program ID
///
/// # Arguments
/// * `payee` - The payee PDA pubkey
/// * `terms_id` - The payment terms identifier as string
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn payment_terms_address_from_string_with_program_id(
    payee: &Pubkey,
    terms_id: &str,
    program_id: &Pubkey,
) -> Pubkey {
    payment_terms_from_string_with_program_id(payee, terms_id, program_id).0
}

/// Compute the `PaymentAgreement` PDA
///
/// # Arguments
/// * `payment_terms` - The payment terms PDA pubkey
/// * `payer` - The payer's pubkey
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_agreement(payment_terms: &Pubkey, payer: &Pubkey) -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(payment_agreement_with_program_id(payment_terms, payer, &program_id))
}

/// Compute the `PaymentAgreement` PDA address only (without bump)
///
/// # Arguments
/// * `payment_terms` - The payment terms PDA pubkey
/// * `payer` - The payer's pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn payment_agreement_address(payment_terms: &Pubkey, payer: &Pubkey) -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(payment_agreement_address_with_program_id(
        payment_terms,
        payer,
        &program_id,
    ))
}

/// Compute the `PaymentAgreement` PDA with custom program ID
///
/// # Arguments
/// * `payment_terms` - The payment terms PDA pubkey
/// * `payer` - The payer's pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn payment_agreement_with_program_id(
    payment_terms: &Pubkey,
    payer: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let seeds = &[b"payment_agreement", payment_terms.as_ref(), payer.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the `PaymentAgreement` PDA address only (without bump) with custom program ID
///
/// # Arguments
/// * `payment_terms` - The payment terms PDA pubkey
/// * `payer` - The payer's pubkey
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The PDA address
#[must_use]
pub fn payment_agreement_address_with_program_id(
    payment_terms: &Pubkey,
    payer: &Pubkey,
    program_id: &Pubkey,
) -> Pubkey {
    payment_agreement_with_program_id(payment_terms, payer, program_id).0
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

/// Compute the global Delegate PDA
///
/// The protocol uses a single global delegate shared by all payees,
/// enabling users to subscribe to multiple payees with one token account.
///
/// # Returns
/// * `Ok((Pubkey, u8))` - The PDA address and bump seed
///
/// # Errors
/// Returns an error if the program ID cannot be parsed or PDA computation fails
pub fn delegate() -> Result<(Pubkey, u8)> {
    let program_id = program_id_string().parse()?;
    Ok(delegate_with_program_id(&program_id))
}

/// Compute the global Delegate PDA address only (without bump)
///
/// The protocol uses a single global delegate shared by all payees.
///
/// # Returns
/// * `Ok(Pubkey)` - The delegate PDA address
/// * `Err(TallyError)` - If PDA computation fails
pub fn delegate_address() -> Result<Pubkey> {
    let program_id = program_id_string().parse()?;
    Ok(delegate_address_with_program_id(&program_id))
}

/// Compute the global Delegate PDA with custom program ID
///
/// The protocol uses a single global delegate shared by all payees.
///
/// # Arguments
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `(Pubkey, u8)` - The PDA address and bump seed
#[must_use]
pub fn delegate_with_program_id(program_id: &Pubkey) -> (Pubkey, u8) {
    let seeds = &[b"delegate" as &[u8]];
    Pubkey::find_program_address(seeds, program_id)
}

/// Compute the global Delegate PDA address only (without bump) with custom program ID
///
/// The protocol uses a single global delegate shared by all payees.
///
/// # Arguments
/// * `program_id` - The program ID to use for PDA computation
///
/// # Returns
/// * `Pubkey` - The delegate PDA address
#[must_use]
pub fn delegate_address_with_program_id(program_id: &Pubkey) -> Pubkey {
    delegate_with_program_id(program_id).0
}




#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::{Keypair, Signer};
    use std::str::FromStr;

    #[test]
    fn test_payee_pda() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let (payee_pda, _bump) = payee(&authority).unwrap();

        // PDA should be different from authority
        assert_ne!(payee_pda, authority);

        // Should be deterministic
        let (payee_pda2, _) = payee(&authority).unwrap();
        assert_eq!(payee_pda, payee_pda2);
    }

    #[test]
    fn test_payment_terms_pda() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let terms_id = b"premium_payment_terms";

        let (terms_pda, _bump) = payment_terms(&payee, terms_id).unwrap();

        // Should be deterministic
        let (terms_pda2, _) = payment_terms(&payee, terms_id).unwrap();
        assert_eq!(terms_pda, terms_pda2);

        // Different terms IDs should produce different PDAs
        let (terms_pda3, _) = payment_terms(&payee, b"basic_payment_terms").unwrap();
        assert_ne!(terms_pda, terms_pda3);
    }

    #[test]
    fn test_payment_agreement_pda() {
        let payment_terms_pda = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let (agreement_pda, _bump) = payment_agreement(&payment_terms_pda, &payer).unwrap();

        // Should be deterministic
        let (agreement_pda2, _) = payment_agreement(&payment_terms_pda, &payer).unwrap();
        assert_eq!(agreement_pda, agreement_pda2);

        // Different payers should produce different PDAs
        let payer2 = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let (agreement_pda3, _) = payment_agreement(&payment_terms_pda, &payer2).unwrap();
        assert_ne!(agreement_pda, agreement_pda3);
    }

    #[test]
    fn test_payment_terms_string_functions() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let terms_id = "premium_payment_terms";

        let (pda1, bump1) = payment_terms_from_string(&payee, terms_id).unwrap();
        let (pda2, bump2) = payment_terms(&payee, terms_id.as_bytes()).unwrap();

        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);

        let addr1 = payment_terms_address_from_string(&payee, terms_id).unwrap();
        let addr2 = payment_terms_address(&payee, terms_id.as_bytes()).unwrap();

        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_address_only_functions() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee_addr = payee_address(&authority).unwrap();
        let (payee_pda, _) = payee(&authority).unwrap();
        assert_eq!(payee_addr, payee_pda);

        let terms_id = b"test_payment_terms";
        let terms_addr = payment_terms_address(&payee_addr, terms_id).unwrap();
        let (terms_pda, _) = payment_terms(&payee_addr, terms_id).unwrap();
        assert_eq!(terms_addr, terms_pda);

        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let agreement_addr = payment_agreement_address(&terms_addr, &payer).unwrap();
        let (agreement_pda, _) = payment_agreement(&terms_addr, &payer).unwrap();
        assert_eq!(agreement_addr, agreement_pda);
    }

    #[test]
    fn test_terms_id_raw_bytes() {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test short terms ID (uses raw bytes directly)
        let short_id = b"short";
        let (pda1, _) = payment_terms(&payee, short_id).unwrap();

        // Test same terms ID should produce same PDA (deterministic)
        let (pda2, _) = payment_terms(&payee, b"short").unwrap();
        assert_eq!(pda1, pda2);

        // Test different terms IDs should produce different PDAs
        let basic_id = b"basic";
        let (pda3, _) = payment_terms(&payee, basic_id).unwrap();
        assert_ne!(pda1, pda3);

        let other_id = b"other";
        let (pda5, _) = payment_terms(&payee, other_id).unwrap();
        assert_ne!(pda1, pda5);
        assert_ne!(pda3, pda5);

        // Test longer terms ID (but within reasonable limits for PDA generation)
        let long_id = b"premium_monthly";
        let (pda4, _) = payment_terms(&payee, long_id).unwrap();

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
        let (delegate_pda, _bump) = delegate().unwrap();

        // Should be deterministic
        let (delegate_pda2, _) = delegate().unwrap();
        assert_eq!(delegate_pda, delegate_pda2);

        // Test address-only function
        let delegate_addr = delegate_address().unwrap();
        assert_eq!(delegate_pda, delegate_addr);

        // Global delegate: same PDA regardless of payee
        // This is intentional - all payees share the same delegate
        let (delegate_pda3, _) = delegate().unwrap();
        assert_eq!(delegate_pda, delegate_pda3);
    }

    #[test]
    fn test_program_id_from_env() {
        // Test requires TALLY_PROGRAM_ID to be set
        // In CI/local testing, this must be set before running tests
        let program_id_str = std::env::var("TALLY_PROGRAM_ID")
            .expect("TALLY_PROGRAM_ID must be set for tests. \
                     Example: export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111");

        let expected_program_id = Pubkey::from_str(&program_id_str).unwrap();
        let actual_program_id = program_id_string().parse().unwrap();

        assert_eq!(expected_program_id, actual_program_id,
                   "Program ID from program_id_string() should match TALLY_PROGRAM_ID env var");
    }
}
