use crate::{errors::RecurringPaymentError, events::*, state::*};
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Revoke, Token, TokenAccount};

/// Arguments for canceling an active payment_agreement.
///
/// # Rate Limiting Considerations
///
/// This instruction has **no on-chain rate limiting** by design. While cancellation spam
/// is the **cheapest attack vector**, it is also the **lowest impact** since users can
/// only cancel their own subscriptions (self-inflicted spam).
///
/// ## Economic Deterrence
/// - **Transaction Fee**: 0.000005 SOL (~$0.0007) per cancellation
/// - **No Rent Refund**: PaymentAgreement account remains (can be reactivated)
/// - **Total Cost**: ~$0.0007 per cancellation (cheapest operation)
///
/// **Cancellation Spam Cost**: Repeatedly canceling subscriptions:
/// - 1,000 cancellations: 0.005 SOL (~$0.70)
/// - **Lowest cost attack**, but also **lowest impact** (affects only attacker)
///
/// ## Why Cancellation Spam is Low Priority
///
/// 1. **Self-Inflicted**: Users can only cancel their own subscriptions, not others'
/// 2. **No State Bloat**: Cancellation doesn't create new accounts
/// 3. **Idempotent**: Canceling already-canceled subscriptions is safe (no-op)
/// 4. **No System Impact**: Event spam doesn't affect protocol operations
/// 5. **Economic Prerequisite**: Requires pre-existing subscriptions to cancel
///
/// **Assessment**: Cancellation spam is a nuisance attack with minimal operational impact.
///
/// ## Off-Chain Monitoring
///
/// Recommended monitoring thresholds (see `/docs/SPAM_DETECTION.md`):
/// - **Info Alert**: >10 cancellations per account per hour
/// - **Pattern Alert**: Repeated cancel-reactivate cycles on same payment_agreement
/// - **Volume Alert**: Unusual spike in system-wide cancellation rate
///
/// Detection is primarily for **observability and abuse prevention**, not critical
/// system protection, since the impact is limited to the attacker's own subscriptions.
///
/// ## Attack Scenarios
///
/// ### Cancellation Spam
/// **Attack**: User repeatedly cancels their own subscriptions to generate event noise.
/// **Cost**: ~$0.0007 per cancellation (cheapest attack).
/// **Impact**: Low - only affects attacker's subscriptions, no state bloat.
/// **Detection**: Monitor per-account cancellation frequency.
/// **Mitigation**: RPC rate limiting to 5 cancellations per hour per account.
///
/// ### Cancel-Reactivate Churn
/// **Attack**: User alternates between canceling and reactivating subscriptions.
/// **Cost**: ~$0.002 per cycle (cancel + reactivate).
/// **Impact**: Low - generates event noise but doesn't affect other users.
/// **Detection**: Track payment_agreement state flip frequency.
/// **Mitigation**: Application-layer cooldown between state changes.
///
/// ## Why No On-Chain Rate Limiting?
///
/// On-chain rate limiting for cancellation was deliberately not implemented because:
/// 1. **Low Impact**: Self-inflicted spam doesn't affect other users or system stability
/// 2. **User Rights**: Users should be able to cancel subscriptions freely
/// 3. **State Efficiency**: Avoiding rate limit state keeps accounts lean
/// 4. **Idempotency**: Repeated cancellations are safe and don't cause issues
///
/// For comprehensive rate limiting strategy, see `/docs/RATE_LIMITING_STRATEGY.md`.
///
/// ## Mitigation Recommendations
///
/// 1. **RPC Layer**: Limit cancellations to 5-10 per hour per account (low priority)
/// 2. **Indexer**: Monitor cancellation patterns for abuse detection
/// 3. **Dashboard**: Track cancellation rates for merchant analytics
/// 4. **Idempotency**: Already implemented - safe to call multiple times
///
/// See `/docs/OPERATIONAL_PROCEDURES.md` for incident response procedures.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PauseAgreementArgs {
    // No args needed for cancellation
}

