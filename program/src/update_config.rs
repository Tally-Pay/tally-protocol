use anchor_lang::prelude::*;

use crate::{errors::RecurringPaymentError, events::ConfigUpdated, state::Config};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct UpdateConfigArgs {
    pub keeper_fee_bps: Option<u16>,
    pub max_withdrawal_amount: Option<u64>,
    pub max_grace_period_seconds: Option<u64>,
    pub min_platform_fee_bps: Option<u16>,
    pub max_platform_fee_bps: Option<u16>,
    pub min_period_seconds: Option<u64>,
    pub default_allowance_periods: Option<u8>,
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump
    )]
    pub config: Account<'info, Config>,

    pub platform_authority: Signer<'info>,
}

pub fn handler(ctx: Context<UpdateConfig>, args: UpdateConfigArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    // Validate caller is platform authority
    require!(
        ctx.accounts.platform_authority.key() == config.platform_authority,
        RecurringPaymentError::Unauthorized
    );

    // Track if any changes were made by checking all Option fields
    let has_update = args.keeper_fee_bps.is_some()
        || args.max_withdrawal_amount.is_some()
        || args.max_grace_period_seconds.is_some()
        || args.min_platform_fee_bps.is_some()
        || args.max_platform_fee_bps.is_some()
        || args.min_period_seconds.is_some()
        || args.default_allowance_periods.is_some();

    // Require at least one field to be updated
    require!(has_update, RecurringPaymentError::InvalidConfiguration);

    // Update keeper fee if provided
    if let Some(keeper_fee) = args.keeper_fee_bps {
        require!(
            keeper_fee <= 100,
            RecurringPaymentError::InvalidConfiguration
        );
        config.keeper_fee_bps = keeper_fee;
    }

    // Update max withdrawal if provided
    if let Some(max_withdrawal) = args.max_withdrawal_amount {
        require!(max_withdrawal > 0, RecurringPaymentError::InvalidConfiguration);
        config.max_withdrawal_amount = max_withdrawal;
    }

    // Update max grace period if provided
    if let Some(max_grace) = args.max_grace_period_seconds {
        require!(max_grace > 0, RecurringPaymentError::InvalidConfiguration);
        config.max_grace_period_seconds = max_grace;
    }

    // Update fee bounds if provided (validate min <= max)
    if let Some(min_fee) = args.min_platform_fee_bps {
        if let Some(max_fee) = args.max_platform_fee_bps {
            require!(
                min_fee <= max_fee,
                RecurringPaymentError::InvalidConfiguration
            );
            config.min_platform_fee_bps = min_fee;
            config.max_platform_fee_bps = max_fee;
        } else {
            require!(
                min_fee <= config.max_platform_fee_bps,
                RecurringPaymentError::InvalidConfiguration
            );
            config.min_platform_fee_bps = min_fee;
        }
    } else if let Some(max_fee) = args.max_platform_fee_bps {
        require!(
            config.min_platform_fee_bps <= max_fee,
            RecurringPaymentError::InvalidConfiguration
        );
        config.max_platform_fee_bps = max_fee;
    }

    // Update min period if provided
    if let Some(min_period) = args.min_period_seconds {
        require!(min_period > 0, RecurringPaymentError::InvalidConfiguration);
        config.min_period_seconds = min_period;
    }

    // Update default allowance periods if provided
    if let Some(allowance_periods) = args.default_allowance_periods {
        require!(
            allowance_periods > 0,
            RecurringPaymentError::InvalidConfiguration
        );
        config.default_allowance_periods = allowance_periods;
    }

    // Emit comprehensive update event
    emit!(ConfigUpdated {
        keeper_fee_bps: config.keeper_fee_bps,
        max_withdrawal_amount: config.max_withdrawal_amount,
        max_grace_period_seconds: config.max_grace_period_seconds,
        min_platform_fee_bps: config.min_platform_fee_bps,
        max_platform_fee_bps: config.max_platform_fee_bps,
        updated_by: ctx.accounts.platform_authority.key(),
    });

    Ok(())
}
