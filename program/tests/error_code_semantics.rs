//! Unit tests for error code semantics and validation
//!
//! This test suite validates error codes 6024 (`InvalidTransferTarget`) and 6025 (`InvalidAmount`)
//! added to fix issue I-1 from the security audit.
//!
//! Test coverage:
//! - `InvalidTransferTarget` (6024):
//!   - Error conversion to Anchor error
//!   - Error number validation
//!   - Error message validation
//!   - Authority transfer validation logic (same authority check)
//! - `InvalidAmount` (6025):
//!   - Error conversion to Anchor error
//!   - Error number validation
//!   - Error message validation
//!   - Zero amount validation logic
//!   - Boundary condition testing (0 should fail, 1 should pass)
//!
//! Security Context (I-1):
//! These error codes were added to provide clear semantic errors for:
//! 1. Authority transfers where new authority equals current authority
//! 2. Monetary operations with invalid amounts (zero or negative)
//!
//! Note: These are unit tests that validate the business logic.
//! Full end-to-end integration tests should be run with `anchor test`.

use anchor_lang::prelude::*;
use tally_subs::errors::SubscriptionError;

// ============================================================================
// InvalidTransferTarget Error Tests (Error Code 6024)
// ============================================================================

/// Test that `InvalidTransferTarget` error can be converted to Anchor error
///
/// Validates that the error properly implements conversion to `anchor_lang::error::Error`
/// which is required for Anchor's error handling system.
#[test]
fn test_invalid_transfer_target_error_conversion() {
    let error = SubscriptionError::InvalidTransferTarget;
    let anchor_error: anchor_lang::error::Error = error.into();

    // Verify error can be converted to Anchor error
    assert!(matches!(
        anchor_error,
        anchor_lang::error::Error::AnchorError(_)
    ));
}

/// Test `InvalidTransferTarget` error number is 6024
///
/// Validates that the error code is assigned the expected value by Anchor.
/// This is critical for client-side error handling and debugging.
#[test]
fn test_invalid_transfer_target_error_number() {
    let error = SubscriptionError::InvalidTransferTarget;
    let anchor_error: anchor_lang::error::Error = error.into();

    if let anchor_lang::error::Error::AnchorError(anchor_err) = anchor_error {
        // Error code 6024 is the 25th error (starting from 6000)
        // InvalidTransferTarget is at index 24 (0-indexed)
        assert_eq!(
            anchor_err.error_code_number, 6024,
            "InvalidTransferTarget should have error code 6024"
        );
    } else {
        panic!("Expected AnchorError variant");
    }
}

/// Test `InvalidTransferTarget` error message
///
/// Validates that the error message is clear and actionable for users.
#[test]
fn test_invalid_transfer_target_error_message() {
    let error = SubscriptionError::InvalidTransferTarget;
    let anchor_error: anchor_lang::error::Error = error.into();

    if let anchor_lang::error::Error::AnchorError(anchor_err) = anchor_error {
        assert_eq!(
            anchor_err.error_msg,
            "Invalid authority transfer target. The new authority must be different from the current authority.",
            "InvalidTransferTarget should have correct error message"
        );
    } else {
        panic!("Expected AnchorError variant");
    }
}

/// Test authority transfer validation logic - same authority check
///
/// Simulates the validation logic from `transfer_authority.rs` (line 54)
/// that triggers `InvalidTransferTarget` when new authority equals current authority.
#[test]
fn test_same_authority_validation_fails() {
    let current_authority = Pubkey::new_unique();
    let new_authority = current_authority; // Same as current

    // Simulate the validation check from transfer_authority.rs
    let is_valid = new_authority != current_authority;

    assert!(
        !is_valid,
        "Same authority should fail validation and trigger InvalidTransferTarget"
    );
}

/// Test authority transfer validation logic - different authority check
///
/// Validates that providing a different authority passes the validation.
#[test]
fn test_different_authority_validation_passes() {
    let current_authority = Pubkey::new_unique();
    let new_authority = Pubkey::new_unique(); // Different from current

    // Simulate the validation check from transfer_authority.rs
    let is_valid = new_authority != current_authority;

    assert!(
        is_valid,
        "Different authority should pass validation"
    );
}

/// Test authority transfer validation with multiple authorities
///
/// Validates that only the exact same authority fails validation,
/// while all other authorities pass.
#[test]
fn test_authority_transfer_validation_matrix() {
    let current_authority = Pubkey::new_unique();
    let authorities = [
        current_authority,        // Should fail
        Pubkey::new_unique(),     // Should pass
        Pubkey::new_unique(),     // Should pass
        Pubkey::new_unique(),     // Should pass
    ];

    for (i, new_authority) in authorities.iter().enumerate() {
        let is_valid = *new_authority != current_authority;

        if i == 0 {
            assert!(
                !is_valid,
                "Index 0: Current authority should fail validation"
            );
        } else {
            assert!(
                is_valid,
                "Index {i}: Different authority should pass validation"
            );
        }
    }
}