#[derive(Accounts)]
pub struct PauseAgreement<'info> {
    #[account(
        mut,
        seeds = [b"payment_agreement", payment_terms.key().as_ref(), payer.key().as_ref()],
        bump = payment_agreement.bump,
        has_one = payer @ RecurringPaymentError::Unauthorized
    )]
    pub payment_agreement: Account<'info, PaymentAgreement>,

    pub payment_terms: Account<'info, PaymentTerms>,

    #[account(
        seeds = [b"merchant", merchant.authority.as_ref()],
        bump = merchant.bump
    )]
    pub merchant: Account<'info, Merchant>,

    pub payer: Signer<'info>,

    /// Subscriber's USDC token account where delegate approval will be revoked
    /// CHECK: Validated as USDC token account in handler
    #[account(mut)]
    pub payer_usdc_ata: UncheckedAccount<'info>,

    /// Program PDA that acts as delegate - used to validate delegate identity before revocation
    /// CHECK: PDA derived from program, validated in handler
    #[account(
        seeds = [b"delegate"],
        bump
    )]
    pub program_delegate: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
}

pub fn handler(ctx: Context<PauseAgreement>, _args: PauseAgreementArgs) -> Result<()> {
    let payment_agreement = &mut ctx.accounts.payment_agreement;
    let payment_terms = &ctx.accounts.payment_terms;
    let merchant = &ctx.accounts.merchant;

    // Deserialize and validate payer's token account
    let subscriber_ata_data: TokenAccount =
        TokenAccount::try_deserialize(&mut ctx.accounts.payer_usdc_ata.data.borrow().as_ref())
            .map_err(|_| RecurringPaymentError::InvalidSubscriberTokenAccount)?;

    // Validate token account ownership and mint
    if subscriber_ata_data.owner != ctx.accounts.payer.key() {
        return Err(RecurringPaymentError::Unauthorized.into());
    }

    if subscriber_ata_data.mint != merchant.usdc_mint {
        return Err(RecurringPaymentError::WrongMint.into());
    }

    // Validate program delegate PDA derivation to ensure correct delegate account
    let (expected_delegate_pda, _expected_bump) =
        Pubkey::find_program_address(&[b"delegate"], ctx.program_id);
    require!(
        ctx.accounts.program_delegate.key() == expected_delegate_pda,
        RecurringPaymentError::BadSeeds
    );

    // Revoke delegate approval to prevent further renewals
    //
    // GLOBAL DELEGATE REVOCATION:
    //
    // This protocol uses a single global delegate PDA for all merchants and subscriptions.
    // When a user revokes the delegate, it affects ALL subscriptions using this token account,
    // not just the payment_agreement being canceled.
    //
    // Revocation behavior:
    // 1. Revoking the global delegate here affects ALL subscriptions on this token account
    // 2. All merchants' subscriptions using this account will stop renewing
    // 3. User must re-approve the global delegate to reactivate any subscriptions
    //
    // This is intentional behavior that provides users control over their payment_agreement spending.
    // Users can:
    // - Cancel individual subscriptions without revoking delegate (payment_agreement.active = false)
    // - Revoke delegate to stop ALL subscriptions on this account at once
    // - Re-approve delegate later to reactivate subscriptions
    //
    // We only revoke if the current delegate matches our program's global delegate PDA.
    // This prevents revoking unrelated delegations to other programs.
    if let Some(current_delegate) = Option::<Pubkey>::from(subscriber_ata_data.delegate) {
        if current_delegate == expected_delegate_pda {
            let revoke_accounts = Revoke {
                source: ctx.accounts.payer_usdc_ata.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            };

            token::revoke(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                revoke_accounts,
            ))?;
        }
        // If delegate is not ours, skip revocation (already revoked or delegated elsewhere)
    }

    // Make it idempotent - it's safe to "cancel" an already canceled payment_agreement
    // No need to check if already canceled, just set active = false
    payment_agreement.active = false;

    // Emit PaymentAgreementPaused event
    emit!(PaymentAgreementPaused {
        merchant: merchant.key(),
        payment_terms: payment_terms.key(),
        payer: ctx.accounts.payer.key(),
    });

    Ok(())
}
