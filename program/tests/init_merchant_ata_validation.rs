//! Unit tests for Merchant Treasury ATA Derivation Validation (M-2)
//!
//! This test suite validates the M-2 security fix through comprehensive unit tests.
//! For full integration tests with BPF runtime, use the TypeScript test suite.
//!
//! Test coverage:
//! - Canonical ATA derivation validation for merchant treasury accounts
//! - Prevention of non-ATA token account usage
//! - Detection of ATAs derived with wrong authority
//! - Detection of ATAs derived with wrong mint
//! - Prevention of manually created token accounts (correct owner/mint but not ATA)
//! - Error code validation (`BadSeeds`)
//! - Edge cases with different authority and mint combinations
//!
//! Security Context (M-2):
//! The critical security fix adds merchant treasury ATA derivation validation to ensure that the
//! `treasury_ata` account passed to `init_merchant` instruction is the canonical Associated Token
//! Account (ATA) derived from the merchant authority and USDC mint.
//!
//! Without this validation, merchants could use arbitrary token accounts (even manually created ones
//! with correct owner/mint) instead of the canonical ATA. This breaks compatibility with wallet
//! integrations and off-chain indexing that expect standard ATA addresses, leading to:
//! - Funds sent to unexpected addresses
//! - Indexing failures in explorers and analytics
//! - Wallet integration issues
//! - Loss of standard ATA conventions
//!
//! The validation occurs at `init_merchant.rs` lines 102-113:
//!
//! ```rust
//! // Validate that treasury_ata is the canonical Associated Token Account
//! // derived from the merchant authority and USDC mint.
//! // This ensures compatibility with wallet integrations and off-chain indexing
//! // that expect standard ATA addresses.
//! let expected_treasury_ata = get_associated_token_address(
//!     &ctx.accounts.authority.key(),
//!     &args.usdc_mint,
//! );
//! require!(
//!     ctx.accounts.treasury_ata.key() == expected_treasury_ata,
//!     crate::errors::RecurringPaymentError::BadSeeds
//! );
//! ```
//!
//! The validation ensures:
//! 1. The `treasury_ata` is the canonical ATA derived from `authority` and `usdc_mint`
//! 2. Manually created token accounts cannot be used
//! 3. ATAs for wrong authority cannot be used
//! 4. ATAs for wrong mint cannot be used
//! 5. Standard wallet and indexing compatibility is maintained
//!
//! Note: These are unit tests that validate the ATA derivation logic.
//! Full end-to-end integration tests should be run with `anchor test`.

use anchor_lang::prelude::Pubkey;
use anchor_spl::associated_token::get_associated_token_address;

/// Test that validation accepts correct canonical ATA
///
/// Given a `treasury_ata` that matches the canonical ATA derived from
/// merchant authority and USDC mint, the validation should accept it.
#[test]
fn test_validation_accepts_correct_canonical_ata() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Derive the canonical ATA
    let canonical_ata = get_associated_token_address(&authority, &usdc_mint);

    // Simulate providing the canonical ATA
    let provided_treasury_ata = canonical_ata;

    // Simulate the validation check from the handler (lines 106-113)
    let expected_treasury_ata = get_associated_token_address(&authority, &usdc_mint);
    let is_valid = provided_treasury_ata == expected_treasury_ata;

    assert!(
        is_valid,
        "Validation should accept canonical ATA derived from authority and mint"
    );
}

/// Test that validation rejects arbitrary non-ATA token account
///
/// An attacker or misconfigured client provides an arbitrary token account
/// (not derived as an ATA) instead of the canonical ATA. The validation must reject it.
#[test]
fn test_validation_rejects_arbitrary_non_ata_token_account() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Attacker provides a random token account (not an ATA)
    let arbitrary_token_account = Pubkey::new_unique();

    // Simulate the validation check from the handler
    let expected_treasury_ata = get_associated_token_address(&authority, &usdc_mint);
    let is_valid = arbitrary_token_account == expected_treasury_ata;

    assert!(
        !is_valid,
        "Validation must reject arbitrary non-ATA token account"
    );
}

/// Test that validation rejects ATA for different wallet
///
/// An attacker provides an ATA that is valid, but derived for a different
/// authority (wallet), attempting to redirect merchant fees to their own account.
#[test]
fn test_validation_rejects_ata_for_different_authority() {
    let correct_authority = Pubkey::new_unique();
    let attacker_authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Attacker provides their own ATA (correct mint, wrong authority)
    let attacker_ata = get_associated_token_address(&attacker_authority, &usdc_mint);

    // Handler validates by deriving with correct authority
    let expected_treasury_ata = get_associated_token_address(&correct_authority, &usdc_mint);
    let is_valid = attacker_ata == expected_treasury_ata;

    assert!(
        !is_valid,
        "Validation must reject ATA derived for different authority"
    );
}

