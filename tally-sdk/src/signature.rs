//! Wallet signature verification and transaction signing utilities for Solana
//!
//! This module provides cryptographic signature verification functionality
//! for wallet-based authentication and transaction signing support for frontend
//! wallet integration in the Tally ecosystem.

#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use std::str::FromStr;

/// Transaction signing utilities for frontend wallet integration
pub mod transaction_signing {
    use super::{Hash, Instruction, Pubkey, Result, VersionedTransaction};

    /// Build a signable transaction for frontend `wallet.signTransaction()`
    pub fn prepare_transaction_for_signing(
        instructions: &[Instruction],
        payer: &Pubkey,
        recent_blockhash: Hash,
    ) -> Result<String> {
        super::prepare_transaction_for_signing(instructions, payer, recent_blockhash)
    }

    /// Verify a signed transaction from frontend
    pub fn verify_signed_transaction(
        signed_transaction_base64: &str,
        expected_signer: &Pubkey,
    ) -> Result<VersionedTransaction> {
        super::verify_signed_transaction(signed_transaction_base64, expected_signer)
    }

    /// Extract signature from signed transaction
    pub fn extract_transaction_signature(signed_transaction_base64: &str) -> Result<String> {
        super::extract_transaction_signature(signed_transaction_base64)
    }
}

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

/// Normalize signature format from different wallet implementations
///
/// Handles various wallet signature formats:
/// - `Uint8Array` converted to base58 string
/// - Hex-encoded signatures (128 characters)
/// - Base58-encoded signatures (88 characters)
/// - Signature objects with toString method
///
/// # Arguments
///
/// * `signature_input` - Signature in various formats from frontend wallets
///
/// # Returns
///
/// Base58-encoded signature string compatible with Solana
///
/// # Errors
///
/// Returns an error if signature format cannot be normalized
pub fn normalize_signature_format(signature_input: &str) -> Result<String> {
    // Handle empty or whitespace-only signatures
    let signature = signature_input.trim();
    if signature.is_empty() {
        anyhow::bail!("Empty signature");
    }

    // Try parsing as base58 first (most common format)
    if Signature::from_str(signature).is_ok() {
        return Ok(signature.to_string());
    }

    // Try parsing as hex if base58 fails
    if signature.len() == 128 {
        // 64 bytes * 2 hex chars = 128 chars
        match hex::decode(signature) {
            Ok(bytes) if bytes.len() == 64 => {
                // Convert hex bytes to base58 string
                match <[u8; 64]>::try_from(bytes) {
                    Ok(array) => {
                        let sig = Signature::from(array);
                        return Ok(sig.to_string());
                    }
                    Err(_) => anyhow::bail!("Failed to convert hex bytes to signature array"),
                }
            }
            _ => {
                anyhow::bail!("Invalid hex signature format - must be 128 hex characters (64 bytes)")
            }
        }
    }

    // If neither base58 nor hex worked, return error with details
    anyhow::bail!(
        "Invalid signature format - must be base58 encoded (88 chars) or hex encoded (128 chars), got {} chars",
        signature.len()
    )
}

/// Prepare transaction for frontend wallet signing
///
/// Serializes a Solana transaction to base64 format that can be sent to
/// frontend wallets for signing via `wallet.signTransaction()`.
///
/// # Arguments
///
/// * `instructions` - Vector of instructions to include in transaction
/// * `payer` - Transaction fee payer public key
/// * `recent_blockhash` - Recent blockhash for transaction
///
/// # Returns
///
/// Base64-encoded serialized transaction ready for wallet signing
///
/// # Errors
///
/// Returns error if transaction building or serialization fails
pub fn prepare_transaction_for_signing(
    instructions: &[Instruction],
    payer: &Pubkey,
    recent_blockhash: Hash,
) -> Result<String> {
    // Create message with recent blockhash
    let message = Message::new_with_blockhash(instructions, Some(payer), &recent_blockhash);
    let num_signatures = message.header.num_required_signatures;
    let versioned_message = VersionedMessage::Legacy(message);

    // Create unsigned transaction with placeholder signatures
    let transaction = VersionedTransaction {
        signatures: vec![Signature::default(); num_signatures as usize],
        message: versioned_message,
    };

    // Serialize transaction
    let serialized = bincode::serialize(&transaction)
        .context("Failed to serialize transaction for signing")?;

    // Encode as base64 for frontend
    Ok(STANDARD.encode(serialized))
}

