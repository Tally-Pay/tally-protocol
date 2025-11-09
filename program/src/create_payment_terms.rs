use crate::constants::MAX_PLAN_PRICE_USDC;
use crate::errors::RecurringPaymentError;
use crate::state::{Payee, PaymentTerms};
use anchor_lang::prelude::*;

/// Arguments for creating payment terms.
///
/// # Rate Limiting Considerations
///
/// This instruction has **no on-chain rate limiting** by design. Spam prevention is handled
/// through economic incentives and off-chain monitoring:
///
/// ## Economic Deterrence
/// - **Transaction Fee**: 0.000005 SOL (~$0.0007) per payment terms creation
/// - **Rent Deposit**: 0.00089 SOL (~$0.12) per payment terms (129 bytes account size)
/// - **Total Cost**: ~$0.12 per payment terms, making mass spam attacks expensive
///
/// Example: Creating 10,000 fake payment terms costs ~$1,253 (rent + fees), which provides
/// natural spam deterrence without requiring on-chain rate limiting logic.
///
/// ## Off-Chain Monitoring
///
/// Recommended monitoring thresholds (see `/docs/SPAM_DETECTION.md`):
/// - **Critical Alert**: >100 payment terms created per payee per hour
/// - **Warning Alert**: >10 payment terms created per payee per hour
/// - **RPC Rate Limit**: Throttle to 10 payment terms creations per payee per hour
///
/// ## Why No On-Chain Rate Limiting?
///
/// On-chain rate limiting was deliberately not implemented because:
/// 1. **Account Migration Complexity**: Adding rate limit fields (timestamps, counters) to
///    `Payee` accounts requires migrating all existing accounts, risking data loss.
/// 2. **Storage Costs**: Rate limit state increases account size and rent costs for all payees.
/// 3. **Solana Best Practices**: Rate limiting is more effectively handled at the RPC and
///    indexer layers where it's flexible, configurable, and doesn't bloat on-chain state.
/// 4. **Economic Model**: Transaction fees + rent deposits already provide spam deterrence.
///
/// For comprehensive rate limiting strategy, see `/docs/RATE_LIMITING_STRATEGY.md`.
///
/// ## Mitigation Recommendations
///
/// 1. **RPC Layer**: Configure rate limits (e.g., Nginx, `HAProxy`, or RPC provider limits)
/// 2. **Indexer**: Deploy spam detection indexer to monitor payee activity patterns
/// 3. **Dashboard**: Real-time alerting for anomalous payment terms creation rates
/// 4. **Auto-Throttle**: Implement automatic account throttling for detected spam patterns
///
/// See `/docs/OPERATIONAL_PROCEDURES.md` for incident response procedures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CreatePaymentTermsArgs {
    pub terms_id: String,         // Original string terms ID
    pub terms_id_bytes: [u8; 32], // Padded terms_id bytes for PDA seeds (must match SDK calculation)
    pub amount_usdc: u64,         // Amount in USDC microlamports
    pub period_secs: u64,         // Payment period in seconds
}

#[derive(Accounts)]
#[instruction(args: CreatePaymentTermsArgs)]
pub struct CreatePaymentTerms<'info> {
    /// Global configuration account
    #[account(
        seeds = [b"config"],
        bump = config.bump,
        constraint = !config.paused @ RecurringPaymentError::Inactive
    )]
    pub config: Account<'info, crate::state::Config>,

    #[account(
        init,
        payer = authority,
        space = PaymentTerms::SPACE,
        seeds = [b"payment_terms", payee.key().as_ref(), args.terms_id_bytes.as_ref()],
        bump
    )]
    pub payment_terms: Account<'info, PaymentTerms>,

    #[account(
        seeds = [b"payee", authority.key().as_ref()],
        bump = payee.bump,
        has_one = authority
    )]
    pub payee: Account<'info, Payee>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Converts a string to a fixed-size [u8; 32] array
/// Returns an error if the string's byte representation exceeds 32 bytes
/// Pads with zeros if the string is shorter than 32 bytes
fn string_to_bytes32(input: &str) -> Result<[u8; 32]> {
    let bytes = input.as_bytes();

    // Validate that byte length does not exceed 32
    // This prevents silent truncation which could cause collisions
    require!(
        bytes.len() <= 32,
        RecurringPaymentError::InvalidPaymentTerms
    );

    let mut result = [0u8; 32];
    result[..bytes.len()].copy_from_slice(bytes);
    Ok(result)
}

pub fn handler(ctx: Context<CreatePaymentTerms>, args: CreatePaymentTermsArgs) -> Result<()> {
    // Validate amount_usdc > 0
    require!(args.amount_usdc > 0, RecurringPaymentError::InvalidPaymentTerms);

    // Validate amount_usdc <= MAX_PLAN_PRICE_USDC (M-5 security fix)
    //
    // Enforces a maximum price limit to prevent social engineering attacks where payees
    // create payment terms with extreme prices (e.g., u64::MAX) that could mislead payers.
    //
    // Security Impact:
    // - Prevents creation of payment terms with prices near u64::MAX (~18.4 quintillion USDC)
    // - Mitigates social engineering risks from unrealistic price displays
    // - Reduces potential overflow scenarios in downstream calculations
    // - Establishes reasonable ceiling (1 million USDC) for recurring payment services
    //
    // This validation ensures that all payment term prices remain within a realistic range
    // suitable for legitimate recurring payment business models while blocking extreme
    // values that have no valid use case and could enable malicious behavior.
    require!(
        args.amount_usdc <= MAX_PLAN_PRICE_USDC,
        RecurringPaymentError::InvalidPaymentTerms
    );

    // Validate period_secs >= minimum period from config
    require!(
        args.period_secs >= ctx.accounts.config.min_period_seconds,
        RecurringPaymentError::InvalidPaymentTerms
    );

    // Validate terms_id is not empty
    require!(
        !args.terms_id.is_empty(),
        RecurringPaymentError::InvalidPaymentTerms
    );

    // Validate that terms_id_bytes matches the string conversion to ensure consistency
    // This also validates that terms_id byte length <= 32
    let expected_terms_id_bytes = string_to_bytes32(&args.terms_id)?;
    require!(
        args.terms_id_bytes == expected_terms_id_bytes,
        RecurringPaymentError::InvalidPaymentTerms
    );

    let payment_terms = &mut ctx.accounts.payment_terms;
    payment_terms.payee = ctx.accounts.payee.key();
    payment_terms.terms_id = args.terms_id_bytes;
    payment_terms.amount_usdc = args.amount_usdc;
    payment_terms.period_secs = args.period_secs;

    // Get current timestamp for event
    let clock = Clock::get()?;

    // Emit PaymentTermsCreated event
    emit!(crate::events::PaymentTermsCreated {
        payment_terms: payment_terms.key(),
        payee: ctx.accounts.payee.key(),
        terms_id: args.terms_id,
        amount_usdc: args.amount_usdc,
        period_secs: args.period_secs,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