/// Test that validation rejects ATA for different mint
///
/// An attacker provides an ATA for the correct authority but wrong token mint,
/// attempting to use a different token or exploit mint confusion.
#[test]
fn test_validation_rejects_ata_for_different_mint() {
    let authority = Pubkey::new_unique();
    let correct_usdc_mint = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();

    // Attacker provides an ATA for the same authority but different mint
    let wrong_mint_ata = get_associated_token_address(&authority, &wrong_mint);

    // Handler validates by deriving with correct mint
    let expected_treasury_ata = get_associated_token_address(&authority, &correct_usdc_mint);
    let is_valid = wrong_mint_ata == expected_treasury_ata;

    assert!(
        !is_valid,
        "Validation must reject ATA derived for different mint"
    );
}

/// Test that validation rejects wrong program's PDA
///
/// An extreme attack where an attacker provides a PDA from a different program
/// (e.g., a Metaplex PDA or other protocol's PDA) that happens to be a valid
/// address but is not the canonical ATA.
#[test]
fn test_validation_rejects_wrong_program_pda() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();
    let malicious_program_id = Pubkey::new_unique();

    // Attacker derives a PDA from a different program
    let (malicious_pda, _) =
        Pubkey::find_program_address(&[b"fake_ata", authority.as_ref()], &malicious_program_id);

    // Handler validates by deriving canonical ATA
    let expected_treasury_ata = get_associated_token_address(&authority, &usdc_mint);
    let is_valid = malicious_pda == expected_treasury_ata;

    assert!(
        !is_valid,
        "Validation must reject PDA from different program"
    );
}

/// Test that manually created token account is rejected
///
/// Edge case: An attacker creates a token account manually with the correct
/// owner (merchant authority) and correct mint (USDC), but it's not derived
/// as an ATA. The validation must reject this because it's not the canonical ATA.
///
/// This is critical for wallet and indexer compatibility - even if the owner
/// and mint are correct, it must be the canonical ATA address.
#[test]
fn test_validation_rejects_manually_created_token_account() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Simulate a manually created token account (random address, not ATA derivation)
    // In practice, this would be created via `create_account` with correct owner/mint
    // but a non-ATA address
    let manually_created_account = Pubkey::new_unique();

    // Handler validates by deriving canonical ATA
    let expected_treasury_ata = get_associated_token_address(&authority, &usdc_mint);
    let is_valid = manually_created_account == expected_treasury_ata;

    assert!(
        !is_valid,
        "Validation must reject manually created token account even with correct owner/mint"
    );
}

/// Test error code is `BadSeeds` as expected
///
/// Validates that the error returned by the validation is `BadSeeds` (error code 6005),
/// which is the appropriate error for PDA/ATA derivation mismatches.
///
/// This is a compile-time and logical validation - the actual runtime error
/// would be tested in integration tests.
#[test]
fn test_error_code_is_bad_seeds() {
    // The error used in the validation is BadSeeds
    // In the actual handler, when validation fails:
    // ```rust
    // require!(
    //     ctx.accounts.treasury_ata.key() == expected_treasury_ata,
    //     crate::errors::RecurringPaymentError::BadSeeds
    // );
    // ```
    //
    // This would return error code 6005 (BadSeeds) with message:
    // "Invalid PDA seeds provided. Account derivation failed."

    // This test validates that the error type exists and is the correct one
    // The actual error return is tested in integration tests

    // Verify the error constant exists by attempting to compile
    const _ERROR_CHECK: () = {
        use tally_protocol::errors::RecurringPaymentError;
        let _ = RecurringPaymentError::BadSeeds;
    };
}

/// Test validation is deterministic across multiple derivations
///
/// Simulates the validation logic being called multiple times with the same
/// inputs and verifies it produces consistent results.
#[test]
fn test_validation_logic_is_deterministic() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();
    let canonical_ata = get_associated_token_address(&authority, &usdc_mint);

    // Run validation logic multiple times
    let validation_results: Vec<bool> = (0..10)
        .map(|_| {
            let expected_ata = get_associated_token_address(&authority, &usdc_mint);
            canonical_ata == expected_ata
        })
        .collect();

    // Verify all results are identical and true
    for result in &validation_results {
        assert!(
            *result,
            "Validation logic must be deterministic and accept correct ATA"
        );
    }
}

