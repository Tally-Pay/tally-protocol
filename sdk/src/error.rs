//! Error types for the Tally SDK
//!
//! This module provides comprehensive error handling for the Tally SDK, including
//! automatic mapping of program-specific error codes to meaningful error variants.
//!
//! # Program Error Mapping
//!
//! The SDK automatically maps specific program error codes to detailed error variants:
//!
//! - **6012**: `InvalidPayerTokenAccount` - Invalid payer USDC token account
//! - **6013**: `InvalidPayeeTreasuryAccount` - Invalid payee treasury account
//! - **6014**: `InvalidPlatformTreasuryAccount` - Invalid platform treasury account
//! - **6015**: `InvalidUsdcMint` - Invalid USDC mint account
//! - **6016**: `PayeeNotFound` - Payee account not found or invalid
//! - **6017**: `PaymentTermsNotFound` - `PaymentAgreement` `payment_terms` not found or invalid
//! - **6018**: `PaymentAgreementNotFound` - `PaymentAgreement` not found or invalid
//! - **6019**: `ConfigNotFound` - Global configuration account not found
//!
//! # Example
//!
//! ```rust
//! use tally_sdk::{SimpleTallyClient, error::TallyError};
//! use anchor_lang::prelude::Pubkey;
//!
//! async fn handle_transaction_error() {
//!     let client = SimpleTallyClient::new("https://api.devnet.solana.com").unwrap();
//!     let some_address = Pubkey::default();
//!
//!     // When a transaction fails, you get specific error information:
//!     match client.get_payee(&some_address) {
//!         Ok(payee) => println!("Found payee: {:?}", payee),
//!         Err(TallyError::PayeeNotFound) => {
//!             println!("Payee account not found - ensure it's properly initialized");
//!         }
//!         Err(TallyError::InvalidPayerTokenAccount) => {
//!             println!("Invalid payer token account provided");
//!         }
//!         Err(other_error) => {
//!             println!("Other error: {}", other_error);
//!         }
//!     }
//! }
//! ```

use thiserror::Error;

/// Result type for Tally SDK operations
pub type Result<T> = std::result::Result<T, TallyError>;

/// Error types that can occur when using the Tally SDK
#[derive(Error, Debug)]
pub enum TallyError {
    /// Error from Anchor framework
    #[error("Anchor error: {0}")]
    Anchor(anchor_lang::error::Error),

    /// Error from Anchor client
    #[error("Anchor client error: {0}")]
    AnchorClient(Box<anchor_client::ClientError>),

