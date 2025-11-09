use crate::{errors::RecurringPaymentError, events::*, state::*};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CloseAgreementArgs {
    // No args needed for closing
}

#[derive(Accounts)]
pub struct CloseAgreement<'info> {
    #[account(
        mut,
        seeds = [b"payment_agreement", payment_agreement.payment_terms.as_ref(), payer.key().as_ref()],
        bump = payment_agreement.bump,
        has_one = payer @ RecurringPaymentError::Unauthorized,
        constraint = !payment_agreement.active @ RecurringPaymentError::AlreadyActive,
        close = payer
    )]
    pub payment_agreement: Account<'info, PaymentAgreement>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

pub fn handler(ctx: Context<CloseAgreement>, _args: CloseAgreementArgs) -> Result<()> {
    let payment_agreement = &ctx.accounts.payment_agreement;

    // Emit PaymentAgreementClosed event before account is closed
    emit!(PaymentAgreementClosed {
        payment_terms: payment_agreement.payment_terms,
        payer: ctx.accounts.payer.key(),
    });

    // The `close` constraint in the Accounts struct will:
    // 1. Transfer all lamports (rent) to payer account
    // 2. Set payment_agreement account data to all zeros
    // 3. Set payment_agreement account owner to System Program
    // This is handled automatically by Anchor after handler returns Ok(())

    Ok(())
}
