//! Transaction utility functions for Solana Actions and Web3 applications
//!
//! This module provides core transaction building and serialization utilities
//! that are commonly needed across different applications in the Tally ecosystem.

#![forbid(unsafe_code)]

use crate::error::{Result, TallyError};
use anchor_client::solana_sdk::{
    hash::Hash,
    instruction::Instruction,
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use base64::{engine::general_purpose::STANDARD, Engine};

/// Parameters for building a start agreement transaction
#[derive(Debug, Clone)]
pub struct StartAgreementTransactionParams<'a> {
    pub payer: &'a Pubkey,
    pub payment_terms_pda: &'a Pubkey,
    pub payment_amount: u64,
    pub allowance_periods: u8,
    pub recent_blockhash: Hash,
    pub payee: &'a crate::program_types::Payee,
    pub payment_terms: &'a crate::program_types::PaymentTerms,
    pub platform_treasury_ata: &'a Pubkey,
}

/// Convert from anchor pubkey to `solana_sdk` pubkey
///
/// This function is now a no-op since both types are the same after the refactor,
/// but kept for backward compatibility.
///
/// # Arguments
/// * `pk` - The pubkey to convert
///
/// # Returns
/// The same pubkey (no conversion needed)
#[must_use]
pub const fn convert_anchor_pubkey(pk: &Pubkey) -> Pubkey {
    *pk // No conversion needed since types are now the same
}

/// Creates a Memo instruction for transaction traceability
///
/// # Arguments
/// * `memo` - The memo string to include
///
/// # Returns
/// Memo instruction for adding to transactions
#[must_use]
pub fn create_memo_instruction(memo: &str) -> Instruction {
    Instruction {
        program_id: spl_memo::ID,
        accounts: vec![],
        data: memo.as_bytes().to_vec(),
    }
}

/// Builds a complete transaction with recent blockhash and serializes it to base64
///
/// This is the core transaction building utility used across the Tally ecosystem
/// for preparing transactions that can be signed by wallets.
///
/// # Arguments
/// * `instructions` - Vector of instructions to include
/// * `payer` - Transaction fee payer
/// * `recent_blockhash` - Recent blockhash for transaction
///
/// # Returns
/// Serialized transaction as base64 string
///
/// # Errors
/// Returns error if transaction building or serialization fails
pub fn build_transaction(
    instructions: &[Instruction],
    payer: &Pubkey,
    recent_blockhash: Hash,
) -> Result<String> {
    // Create message
    let message = Message::new_with_blockhash(instructions, Some(payer), &recent_blockhash);
    let num_signatures = message.header.num_required_signatures;
    let versioned_message = VersionedMessage::Legacy(message);

    // Create unsigned transaction
    let transaction = VersionedTransaction {
        signatures: vec![Signature::default(); num_signatures as usize],
        message: versioned_message,
    };

    // Serialize transaction
    let serialized = bincode::serialize(&transaction)
        .map_err(|e| TallyError::Generic(format!("Transaction serialization failed: {e}")))?;

    // Encode as base64
    Ok(STANDARD.encode(serialized))
}

/// Gets or creates the associated token address for a user's USDC account
/// using tally-sdk ATA utilities
///
/// # Arguments
/// * `user` - User's public key
/// * `usdc_mint` - USDC mint address
///
/// # Returns
/// Associated token account address
///
/// # Errors
/// Returns error if ATA computation fails
pub fn get_user_usdc_ata(user: &Pubkey, usdc_mint: &Pubkey) -> Result<Pubkey> {
    crate::ata::get_associated_token_address_for_mint(user, usdc_mint)
}

/// Map `TallyError` to a generic error message string
///
/// This provides a way to convert SDK errors into string messages suitable
/// for external applications that don't want to depend on `TallyError` directly.
///
/// # Arguments
/// * `err` - The `TallyError` to convert
///
/// # Returns
/// String representation of the error
#[must_use]
pub fn map_tally_error_to_string(err: &TallyError) -> String {
    match err {
        TallyError::Generic(msg) => msg.clone(),
        TallyError::InvalidPda(msg) => {
            format!("Invalid account: {msg}")
        }
        TallyError::SplToken(err) => format!("SPL Token error: {err}"),
        TallyError::Program(err) => format!("Program error: {err}"),
        TallyError::Solana(err) => format!("Solana error: {err}"),
        TallyError::InvalidTokenProgram { expected, found } => {
            format!("Invalid token program: expected {expected}, found {found}")
        }
        TallyError::AccountNotFound(account) => format!("Account not found: {account}"),
        TallyError::InsufficientFunds {
            required,
            available,
        } => {
            format!("Insufficient funds: required {required}, available {available}")
        }
        TallyError::TokenProgramDetectionFailed { mint } => {
            format!("Failed to detect token program for mint: {mint}")
        }
        _ => err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_anchor_pubkey() {
        let pk = Pubkey::new_unique();
        let converted = convert_anchor_pubkey(&pk);
        assert_eq!(pk, converted);
    }

    #[test]
    fn test_create_memo_instruction() {
        let memo = "Test memo";
        let instruction = create_memo_instruction(memo);

        assert_eq!(instruction.program_id, spl_memo::ID);
        assert_eq!(instruction.data, memo.as_bytes());
        assert!(instruction.accounts.is_empty());
    }

    #[test]
    fn test_build_transaction() {
        let payer = Pubkey::new_unique();
        let recent_blockhash = Hash::default();

        // Create a simple instruction for testing
        let instruction = Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![],
            data: vec![],
        };
        let instructions = vec![instruction];

        let result = build_transaction(&instructions, &payer, recent_blockhash);
        assert!(result.is_ok());

        let transaction_b64 = result.unwrap();
        assert!(!transaction_b64.is_empty());

        // Verify it's valid base64
        let decoded = STANDARD.decode(&transaction_b64);
        assert!(decoded.is_ok());

        // Verify we can deserialize it back to a transaction
        let transaction_bytes = decoded.unwrap();
        let transaction: std::result::Result<VersionedTransaction, _> =
            bincode::deserialize(&transaction_bytes);
        assert!(transaction.is_ok());
    }

    #[test]
    fn test_get_user_usdc_ata() {
        let user = Pubkey::new_unique();
        let usdc_mint = Pubkey::new_unique();

        let result1 = get_user_usdc_ata(&user, &usdc_mint);
        let result2 = get_user_usdc_ata(&user, &usdc_mint);

        // Both should succeed
        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Should be deterministic
        assert_eq!(result1.unwrap(), result2.unwrap());
    }

    #[test]
    fn test_map_tally_error_to_string() {
        let generic_error = TallyError::Generic("Test error".to_string());
        let mapped = map_tally_error_to_string(&generic_error);
        assert_eq!(mapped, "Test error");

        let account_error = TallyError::AccountNotFound("test_account".to_string());
        let mapped = map_tally_error_to_string(&account_error);
        assert_eq!(mapped, "Account not found: test_account");
    }
}
