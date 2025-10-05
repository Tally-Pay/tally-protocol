use anchor_lang::prelude::*;

/// Custom error codes for the subscription program
///
/// Note: Anchor automatically assigns error codes starting from 6000.
/// The error codes in comments (1001-1007) represent the logical PRD mapping,
/// but Anchor will assign them as 6000-6006 respectively.
#[error_code]
pub enum SubscriptionError {
    /// Error Code: 6000 (maps to PRD 1001)
    /// When delegate allowance is insufficient for subscription payments
    #[msg(
        "Insufficient USDC allowance. Please approve a higher allowance for subscription payments."
    )]
    InsufficientAllowance,

    /// Error Code: 6001 (maps to PRD 1002)
    /// When user has insufficient USDC balance for subscription
    #[msg("Insufficient USDC funds in your account to complete the subscription payment.")]
    InsufficientFunds,

    /// Error Code: 6002 (maps to PRD 1003)
    /// When renewal attempt is made after grace period has expired
    #[msg("Subscription renewal window has expired. Grace period has passed.")]
    PastGrace,

    /// Error Code: 6003 (maps to PRD 1004)
    /// When subscription or plan is marked as inactive
    #[msg("Subscription or plan is inactive and cannot be used for operations.")]
    Inactive,

    /// Error Code: 6004 (maps to PRD 1005)
    /// When incorrect token mint is provided (expecting USDC)
    #[msg("Invalid token mint provided. Only USDC is supported for subscriptions.")]
    WrongMint,

    /// Error Code: 6005 (maps to PRD 1006)
    /// When PDA seeds don't match expected values
    #[msg("Invalid PDA seeds provided. Account derivation failed.")]
    BadSeeds,

    /// Error Code: 6006 (maps to PRD 1007)
    /// When plan configuration is invalid (price, period, etc.)
    #[msg("Invalid plan configuration. Check price, period, or other plan parameters.")]
    InvalidPlan,

    /// Error Code: 6007
    /// When arithmetic operations would overflow/underflow
    #[msg("Arithmetic operation would result in overflow or underflow.")]
    ArithmeticError,

    /// Error Code: 6008
    /// When subscription is already active and cannot be started again
    #[msg("Subscription is already active and cannot be started again.")]
    AlreadyActive,

    /// Error Code: 6009
    /// When trying to renew a subscription that's not due yet
    #[msg("Subscription is not due for renewal yet.")]
    NotDue,

    /// Error Code: 6010
    /// When unauthorized access is attempted
    #[msg("Unauthorized access. Only the merchant or platform admin can perform this action.")]
    Unauthorized,

    /// Error Code: 6011
    /// When subscription has already been canceled
    #[msg("Subscription has already been canceled and cannot be operated on.")]
    AlreadyCanceled,

    /// Error Code: 6012
    /// When provided subscriber token account is invalid or cannot be deserialized
    #[msg("Invalid subscriber token account. Ensure the account is a valid USDC token account owned by the subscriber.")]
    InvalidSubscriberTokenAccount,

    /// Error Code: 6013
    /// When provided merchant treasury token account is invalid or cannot be deserialized
    #[msg("Invalid merchant treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidMerchantTreasuryAccount,

    /// Error Code: 6014
    /// When provided platform treasury token account is invalid or cannot be deserialized
    #[msg("Invalid platform treasury token account. Ensure the account is a valid USDC token account.")]
    InvalidPlatformTreasuryAccount,

    /// Error Code: 6015
    /// When provided USDC mint account is invalid or cannot be deserialized
    #[msg("Invalid USDC mint account. Ensure the account is a valid token mint account.")]
    InvalidUsdcMint,

    /// Error Code: 6016
    /// When a required merchant account is missing or invalid
    #[msg(
        "Merchant account not found or invalid. Ensure the merchant has been properly initialized."
    )]
    MerchantNotFound,

    /// Error Code: 6017
    /// When a required plan account is missing or invalid
    #[msg("Subscription plan not found or invalid. Ensure the plan exists and belongs to the specified merchant.")]
    PlanNotFound,

    /// Error Code: 6018
    /// When a required subscription account is missing or invalid
    #[msg("Subscription not found or invalid. Ensure the subscription exists for this plan and subscriber.")]
    SubscriptionNotFound,

    /// Error Code: 6019
    /// When the global configuration account is missing or invalid
    #[msg("Global configuration account not found or invalid. Ensure the program has been properly initialized.")]
    ConfigNotFound,

    /// Error Code: 6020
    /// When the program data account is invalid or cannot be deserialized
    #[msg("Invalid program data account. Ensure the account is the correct program data account for this program.")]
    InvalidProgramData,

    /// Error Code: 6021
    /// When attempting to accept authority transfer but no transfer is pending
    #[msg("No pending authority transfer. A transfer must be initiated before it can be accepted.")]
    NoPendingTransfer,

    /// Error Code: 6022
    /// When attempting to initiate authority transfer but one is already pending
    #[msg("Authority transfer already pending. Complete or cancel the current transfer before initiating a new one.")]
    TransferAlreadyPending,
}