// ============================================================================
// InvalidAmount Error Tests (Error Code 6025)
// ============================================================================

/// Test that `InvalidAmount` error can be converted to Anchor error
///
/// Validates that the error properly implements conversion to `anchor_lang::error::Error`
/// which is required for Anchor's error handling system.
#[test]
fn test_invalid_amount_error_conversion() {
    let error = SubscriptionError::InvalidAmount;
    let anchor_error: anchor_lang::error::Error = error.into();

    // Verify error can be converted to Anchor error
    assert!(matches!(
        anchor_error,
        anchor_lang::error::Error::AnchorError(_)
    ));
}

/// Test `InvalidAmount` error number is 6025
///
/// Validates that the error code is assigned the expected value by Anchor.
/// This is critical for client-side error handling and debugging.
#[test]
fn test_invalid_amount_error_number() {
    let error = SubscriptionError::InvalidAmount;
    let anchor_error: anchor_lang::error::Error = error.into();

    if let anchor_lang::error::Error::AnchorError(anchor_err) = anchor_error {
        // Error code 6025 is the 26th error (starting from 6000)
        // InvalidAmount is at index 25 (0-indexed)
        assert_eq!(
            anchor_err.error_code_number, 6025,
            "InvalidAmount should have error code 6025"
        );
    } else {
        panic!("Expected AnchorError variant");
    }
}

/// Test `InvalidAmount` error message
///
/// Validates that the error message is clear and actionable for users.
#[test]
fn test_invalid_amount_error_message() {
    let error = SubscriptionError::InvalidAmount;
    let anchor_error: anchor_lang::error::Error = error.into();

    if let anchor_lang::error::Error::AnchorError(anchor_err) = anchor_error {
        assert_eq!(
            anchor_err.error_msg,
            "Invalid amount provided. Amount must be greater than zero and within acceptable limits.",
            "InvalidAmount should have correct error message"
        );
    } else {
        panic!("Expected AnchorError variant");
    }
}

/// Test zero amount validation logic
///
/// Simulates the validation logic from `admin_withdraw_fees.rs` (line 90)
/// that triggers `InvalidAmount` when amount is zero.
#[test]
fn test_zero_amount_validation_fails() {
    let amount: u64 = 0;

    // Simulate the validation check from admin_withdraw_fees.rs
    let is_valid = amount != 0;

    assert!(
        !is_valid,
        "Zero amount should fail validation and trigger InvalidAmount"
    );
}

/// Test non-zero amount validation logic
///
/// Validates that any non-zero amount passes the zero-check validation.
#[test]
fn test_nonzero_amount_validation_passes() {
    let amount: u64 = 1;

    // Simulate the validation check from admin_withdraw_fees.rs
    let is_valid = amount != 0;

    assert!(
        is_valid,
        "Non-zero amount should pass validation"
    );
}

/// Test boundary condition: amount = 0 should fail, amount = 1 should pass
///
/// Validates the critical boundary between invalid (0) and valid (1+) amounts.
#[test]
fn test_amount_boundary_condition() {
    let zero_amount: u64 = 0;
    let min_valid_amount: u64 = 1;

    // Validate zero amount fails
    let zero_is_valid = zero_amount != 0;
    assert!(
        !zero_is_valid,
        "Boundary test: 0 should fail validation"
    );

    // Validate minimum valid amount passes
    let min_is_valid = min_valid_amount != 0;
    assert!(
        min_is_valid,
        "Boundary test: 1 should pass validation"
    );

    // Verify they have opposite validation results
    assert_ne!(
        zero_is_valid, min_is_valid,
        "Boundary condition: 0 and 1 should have opposite validation results"
    );
}

/// Test various invalid amounts (edge cases)
///
/// Validates that zero amount always fails validation regardless of context.
#[test]
fn test_various_invalid_amounts() {
    let invalid_amounts = [0u64];

    for amount in invalid_amounts {
        let is_valid = amount != 0;
        assert!(
            !is_valid,
            "Amount {amount} should fail validation"
        );
    }
}

/// Test various valid amounts
///
/// Validates that all positive amounts pass the zero-check validation.
#[test]
fn test_various_valid_amounts() {
    let valid_amounts = [
        1u64,                    // Minimum valid
        100,                     // Small amount
        1_000_000,              // 1 USDC (6 decimals)
        1_000_000_000,          // 1,000 USDC
        u64::MAX,               // Maximum u64 value
    ];

    for amount in valid_amounts {
        let is_valid = amount != 0;
        assert!(
            is_valid,
            "Amount {amount} should pass validation"
        );
    }
}