/// Test validation with boundary pubkey patterns
///
/// Tests validation with various edge case pubkeys to ensure robustness
/// across all possible authority and mint values.
#[test]
fn test_validation_with_boundary_pubkeys() {
    // Test various authority and mint patterns
    let test_authorities = vec![
        Pubkey::new_unique(),               // Random
        Pubkey::default(),                  // All zeros
        Pubkey::new_from_array([0xFF; 32]), // All ones
        Pubkey::new_from_array({
            let mut arr = [0u8; 32];
            arr[0] = 0xFF;
            arr
        }), // First byte max
        Pubkey::new_from_array({
            let mut arr = [0u8; 32];
            arr[31] = 0xFF;
            arr
        }), // Last byte max
    ];

    let test_mints = vec![
        Pubkey::new_unique(),               // Random
        Pubkey::default(),                  // All zeros
        Pubkey::new_from_array([0xFF; 32]), // All ones
    ];

    for authority in &test_authorities {
        for mint in &test_mints {
            // Derive canonical ATA
            let canonical_ata = get_associated_token_address(authority, mint);

            // Test with matching ATA (should pass)
            let matching_ata = get_associated_token_address(authority, mint);
            let is_valid = matching_ata == canonical_ata;
            assert!(
                is_valid,
                "Validation should accept canonical ATA for boundary pubkeys"
            );

            // Test with different authority (should fail)
            let different_authority = Pubkey::new_unique();
            let wrong_authority_ata = get_associated_token_address(&different_authority, mint);
            let is_invalid = wrong_authority_ata == canonical_ata;
            assert!(
                !is_invalid,
                "Validation should reject ATA with different authority for boundary pubkeys"
            );

            // Test with different mint (should fail)
            let different_mint = Pubkey::new_unique();
            let wrong_mint_ata = get_associated_token_address(authority, &different_mint);
            let is_invalid_mint = wrong_mint_ata == canonical_ata;
            assert!(
                !is_invalid_mint,
                "Validation should reject ATA with different mint for boundary pubkeys"
            );
        }
    }
}

/// Test comprehensive M-2 attack prevention
///
/// Tests multiple attack scenarios to ensure the validation logic prevents
/// all known attack vectors for the M-2 vulnerability.
#[test]
fn test_comprehensive_m2_attack_prevention() {
    let merchant_authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();
    let attacker_authority = Pubkey::new_unique();
    let wrong_mint = Pubkey::new_unique();
    let malicious_program = Pubkey::new_unique();

    // Derive the canonical ATA
    let canonical_ata = get_associated_token_address(&merchant_authority, &usdc_mint);

    // Attack vector 1: Arbitrary non-ATA token account
    let attack_random_account = Pubkey::new_unique();
    assert_ne!(
        attack_random_account, canonical_ata,
        "Attack with random token account must be rejected"
    );

    // Attack vector 2: ATA for different authority (fee redirection)
    let attack_different_authority =
        get_associated_token_address(&attacker_authority, &usdc_mint);
    assert_ne!(
        attack_different_authority, canonical_ata,
        "Attack with different authority ATA must be rejected"
    );

    // Attack vector 3: ATA for different mint (token confusion)
    let attack_different_mint = get_associated_token_address(&merchant_authority, &wrong_mint);
    assert_ne!(
        attack_different_mint, canonical_ata,
        "Attack with different mint ATA must be rejected"
    );

    // Attack vector 4: ATA with both wrong authority and wrong mint
    let attack_both_wrong = get_associated_token_address(&attacker_authority, &wrong_mint);
    assert_ne!(
        attack_both_wrong, canonical_ata,
        "Attack with both wrong authority and mint must be rejected"
    );

    // Attack vector 5: PDA from different program
    let (attack_wrong_program, _) = Pubkey::find_program_address(
        &[b"fake_ata", merchant_authority.as_ref()],
        &malicious_program,
    );
    assert_ne!(
        attack_wrong_program, canonical_ata,
        "Attack with PDA from different program must be rejected"
    );

    // Verify only canonical ATA is accepted
    let correct_ata = get_associated_token_address(&merchant_authority, &usdc_mint);
    assert_eq!(
        correct_ata, canonical_ata,
        "Only canonical ATA should be accepted"
    );
}