    /// Error from Solana SDK
    #[error("Solana SDK error: {0}")]
    Solana(#[from] anchor_client::solana_sdk::pubkey::ParsePubkeyError),

    /// Error from SPL Token
    #[error("SPL Token error: {0}")]
    SplToken(#[from] spl_token::error::TokenError),

    /// Error from Solana Program
    #[error("Program error: {0}")]
    Program(#[from] solana_program::program_error::ProgramError),

    /// Error from serde JSON
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error with message
    #[error("Tally SDK error: {0}")]
    Generic(String),

    /// Event parsing error
    #[error("Event parsing error: {0}")]
    ParseError(String),

    /// Invalid PDA computation
    #[error("Invalid PDA: {0}")]
    InvalidPda(String),

    /// Invalid token program
    #[error("Invalid token program: expected {expected}, found {found}")]
    InvalidTokenProgram { expected: String, found: String },

    /// Account not found
    #[error("Account not found: {0}")]
    AccountNotFound(String),

    /// Insufficient funds
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds { required: u64, available: u64 },

    /// Invalid payment agreement state
    #[error("Invalid payment agreement state: {0}")]
    InvalidPaymentAgreementState(String),

    /// Token program detection failed
    #[error("Failed to detect token program for mint: {mint}")]
    TokenProgramDetectionFailed { mint: String },

    /// RPC error for blockchain queries
    #[error("RPC error: {0}")]
    RpcError(String),

    // Specific program error variants (maps to Anchor error codes 6012-6019)
    /// Invalid payer token account (program error 6012)
    #[error("Invalid payer token account. Ensure the account is a valid USDC token account owned by the payer.")]
    InvalidPayerTokenAccount,

    /// Invalid payee treasury token account (program error 6013)
    #[error("Invalid payee treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidPayeeTreasuryAccount,

    /// Invalid platform treasury token account (program error 6014)
    #[error("Invalid platform treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidPlatformTreasuryAccount,

    /// Invalid USDC mint account (program error 6015)
    #[error("Invalid USDC mint account. Ensure the account is a valid token mint account.")]
    InvalidUsdcMint,

    /// Payee account not found or invalid (program error 6016)
    #[error(
        "Payee account not found or invalid. Ensure the payee has been properly initialized."
    )]
    PayeeNotFound,

    /// `PaymentTerms` not found or invalid (program error 6017)
    #[error("PaymentTerms not found or invalid. Ensure the payment terms exist and belong to the specified payee.")]
    PaymentTermsNotFound,

    /// `PaymentAgreement` not found or invalid (program error 6018)
    #[error("PaymentAgreement not found or invalid. Ensure the payment agreement exists for these payment terms and payer.")]
    PaymentAgreementNotFound,

    /// Global configuration account not found or invalid (program error 6019)
    #[error("Global configuration account not found or invalid. Ensure the program has been properly initialized.")]
    ConfigNotFound,
}

// Update the From implementation for anchor_client::ClientError to use our mapping
impl From<anchor_client::ClientError> for TallyError {
    fn from(error: anchor_client::ClientError) -> Self {
        Self::from_anchor_client_error(error)
    }
}

// Update the From implementation for anchor_lang::error::Error to use our mapping
impl From<anchor_lang::error::Error> for TallyError {
    fn from(error: anchor_lang::error::Error) -> Self {
        Self::from_anchor_error(error)
    }
}

impl From<String> for TallyError {
    fn from(msg: String) -> Self {
        Self::Generic(msg)
    }
}

impl From<&str> for TallyError {
    fn from(msg: &str) -> Self {
        Self::Generic(msg.to_string())
    }
}

impl From<anchor_lang::prelude::ProgramError> for TallyError {
    fn from(error: anchor_lang::prelude::ProgramError) -> Self {
        Self::Generic(format!("Program error: {error:?}"))
    }
}

impl From<anyhow::Error> for TallyError {
    fn from(error: anyhow::Error) -> Self {
        Self::Generic(error.to_string())
    }
}

impl TallyError {
    /// Map program error codes to specific `TallyError` variants
    ///
    /// This function takes an Anchor error and attempts to map it to a more specific
    /// `TallyError` variant based on the error code. If no specific mapping exists,
    /// it returns the original Anchor error wrapped in `TallyError::Anchor`.
    ///
    /// # Arguments
    /// * `anchor_error` - The Anchor error to map
    ///
    /// # Returns
    /// * `TallyError` - The mapped specific error variant or generic Anchor error
    #[must_use]
    pub fn from_anchor_error(anchor_error: anchor_lang::error::Error) -> Self {
        use anchor_lang::error::Error;

        match &anchor_error {
            Error::AnchorError(anchor_err) => {
                // Map specific error codes to our custom variants
                // Anchor assigns error codes starting from 6000 for custom errors
                match anchor_err.error_code_number {
                    6012 => Self::InvalidPayerTokenAccount,
                    6013 => Self::InvalidPayeeTreasuryAccount,
                    6014 => Self::InvalidPlatformTreasuryAccount,
                    6015 => Self::InvalidUsdcMint,
                    6016 => Self::PayeeNotFound,
                    6017 => Self::PaymentTermsNotFound,
                    6018 => Self::PaymentAgreementNotFound,
                    6019 => Self::ConfigNotFound,
                    // For any other error codes, fall back to the generic Anchor error
                    _ => Self::Anchor(anchor_error),
                }
            }
            // For non-AnchorError variants, use the generic Anchor wrapper
            Error::ProgramError(_) => Self::Anchor(anchor_error),
        }
    }

    /// Convenience method to map Anchor client errors to specific `TallyError` variants
    ///
    /// # Arguments
    /// * `client_error` - The Anchor client error to map
    ///
    /// # Returns
    /// * `TallyError` - The mapped specific error variant or generic client error
    pub fn from_anchor_client_error(client_error: anchor_client::ClientError) -> Self {
        // Check if the client error contains a program error we can map
        if let anchor_client::ClientError::SolanaClientError(solana_err) = &client_error {
            // Use get_transaction_error() method as suggested by the compiler
            if let Some(
                anchor_client::solana_sdk::transaction::TransactionError::InstructionError(
                    _,
                    anchor_client::solana_sdk::instruction::InstructionError::Custom(error_code),
                ),
            ) = solana_err.get_transaction_error()
            {
                // Map specific program error codes
                match error_code {
                    6012 => return Self::InvalidPayerTokenAccount,
                    6013 => return Self::InvalidPayeeTreasuryAccount,
                    6014 => return Self::InvalidPlatformTreasuryAccount,
                    6015 => return Self::InvalidUsdcMint,
                    6016 => return Self::PayeeNotFound,
                    6017 => return Self::PaymentTermsNotFound,
                    6018 => return Self::PaymentAgreementNotFound,
                    6019 => return Self::ConfigNotFound,
                    _ => {} // Fall through to generic handling
                }
            }
        }

        // If no specific mapping found, use the generic client error wrapper
        Self::AnchorClient(Box::new(client_error))
    }
}