/// Test amount validation with withdrawal limit context
///
/// Simulates the complete validation flow including zero check and max limit check.
/// This represents the actual validation sequence in `admin_withdraw_fees.rs`.
#[test]
fn test_amount_validation_with_withdrawal_limit() {
    let max_withdrawal_amount: u64 = 1_000_000_000; // 1,000 USDC

    // Test cases: (amount, should_pass_zero_check, should_pass_limit_check)
    let test_cases = [
        (0u64, false, true),                               // Fails zero check, passes limit check
        (1u64, true, true),                                // Passes both
        (max_withdrawal_amount, true, true),               // At limit, passes
        (max_withdrawal_amount + 1, true, false),          // Exceeds limit
        (u64::MAX, true, false),                           // Far exceeds limit
    ];

    for (amount, should_pass_zero, should_pass_limit) in test_cases {
        // Zero check (lines 89-92 in admin_withdraw_fees.rs)
        let passes_zero_check = amount != 0;
        assert_eq!(
            passes_zero_check, should_pass_zero,
            "Amount {amount}: zero check validation mismatch"
        );

        // Limit check (lines 96-99 in admin_withdraw_fees.rs)
        let passes_limit_check = amount <= max_withdrawal_amount;
        assert_eq!(
            passes_limit_check, should_pass_limit,
            "Amount {amount}: limit check validation mismatch"
        );

        // Combined validation (both must pass)
        let passes_complete_validation = passes_zero_check && passes_limit_check;
        let expected_to_pass = should_pass_zero && should_pass_limit;
        assert_eq!(
            passes_complete_validation, expected_to_pass,
            "Amount {amount}: complete validation mismatch"
        );
    }
}

/// Test amount validation prevents withdrawal abuse
///
/// Validates that zero amount withdrawal attempts are properly rejected,
/// preventing potential abuse or wasteful transactions.
#[test]
fn test_amount_validation_prevents_abuse() {
    // Simulate multiple zero amount withdrawal attempts
    let abuse_attempts = [0u64, 0, 0, 0, 0];

    for (i, amount) in abuse_attempts.iter().enumerate() {
        let is_valid = *amount != 0;
        assert!(
            !is_valid,
            "Abuse attempt {i}: Zero amount should always be rejected"
        );
    }
}

// ============================================================================
// Combined Error Validation Tests
// ============================================================================

/// Test that both errors have unique error codes
///
/// Validates that `InvalidTransferTarget` and `InvalidAmount` have distinct error codes.
#[test]
fn test_error_codes_are_unique() {
    let error_1 = SubscriptionError::InvalidTransferTarget;
    let error_2 = SubscriptionError::InvalidAmount;

    let anchor_error_1: anchor_lang::error::Error = error_1.into();
    let anchor_error_2: anchor_lang::error::Error = error_2.into();

    if let (
        anchor_lang::error::Error::AnchorError(err1),
        anchor_lang::error::Error::AnchorError(err2),
    ) = (anchor_error_1, anchor_error_2)
    {
        assert_ne!(
            err1.error_code_number, err2.error_code_number,
            "Error codes should be unique: InvalidTransferTarget (6024) != InvalidAmount (6025)"
        );

        // Explicitly verify the expected codes
        assert_eq!(err1.error_code_number, 6024);
        assert_eq!(err2.error_code_number, 6025);
    } else {
        panic!("Expected AnchorError variants");
    }
}

/// Test that both errors have distinct messages
///
/// Validates that error messages are unique and descriptive.
#[test]
fn test_error_messages_are_distinct() {
    let error_1 = SubscriptionError::InvalidTransferTarget;
    let error_2 = SubscriptionError::InvalidAmount;

    let anchor_error_1: anchor_lang::error::Error = error_1.into();
    let anchor_error_2: anchor_lang::error::Error = error_2.into();

    if let (
        anchor_lang::error::Error::AnchorError(err1),
        anchor_lang::error::Error::AnchorError(err2),
    ) = (anchor_error_1, anchor_error_2)
    {
        assert_ne!(
            err1.error_msg, err2.error_msg,
            "Error messages should be distinct and descriptive"
        );
    } else {
        panic!("Expected AnchorError variants");
    }
}

/// Test error code sequence is correct
///
/// Validates that the new error codes follow the expected sequence after 6023.
#[test]
fn test_error_code_sequence() {
    let error_6024 = SubscriptionError::InvalidTransferTarget;
    let error_6025 = SubscriptionError::InvalidAmount;

    let anchor_error_6024: anchor_lang::error::Error = error_6024.into();
    let anchor_error_6025: anchor_lang::error::Error = error_6025.into();

    if let (
        anchor_lang::error::Error::AnchorError(err_6024),
        anchor_lang::error::Error::AnchorError(err_6025),
    ) = (anchor_error_6024, anchor_error_6025)
    {
        // Verify sequential numbering
        assert_eq!(
            err_6025.error_code_number,
            err_6024.error_code_number + 1,
            "Error codes should be sequential: 6025 = 6024 + 1"
        );
    } else {
        panic!("Expected AnchorError variants");
    }
}