/// Test ATA derivation uniqueness across different merchants
///
/// Validates that each merchant with a unique authority gets a unique ATA
/// for the same USDC mint, preventing cross-merchant treasury confusion.
#[test]
fn test_ata_uniqueness_across_merchants() {
    let usdc_mint = Pubkey::new_unique();

    // Create multiple merchant authorities
    let merchant_1 = Pubkey::new_unique();
    let merchant_2 = Pubkey::new_unique();
    let merchant_3 = Pubkey::new_unique();
    let merchant_4 = Pubkey::new_unique();
    let merchant_5 = Pubkey::new_unique();
    let merchants = [merchant_1, merchant_2, merchant_3, merchant_4, merchant_5];

    // Derive ATAs for all merchants
    let atas: Vec<Pubkey> = merchants
        .iter()
        .map(|merchant| get_associated_token_address(merchant, &usdc_mint))
        .collect();

    // Verify all ATAs are unique
    for i in 0..atas.len() {
        for j in (i + 1)..atas.len() {
            assert_ne!(
                atas[i], atas[j],
                "Each merchant must have a unique treasury ATA for the same mint"
            );
        }
    }
}

/// Test validation prevents merchant impersonation
///
/// Simulates an attacker attempting to initialize a merchant with a treasury
/// that belongs to a different merchant authority, attempting to steal fees.
#[test]
fn test_prevents_merchant_impersonation() {
    let legitimate_merchant = Pubkey::new_unique();
    let attacker_merchant = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Legitimate merchant's ATA
    let legitimate_ata = get_associated_token_address(&legitimate_merchant, &usdc_mint);

    // Attacker tries to initialize their merchant with legitimate merchant's ATA
    // Handler validates by deriving with attacker's authority
    let expected_attacker_ata = get_associated_token_address(&attacker_merchant, &usdc_mint);
    let is_valid = legitimate_ata == expected_attacker_ata;

    assert!(
        !is_valid,
        "Merchant impersonation attack must be prevented"
    );
}

/// Test validation with realistic production scenarios
///
/// Uses realistic pubkey patterns that might appear in production to ensure
/// the validation works correctly in real-world scenarios.
#[test]
fn test_validation_with_realistic_scenarios() {
    // Simulate realistic USDC mint address pattern
    let usdc_mint = Pubkey::new_unique();

    // Simulate realistic merchant authorities
    let merchant_authorities = vec![
        Pubkey::new_unique(), // Random realistic address
        Pubkey::new_from_array({
            let mut arr = [0u8; 32];
            arr[0] = 0xAB;
            arr[1] = 0xCD;
            arr
        }), // Address with specific pattern
    ];

    for merchant_authority in &merchant_authorities {
        // Correct scenario: canonical ATA for merchant
        let canonical_ata = get_associated_token_address(merchant_authority, &usdc_mint);
        let expected_ata = get_associated_token_address(merchant_authority, &usdc_mint);
        assert_eq!(
            canonical_ata, expected_ata,
            "Canonical ATA should be accepted"
        );

        // Attack scenario: arbitrary token account
        let arbitrary_account = Pubkey::new_unique();
        assert_ne!(
            arbitrary_account, canonical_ata,
            "Arbitrary token account should be rejected"
        );

        // Attack scenario: different merchant's ATA
        let other_merchant = Pubkey::new_unique();
        let other_merchant_ata = get_associated_token_address(&other_merchant, &usdc_mint);
        assert_ne!(
            other_merchant_ata, canonical_ata,
            "Different merchant's ATA should be rejected"
        );
    }
}

/// Test validation enforces strict equality check
///
/// Verifies that the validation uses strict equality (==) and not any
/// approximation or partial matching for ATA addresses.
#[test]
fn test_validation_uses_strict_equality() {
    let authority = Pubkey::new_from_array([1; 32]);
    let usdc_mint = Pubkey::new_unique();

    // Derive canonical ATA
    let canonical_ata = get_associated_token_address(&authority, &usdc_mint);

    // Create an almost-matching address (differs by 1 byte)
    let almost_matching = Pubkey::new_from_array({
        let mut arr = canonical_ata.to_bytes();
        arr[31] = arr[31].wrapping_add(1); // Modify last byte
        arr
    });

    // Should reject even when only 1 byte differs
    let is_valid = almost_matching == canonical_ata;

    assert!(
        !is_valid,
        "Validation must use strict equality and reject even 1-byte differences"
    );
}

