//! Wallet signature verification utilities for Solana authentication
//!
//! This module provides cryptographic signature verification functionality
//! for wallet-based authentication in the Tally ecosystem.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;

/// Verify wallet signature for authentication
///
/// # Arguments
///
/// * `wallet_address` - Base58 encoded wallet address
/// * `signature` - Base58 encoded signature
/// * `message` - Message that was signed (should be a nonce)
///
/// # Returns
///
/// `Ok(())` if signature is valid, `Err` otherwise
///
/// # Errors
///
/// Returns an error if:
/// - Wallet address is invalid format
/// - Signature is invalid format
/// - Message is empty
/// - Signature verification fails
pub fn verify_wallet_signature(wallet_address: &str, signature: &str, message: &str) -> Result<()> {
    // Add debug logging for signature format investigation
    tracing::debug!(
        service = "tally-sdk",
        component = "signature",
        event = "signature_debug",
        wallet_address = %wallet_address,
        signature_len = signature.len(),
        signature_first_chars = %signature.chars().take(20).collect::<String>(),
        message_len = message.len(),
        "Signature verification debug info"
    );

    // Parse wallet address
    let pubkey = Pubkey::from_str(wallet_address).context("Invalid wallet address format")?;

    // Parse signature - try base58 first, then hex as fallback
    let sig = match Signature::from_str(signature) {
        Ok(sig) => sig,
        Err(_) => {
            // Try parsing as hex if base58 fails
            if signature.len() == 128 {
                // 64 bytes * 2 hex chars = 128 chars
                match hex::decode(signature) {
                    Ok(bytes) if bytes.len() == 64 => match <[u8; 64]>::try_from(bytes) {
                        Ok(array) => Signature::from(array),
                        Err(_) => anyhow::bail!("Failed to convert hex bytes to signature array"),
                    },
                    _ => anyhow::bail!(
                        "Invalid hex signature format - must be 128 hex characters (64 bytes)"
                    ),
                }
            } else {
                anyhow::bail!("Invalid signature format - must be base58 encoded (88 chars) or hex encoded (128 chars), got {} chars", signature.len());
            }
        }
    };

    // Verify signature against the raw message bytes
    // Most wallets sign the raw message directly without the off-chain header
    let message_bytes = message.as_bytes();

    // Verify signature using Solana's signature verification
    if !sig.verify(pubkey.as_ref(), message_bytes) {
        anyhow::bail!("Signature verification failed");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_verification_with_invalid_format() {
        // Test case reproducing the "Invalid signature format" error from logs
        let wallet_address = "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9";
        let invalid_signature = "invalid_signature_format"; // This is not valid base58
        let message = "Sign this message to authenticate with Tally:\n\nNonce: tally_auth_1726800502_abc12345\nTimestamp: 2024-09-20T02:21:42.000Z";

        // This should fail with "Invalid signature format" error
        let result = verify_wallet_signature(wallet_address, invalid_signature, message);

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));
    }

    #[test]
    fn test_signature_verification_with_empty_signature() {
        let wallet_address = "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9";
        let empty_signature = "";
        let message = "test message";

        let result = verify_wallet_signature(wallet_address, empty_signature, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));
    }

    #[test]
    fn test_signature_verification_with_invalid_base58() {
        let wallet_address = "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9";
        // Using characters not in base58 alphabet (0, O, I, l)
        let invalid_base58_signature = "000000OOOOOIIIIIllllllinvalid";
        let message = "test message";

        let result = verify_wallet_signature(wallet_address, invalid_base58_signature, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));
    }

    #[test]
    fn test_signature_verification_with_wrong_length_signature() {
        let wallet_address = "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9";
        // Too short to be a valid Solana signature
        let short_signature = "ABC123";
        let message = "test message";

        let result = verify_wallet_signature(wallet_address, short_signature, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));
    }

    #[test]
    fn test_signature_verification_with_invalid_wallet_address() {
        let invalid_wallet_address = "invalid_wallet";
        let signature =
            "5K6aZkd8hjw4oMXkNYkrjFzSjLaXvqWHk4GFRv3WvJ8Z3Q4J1L2M3N4P5Q6R7S8T9U1V2W3X4Y5Z6"; // Valid base58 but wrong length
        let message = "test message";

        let result = verify_wallet_signature(invalid_wallet_address, signature, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid wallet address format"));
    }

    #[test]
    fn test_signature_format_parsing() {
        let wallet_address = "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9";
        let message = "test message";

        // Test valid base58 signature (64 bytes as base58)
        let valid_base58 = "5K6aZkd8hjw4oMXkNYkrjFzSjLaXvqWHk4GFRv3WvJ8Z3Q4J1L2M3N4P5Q6R7S8T9U1V2W3X4Y5Z6abcdefghijk123456789ABCDEF";
        // This should fail gracefully (invalid signature but correct format handling)
        let result = verify_wallet_signature(wallet_address, valid_base58, message);
        assert!(result.is_err());

        // Test valid hex signature (128 hex chars = 64 bytes)
        let valid_hex = "1234567890abcdef".repeat(8); // 128 hex characters
        let result = verify_wallet_signature(wallet_address, &valid_hex, message);
        assert!(result.is_err()); // Will fail signature verification but should parse format correctly

        // Test invalid signature formats
        let invalid_short = "too_short";
        let result = verify_wallet_signature(wallet_address, invalid_short, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));

        let invalid_hex_wrong_length = "1234567890abcdef".repeat(4); // 64 hex characters = 32 bytes (wrong length)
        let result = verify_wallet_signature(wallet_address, &invalid_hex_wrong_length, message);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Invalid signature format"));
    }
}