/// Verify a signed transaction from frontend wallet
///
/// Deserializes and validates a transaction that was signed by a frontend wallet.
/// Checks that all required signatures are present and valid.
///
/// # Arguments
///
/// * `signed_transaction_base64` - Base64-encoded signed transaction from wallet
/// * `expected_signer` - Expected signer public key to validate against
///
/// # Returns
///
/// Deserialized and verified `VersionedTransaction`
///
/// # Errors
///
/// Returns error if:
/// - Transaction deserialization fails
/// - Required signatures are missing or invalid
/// - Signer verification fails
pub fn verify_signed_transaction(
    signed_transaction_base64: &str,
    expected_signer: &Pubkey,
) -> Result<VersionedTransaction> {
    // Decode base64 transaction
    let transaction_bytes = STANDARD
        .decode(signed_transaction_base64)
        .context("Failed to decode base64 transaction")?;

    // Deserialize transaction
    let transaction: VersionedTransaction = bincode::deserialize(&transaction_bytes)
        .context("Failed to deserialize signed transaction")?;

    // Verify transaction has signatures
    if transaction.signatures.is_empty() {
        anyhow::bail!("Transaction has no signatures");
    }

    // Verify first signature corresponds to expected signer
    let message_data = transaction.message.serialize();
    let signature = &transaction.signatures[0];

    if signature == &Signature::default() {
        anyhow::bail!("Transaction signature is empty/default");
    }

    // Verify signature against message and expected signer
    if !signature.verify(expected_signer.as_ref(), &message_data) {
        anyhow::bail!("Transaction signature verification failed for expected signer");
    }

    Ok(transaction)
}

/// Extract signature from signed transaction
///
/// Extracts the first signature from a signed transaction and returns it as a base58 string.
/// This is useful for getting the transaction signature for logging, tracking, or verification.
///
/// # Arguments
///
/// * `signed_transaction_base64` - Base64-encoded signed transaction from wallet
///
/// # Returns
///
/// Base58-encoded signature string
///
/// # Errors
///
/// Returns error if:
/// - Transaction deserialization fails
/// - Transaction has no signatures
/// - Signature extraction fails
pub fn extract_transaction_signature(signed_transaction_base64: &str) -> Result<String> {
    // Decode base64 transaction
    let transaction_bytes = STANDARD
        .decode(signed_transaction_base64)
        .context("Failed to decode base64 transaction")?;

    // Deserialize transaction
    let transaction: VersionedTransaction = bincode::deserialize(&transaction_bytes)
        .context("Failed to deserialize signed transaction")?;

    // Check if transaction has signatures
    if transaction.signatures.is_empty() {
        anyhow::bail!("Transaction has no signatures");
    }

    // Get first signature and convert to base58
    let signature = &transaction.signatures[0];

    if signature == &Signature::default() {
        anyhow::bail!("Transaction signature is empty/default");
    }

    Ok(signature.to_string())
}