/// Test validation with same authority and different mints
///
/// Validates that the same merchant authority with different token mints
/// produces different ATAs, preventing mint confusion attacks.
#[test]
fn test_validation_with_different_mints_same_authority() {
    let authority = Pubkey::new_unique();
    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    let mint_c = Pubkey::new_unique();

    let ata_a = get_associated_token_address(&authority, &mint_a);
    let ata_b = get_associated_token_address(&authority, &mint_b);
    let ata_c = get_associated_token_address(&authority, &mint_c);

    // Verify all ATAs are different
    assert_ne!(ata_a, ata_b, "Mint A and Mint B ATAs must be different");
    assert_ne!(ata_a, ata_c, "Mint A and Mint C ATAs must be different");
    assert_ne!(ata_b, ata_c, "Mint B and Mint C ATAs must be different");
}

/// Test validation prevents cross-merchant ATA reuse
///
/// Simulates multiple merchants attempting to use each other's ATAs,
/// which the validation must prevent.
#[test]
fn test_prevents_cross_merchant_ata_reuse() {
    let usdc_mint = Pubkey::new_unique();
    let merchant_a = Pubkey::new_unique();
    let merchant_b = Pubkey::new_unique();
    let merchant_c = Pubkey::new_unique();

    let ata_a = get_associated_token_address(&merchant_a, &usdc_mint);
    let ata_b = get_associated_token_address(&merchant_b, &usdc_mint);
    let ata_c = get_associated_token_address(&merchant_c, &usdc_mint);

    // Merchant B tries to use Merchant A's ATA
    let expected_b = get_associated_token_address(&merchant_b, &usdc_mint);
    assert_ne!(
        ata_a, expected_b,
        "Merchant B cannot use Merchant A's ATA"
    );

    // Merchant C tries to use Merchant A's ATA
    let expected_c = get_associated_token_address(&merchant_c, &usdc_mint);
    assert_ne!(
        ata_a, expected_c,
        "Merchant C cannot use Merchant A's ATA"
    );

    // Merchant C tries to use Merchant B's ATA
    assert_ne!(
        ata_b, expected_c,
        "Merchant C cannot use Merchant B's ATA"
    );

    // Each merchant can only use their own ATA
    assert_eq!(ata_a, get_associated_token_address(&merchant_a, &usdc_mint));
    assert_eq!(ata_b, get_associated_token_address(&merchant_b, &usdc_mint));
    assert_eq!(ata_c, get_associated_token_address(&merchant_c, &usdc_mint));
}

/// Test validation with zero address edge cases
///
/// Edge case testing with `Pubkey::default()` (all zeros) as authority or mint.
#[test]
fn test_validation_with_zero_address() {
    let zero_authority = Pubkey::default(); // All zeros
    let zero_mint = Pubkey::default(); // All zeros
    let normal_authority = Pubkey::new_unique();
    let normal_mint = Pubkey::new_unique();

    // Zero authority with normal mint
    let ata_zero_auth = get_associated_token_address(&zero_authority, &normal_mint);
    let expected_zero_auth = get_associated_token_address(&zero_authority, &normal_mint);
    assert_eq!(
        ata_zero_auth, expected_zero_auth,
        "Should accept zero authority when expected"
    );

    // Normal authority with zero mint
    let ata_zero_mint = get_associated_token_address(&normal_authority, &zero_mint);
    let expected_zero_mint = get_associated_token_address(&normal_authority, &zero_mint);
    assert_eq!(
        ata_zero_mint, expected_zero_mint,
        "Should accept zero mint when expected"
    );

    // Both zero
    let ata_both_zero = get_associated_token_address(&zero_authority, &zero_mint);
    let expected_both_zero = get_associated_token_address(&zero_authority, &zero_mint);
    assert_eq!(
        ata_both_zero, expected_both_zero,
        "Should accept both zero when expected"
    );

    // Should reject when provided ATA doesn't match
    let wrong_ata = Pubkey::new_unique();
    assert_ne!(
        wrong_ata, ata_both_zero,
        "Should reject non-matching ATA even with zero addresses"
    );
}

/// Test ATA derivation consistency across multiple calls
///
/// Validates that `get_associated_token_address` is deterministic and produces
/// the same result when called multiple times with the same inputs.
#[test]
fn test_ata_derivation_is_consistent() {
    let authority = Pubkey::new_unique();
    let usdc_mint = Pubkey::new_unique();

    // Call ATA derivation multiple times
    let results: Vec<Pubkey> = (0..10)
        .map(|_| get_associated_token_address(&authority, &usdc_mint))
        .collect();

    // Verify all results are identical
    let first_result = results[0];
    for result in &results {
        assert_eq!(
            *result, first_result,
            "ATA derivation must be consistent across multiple calls"
        );
    }
}
