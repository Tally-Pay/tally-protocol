use anchor_lang::prelude::*;

/// Custom error codes for the recurring payments protocol
///
/// Note: Anchor automatically assigns error codes starting from 6000.
#[error_code]
pub enum RecurringPaymentError {
    /// Error Code: 6000
    /// When delegate allowance is insufficient for recurring payments
    #[msg(
        "Insufficient USDC allowance. Approve delegate allowance (recommended: 3x payment amount) to enable recurring payments."
    )]
    InsufficientAllowance,

    /// Error Code: 6001
    /// When user has insufficient USDC balance for payment
    #[msg("Insufficient USDC funds in your account to complete the payment.")]
    InsufficientFunds,

    /// Error Code: 6002
    /// When payment agreement is marked as inactive
    #[msg("Payment agreement is inactive and cannot be used for operations.")]
    Inactive,

    /// Error Code: 6003
    /// When incorrect token mint is provided (expecting USDC)
    #[msg("Invalid token mint provided. Only USDC is supported for payments.")]
    WrongMint,

    /// Error Code: 6004
    /// When PDA seeds don't match expected values
    #[msg("Invalid PDA seeds provided. Account derivation failed.")]
    BadSeeds,

    /// Error Code: 6005
    /// When payment terms configuration is invalid (amount, period, etc.)
    #[msg("Invalid payment terms configuration. Check amount, period, or other parameters.")]
    InvalidPaymentTerms,

    /// Error Code: 6006
    /// When arithmetic operations would overflow/underflow
    #[msg("Arithmetic operation would result in overflow or underflow.")]
    ArithmeticError,

    /// Error Code: 6007
    /// When payment agreement is already active and cannot be started again
    #[msg("Payment agreement is already active and cannot be started again.")]
    AlreadyActive,

    /// Error Code: 6008
    /// When trying to execute a payment that's not due yet
    #[msg("Payment is not due yet. Next payment scheduled for later.")]
    NotDue,

    /// Error Code: 6009
    /// When unauthorized access is attempted
    #[msg("Unauthorized access. Only the payee or platform admin can perform this action.")]
    Unauthorized,

    /// Error Code: 6010
    /// When payment agreement has already been paused
    #[msg("Payment agreement has already been paused and cannot be operated on.")]
    AlreadyPaused,

    /// Error Code: 6011
    /// When provided payer token account is invalid or cannot be deserialized
    #[msg("Invalid payer token account. Ensure the account is a valid USDC token account owned by the payer.")]
    InvalidPayerTokenAccount,

    /// Error Code: 6012
    /// When provided payee treasury token account is invalid or cannot be deserialized
    #[msg("Invalid payee treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidPayeeTreasuryAccount,

    /// Error Code: 6013
    /// When provided platform treasury token account is invalid or cannot be deserialized
    #[msg("Invalid platform treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidPlatformTreasuryAccount,

    /// Error Code: 6014
    /// When provided USDC mint account is invalid or cannot be deserialized
    #[msg("Invalid USDC mint account. Ensure the account is a valid token mint account.")]
    InvalidUsdcMint,

    /// Error Code: 6015
    /// When a required payee account is missing or invalid
    #[msg(
        "Payee account not found or invalid. Ensure the payee has been properly initialized."
    )]
    PayeeNotFound,

    /// Error Code: 6016
    /// When a required payment terms account is missing or invalid
    #[msg("Payment terms not found or invalid. Ensure the terms exist and belong to the specified payee.")]
    PaymentTermsNotFound,

    /// Error Code: 6017
    /// When a required payment agreement account is missing or invalid
    #[msg("Payment agreement not found or invalid. Ensure the agreement exists for these terms and payer.")]
    PaymentAgreementNotFound,

    /// Error Code: 6018
    /// When the global configuration account is missing or invalid
    #[msg("Global configuration account not found or invalid. Ensure the program has been properly initialized.")]
    ConfigNotFound,

    /// Error Code: 6019
    /// When the program data account is invalid or cannot be deserialized
    #[msg("Invalid program data account. Ensure the account is the correct program data account for this program.")]
    InvalidProgramData,

    /// Error Code: 6020
    /// When attempting to accept authority transfer but no transfer is pending
    #[msg(
        "No pending authority transfer. A transfer must be initiated before it can be accepted."
    )]
    NoPendingTransfer,

    /// Error Code: 6021
    /// When attempting to initiate authority transfer but one is already pending
    #[msg("Authority transfer already pending. Complete or cancel the current transfer before initiating a new one.")]
    TransferAlreadyPending,

    /// Error Code: 6022
    /// When withdrawal amount exceeds configured maximum
    #[msg("Withdrawal amount exceeds maximum allowed per transaction. Please reduce the amount or contact platform admin to adjust limits.")]
    WithdrawLimitExceeded,

    /// Error Code: 6023
    /// When authority transfer target is invalid (same as current authority)
    #[msg("Invalid authority transfer target. The new authority must be different from the current authority.")]
    InvalidTransferTarget,

    /// Error Code: 6024
    /// When a monetary amount is invalid (zero, negative, or exceeds limits)
    #[msg("Invalid amount provided. Amount must be greater than zero and within acceptable limits.")]
    InvalidAmount,

    /// Error Code: 6025
    /// When attempting to create payment terms that already exist for this payee
    #[msg("Payment terms with this ID already exist for this payee. Each terms ID must be unique per payee.")]
    PaymentTermsAlreadyExist,

    /// Error Code: 6026
    /// When global configuration parameters are invalid or inconsistent
    #[msg("Invalid configuration parameters. Ensure min/max fee bounds are consistent and all values are within acceptable ranges.")]
    InvalidConfiguration,
}