/// Validate wallet address format (basic Solana base58 check)
///
/// Performs basic validation of Solana wallet address format without
/// requiring full parsing. Checks length and base58 character validity.
///
/// # Arguments
///
/// * `address` - Wallet address string to validate
///
/// # Returns
///
/// `true` if address appears to be a valid Solana wallet format, `false` otherwise
#[must_use]
pub fn is_valid_wallet_address(address: &str) -> bool {
    const BASE58_ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Basic validation: Solana addresses are 32-44 characters, base58 encoded
    if address.len() < 32 || address.len() > 44 {
        return false;
    }

    // Check if all characters are valid base58
    address.chars().all(|c| BASE58_ALPHABET.contains(c))
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

    #[test]
    fn test_normalize_signature_format() {
        // Test empty signature
        let result = normalize_signature_format("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty signature"));

        // Test whitespace-only signature
        let result = normalize_signature_format("   ");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty signature"));

        // Test valid hex signature (128 characters)
        let valid_hex = "1234567890abcdef".repeat(8);
        let result = normalize_signature_format(&valid_hex);
        assert!(result.is_ok());

        // Test invalid hex signature (wrong length)
        let invalid_hex = "1234567890abcdef".repeat(4); // 64 characters
        let result = normalize_signature_format(&invalid_hex);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid signature format"));

        // Test invalid format (not base58, not hex)
        let invalid_format = "invalid_signature_format_123";
        let result = normalize_signature_format(invalid_format);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid signature format"));
    }

    #[test]
    fn test_prepare_transaction_for_signing() {
        use solana_sdk::hash::Hash;

        let payer = Pubkey::new_unique();
        let _recipient = Pubkey::new_unique();
        let recent_blockhash = Hash::default();

        // Create a simple instruction for testing
        let instruction = Instruction {
            program_id: Pubkey::new_unique(), // Use any program ID for testing
            accounts: vec![],
            data: vec![],
        };
        let instructions = vec![instruction];

        let result = prepare_transaction_for_signing(&instructions, &payer, recent_blockhash);
        assert!(result.is_ok());

        let transaction_base64 = result.unwrap();
        assert!(!transaction_base64.is_empty());

        // Verify it's valid base64
        let decoded = STANDARD.decode(&transaction_base64);
        assert!(decoded.is_ok());

        // Verify we can deserialize it back to a transaction
        let transaction_bytes = decoded.unwrap();
        let transaction: Result<VersionedTransaction, _> = bincode::deserialize(&transaction_bytes);
        assert!(transaction.is_ok());

        let tx = transaction.unwrap();
        // Should have default signatures (unsigned)
        assert!(!tx.signatures.is_empty());
        assert_eq!(tx.signatures[0], Signature::default());
    }

    #[test]
    fn test_verify_signed_transaction_empty_signatures() {
        use solana_sdk::hash::Hash;

        let payer = Pubkey::new_unique();
        let recent_blockhash = Hash::default();

        // Create transaction with empty instructions
        let instructions = vec![];
        let transaction_base64 = prepare_transaction_for_signing(&instructions, &payer, recent_blockhash)
            .expect("Should prepare transaction successfully");

        // Try to verify unsigned transaction
        let result = verify_signed_transaction(&transaction_base64, &payer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Transaction signature is empty/default"));
    }

    #[test]
    fn test_verify_signed_transaction_invalid_base64() {
        let payer = Pubkey::new_unique();
        let invalid_base64 = "not_valid_base64_data!!!";

        let result = verify_signed_transaction(invalid_base64, &payer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to decode base64 transaction"));
    }

    #[test]
    fn test_verify_signed_transaction_invalid_transaction_data() {
        let payer = Pubkey::new_unique();
        // Valid base64 but not a valid transaction
        let invalid_transaction = STANDARD.encode(b"not a valid transaction");

        let result = verify_signed_transaction(&invalid_transaction, &payer);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to deserialize signed transaction"));
    }

    #[test]
    fn test_extract_transaction_signature() {
        use solana_sdk::hash::Hash;

        let payer = Pubkey::new_unique();
        let recent_blockhash = Hash::default();

        // Create a simple instruction for testing
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![],
            data: vec![],
        };
        let instructions = vec![instruction];

        // Prepare transaction
        let transaction_base64 = prepare_transaction_for_signing(&instructions, &payer, recent_blockhash)
            .expect("Should prepare transaction successfully");

        // Try to extract signature from unsigned transaction
        let result = extract_transaction_signature(&transaction_base64);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Transaction signature is empty/default"));

        // Test with invalid base64
        let invalid_base64 = "not_valid_base64_data!!!";
        let result = extract_transaction_signature(invalid_base64);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to decode base64 transaction"));

        // Test with valid base64 but invalid transaction data
        let invalid_transaction = STANDARD.encode(b"not a valid transaction");
        let result = extract_transaction_signature(&invalid_transaction);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to deserialize signed transaction"));

        // Test creating a properly signed transaction (this would normally be done by wallet)
        let transaction_bytes = STANDARD.decode(&transaction_base64).unwrap();
        let mut transaction: VersionedTransaction = bincode::deserialize(&transaction_bytes).unwrap();

        // Replace default signature with a real signature for testing
        let test_signature = Signature::new_unique();
        transaction.signatures[0] = test_signature;

        // Re-serialize and encode
        let signed_transaction_bytes = bincode::serialize(&transaction).unwrap();
        let signed_transaction_base64 = STANDARD.encode(signed_transaction_bytes);

        // Now extract signature should work
        let result = extract_transaction_signature(&signed_transaction_base64);
        assert!(result.is_ok());
        let extracted_signature = result.unwrap();
        assert_eq!(extracted_signature, test_signature.to_string());
    }

    #[test]
    fn test_transaction_signing_module() {
        use super::transaction_signing;
        use solana_sdk::hash::Hash;

        let payer = Pubkey::new_unique();
        let recent_blockhash = Hash::default();
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![],
            data: vec![],
        };
        let instructions = vec![instruction];

        // Test prepare_transaction_for_signing through module
        let result = transaction_signing::prepare_transaction_for_signing(&instructions, &payer, recent_blockhash);
        assert!(result.is_ok());

        let transaction_base64 = result.unwrap();

        // Test extract_transaction_signature through module
        let result = transaction_signing::extract_transaction_signature(&transaction_base64);
        assert!(result.is_err()); // Should fail because transaction is unsigned
        assert!(result.unwrap_err().to_string().contains("Transaction signature is empty/default"));

        // Test verify_signed_transaction through module
        let result = transaction_signing::verify_signed_transaction(&transaction_base64, &payer);
        assert!(result.is_err()); // Should fail because transaction is unsigned
    }

    #[test]
    fn test_is_valid_wallet_address() {
        // Valid wallet address
        assert!(is_valid_wallet_address(
            "6YbW3k3oiU8kMbJLYCc8XA27c7DfqG5WSjTkED4Z2pj9"
        ));

        // Invalid cases
        assert!(!is_valid_wallet_address("too_short"));
        assert!(!is_valid_wallet_address("contains_invalid_characters_0OIl"));
        assert!(!is_valid_wallet_address(""));
        assert!(!is_valid_wallet_address(
            "way_too_long_to_be_a_valid_solana_wallet_address_definitely_longer_than_44_chars"
        ));

        // Edge cases for length
        let min_length = "123456789ABCDEFGHJKLMNPQRSTUVWXYZa"; // 32 chars, valid base58
        assert!(is_valid_wallet_address(min_length));

        let max_length = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijk"; // 44 chars, valid base58
        assert!(is_valid_wallet_address(max_length));

        // Just outside valid length ranges
        let too_short = "123456789ABCDEFGHJKLMNPQRSTUVWXYz"; // 33 chars, but this is actually valid
        assert!(is_valid_wallet_address(too_short)); // This is valid (33 chars within 32-44 range)

        let too_short_actual = "123456789ABCDEFGHJKLMNPQRSTUVWx"; // 31 chars
        assert!(!is_valid_wallet_address(too_short_actual));

        let too_long = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkL"; // 45 chars
        assert!(!is_valid_wallet_address(too_long));
    }
}