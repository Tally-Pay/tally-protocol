//! Transaction building utilities for Tally subscription flows

use crate::{
    ata::{get_associated_token_address_with_program, TokenProgram},
    error::{Result, TallyError},
    pda, program_id,
    program_types::{
        AdminWithdrawFeesArgs, CancelSubscriptionArgs, CreatePlanArgs, InitConfigArgs,
        InitMerchantArgs, Merchant, Plan, StartSubscriptionArgs, UpdateConfigArgs, UpdatePlanArgs,
    },
};
use anchor_client::solana_sdk::instruction::{AccountMeta, Instruction};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use spl_token::instruction::{approve_checked as approve_checked_token, revoke as revoke_token};
use spl_token_2022::instruction::{
    approve_checked as approve_checked_token2022, revoke as revoke_token2022,
};

/// Builder for start subscription transactions (approve → start flow)
#[derive(Clone, Debug, Default)]
pub struct StartSubscriptionBuilder {
    plan: Option<Pubkey>,
    subscriber: Option<Pubkey>,
    payer: Option<Pubkey>,
    allowance_periods: Option<u8>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for cancel subscription transactions (revoke → cancel flow)
#[derive(Clone, Debug, Default)]
pub struct CancelSubscriptionBuilder {
    plan: Option<Pubkey>,
    subscriber: Option<Pubkey>,
    payer: Option<Pubkey>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for create merchant transactions
#[derive(Clone, Debug, Default)]
pub struct CreateMerchantBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    usdc_mint: Option<Pubkey>,
    treasury_ata: Option<Pubkey>,
    platform_fee_bps: Option<u16>,
    program_id: Option<Pubkey>,
}

/// Builder for create plan transactions
#[derive(Clone, Debug, Default)]
pub struct CreatePlanBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    plan_args: Option<CreatePlanArgs>,
    program_id: Option<Pubkey>,
}

/// Builder for update plan transactions
#[derive(Clone, Debug, Default)]
pub struct UpdatePlanBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    plan_key: Option<Pubkey>,
    update_args: Option<UpdatePlanArgs>,
    program_id: Option<Pubkey>,
}

/// Builder for admin fee withdrawal transactions
#[derive(Clone, Debug, Default)]
pub struct AdminWithdrawFeesBuilder {
    platform_authority: Option<Pubkey>,
    platform_treasury_ata: Option<Pubkey>,
    destination_ata: Option<Pubkey>,
    usdc_mint: Option<Pubkey>,
    amount: Option<u64>,
    program_id: Option<Pubkey>,
}

/// Builder for initialize config transactions
#[derive(Clone, Debug, Default)]
pub struct InitConfigBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    config_args: Option<InitConfigArgs>,
    program_id: Option<Pubkey>,
}

/// Builder for renew subscription transactions
#[derive(Clone, Debug, Default)]
pub struct RenewSubscriptionBuilder {
    plan: Option<Pubkey>,
    subscriber: Option<Pubkey>,
    keeper: Option<Pubkey>,
    keeper_ata: Option<Pubkey>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for close subscription transactions
#[derive(Clone, Debug, Default)]
pub struct CloseSubscriptionBuilder {
    plan: Option<Pubkey>,
    subscriber: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for transfer authority transactions
#[derive(Clone, Debug, Default)]
pub struct TransferAuthorityBuilder {
    platform_authority: Option<Pubkey>,
    new_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for accept authority transactions
#[derive(Clone, Debug, Default)]
pub struct AcceptAuthorityBuilder {
    new_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for cancel authority transfer transactions
#[derive(Clone, Debug, Default)]
pub struct CancelAuthorityTransferBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for pause program transactions
#[derive(Clone, Debug, Default)]
pub struct PauseBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for unpause program transactions
#[derive(Clone, Debug, Default)]
pub struct UnpauseBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for update config transactions
#[derive(Clone, Debug, Default)]
pub struct UpdateConfigBuilder {
    platform_authority: Option<Pubkey>,
    keeper_fee_bps: Option<u16>,
    max_withdrawal_amount: Option<u64>,
    max_grace_period_seconds: Option<u64>,
    min_platform_fee_bps: Option<u16>,
    max_platform_fee_bps: Option<u16>,
    min_period_seconds: Option<u64>,
    default_allowance_periods: Option<u8>,
    program_id: Option<Pubkey>,
}

/// Builder for update merchant tier transactions
#[derive(Clone, Debug, Default)]
pub struct UpdateMerchantTierBuilder {
    authority: Option<Pubkey>,
    merchant: Option<Pubkey>,
    new_tier: Option<u8>,
    program_id: Option<Pubkey>,
}

impl StartSubscriptionBuilder {
    /// Create a new start subscription builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the plan PDA
    #[must_use]
    pub const fn plan(mut self, plan: Pubkey) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Set the subscriber pubkey
    #[must_use]
    pub const fn subscriber(mut self, subscriber: Pubkey) -> Self {
        self.subscriber = Some(subscriber);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the allowance periods multiplier (default 3)
    #[must_use]
    pub const fn allowance_periods(mut self, periods: u8) -> Self {
        self.allowance_periods = Some(periods);
        self
    }

    /// Set the token program to use
    #[must_use]
    pub const fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = Some(token_program);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instructions
    ///
    /// # Arguments
    /// * `merchant` - The merchant account data
    /// * `plan_data` - The plan account data
    /// * `platform_treasury_ata` - Platform treasury ATA address
    ///
    /// # Returns
    /// * `Ok(Vec<Instruction>)` - The transaction instructions (`approve_checked` + `start_subscription`)
    /// * `Err(TallyError)` - If building fails
    pub fn build_instructions(
        self,
        merchant: &Merchant,
        plan_data: &Plan,
        platform_treasury_ata: &Pubkey,
    ) -> Result<Vec<Instruction>> {
        let plan = self.plan.ok_or("Plan not set")?;
        let subscriber = self.subscriber.ok_or("Subscriber not set")?;
        let _payer = self.payer.unwrap_or(subscriber);
        let allowance_periods = self.allowance_periods.unwrap_or(3);
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let merchant_pda = pda::merchant_address_with_program_id(&merchant.authority, &program_id);
        let subscription_pda =
            pda::subscription_address_with_program_id(&plan, &subscriber, &program_id);
        let delegate_pda = pda::delegate_address_with_program_id(&merchant_pda, &program_id);
        let subscriber_ata = get_associated_token_address_with_program(
            &subscriber,
            &merchant.usdc_mint,
            token_program,
        )?;

        // Calculate allowance amount based on plan price and periods
        let allowance_amount = plan_data
            .price_usdc
            .checked_mul(u64::from(allowance_periods))
            .ok_or_else(|| TallyError::Generic("Arithmetic overflow".to_string()))?;

        // Create approve_checked instruction using the correct token program
        let approve_ix = match token_program {
            TokenProgram::Token => approve_checked_token(
                &token_program.program_id(),
                &subscriber_ata,
                &merchant.usdc_mint,
                &delegate_pda, // Program delegate PDA
                &subscriber,   // Subscriber as owner
                &[],           // No additional signers
                allowance_amount,
                6, // USDC decimals
            )?,
            TokenProgram::Token2022 => approve_checked_token2022(
                &token_program.program_id(),
                &subscriber_ata,
                &merchant.usdc_mint,
                &delegate_pda, // Program delegate PDA
                &subscriber,   // Subscriber as owner
                &[],           // No additional signers
                allowance_amount,
                6, // USDC decimals
            )?,
        };

        // Create start_subscription instruction
        let start_sub_accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(subscription_pda, false),      // subscription (PDA)
            AccountMeta::new_readonly(plan, false),         // plan
            AccountMeta::new_readonly(merchant_pda, false), // merchant
            AccountMeta::new(subscriber, true),             // subscriber (signer)
            AccountMeta::new(subscriber_ata, false),        // subscriber_usdc_ata
            AccountMeta::new(merchant.treasury_ata, false), // merchant_treasury_ata
            AccountMeta::new(*platform_treasury_ata, false), // platform_treasury_ata
            AccountMeta::new_readonly(merchant.usdc_mint, false), // usdc_mint
            AccountMeta::new_readonly(delegate_pda, false), // program_delegate
            AccountMeta::new_readonly(token_program.program_id(), false), // token_program
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ];

        let start_sub_args = StartSubscriptionArgs { allowance_periods };
        let start_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "start_subscription")
            data.extend_from_slice(&[167, 59, 160, 222, 194, 175, 3, 13]);
            borsh::to_writer(&mut data, &start_sub_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        let start_sub_ix = Instruction {
            program_id,
            accounts: start_sub_accounts,
            data: start_sub_data,
        };

        Ok(vec![approve_ix, start_sub_ix])
    }
}

impl CancelSubscriptionBuilder {
    /// Create a new cancel subscription builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the plan PDA
    #[must_use]
    pub const fn plan(mut self, plan: Pubkey) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Set the subscriber pubkey
    #[must_use]
    pub const fn subscriber(mut self, subscriber: Pubkey) -> Self {
        self.subscriber = Some(subscriber);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the token program to use
    #[must_use]
    pub const fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = Some(token_program);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instructions
    ///
    /// # Arguments
    /// * `merchant` - The merchant account data
    ///
    /// # Returns
    /// * `Ok(Vec<Instruction>)` - The transaction instructions (revoke + `cancel_subscription`)
    /// * `Err(TallyError)` - If building fails
    pub fn build_instructions(self, merchant: &Merchant) -> Result<Vec<Instruction>> {
        let plan = self.plan.ok_or("Plan not set")?;
        let subscriber = self.subscriber.ok_or("Subscriber not set")?;
        let _payer = self.payer.unwrap_or(subscriber);
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let subscription_pda =
            pda::subscription_address_with_program_id(&plan, &subscriber, &program_id);
        let subscriber_ata = get_associated_token_address_with_program(
            &subscriber,
            &merchant.usdc_mint,
            token_program,
        )?;

        // Create revoke instruction using the correct token program
        let revoke_ix = match token_program {
            TokenProgram::Token => revoke_token(
                &token_program.program_id(),
                &subscriber_ata,
                &subscriber, // Subscriber as owner
                &[],         // No additional signers
            )?,
            TokenProgram::Token2022 => revoke_token2022(
                &token_program.program_id(),
                &subscriber_ata,
                &subscriber, // Subscriber as owner
                &[],         // No additional signers
            )?,
        };

        // Create cancel_subscription instruction
        let merchant_pda = pda::merchant_address_with_program_id(&merchant.authority, &program_id);
        let cancel_sub_accounts = vec![
            AccountMeta::new(subscription_pda, false), // subscription (PDA)
            AccountMeta::new_readonly(plan, false),    // plan
            AccountMeta::new_readonly(merchant_pda, false), // merchant
            AccountMeta::new_readonly(subscriber, true), // subscriber (signer)
        ];

        let cancel_sub_args = CancelSubscriptionArgs;
        let cancel_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "cancel_subscription")
            data.extend_from_slice(&[60, 139, 189, 242, 191, 208, 143, 18]);
            borsh::to_writer(&mut data, &cancel_sub_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        let cancel_sub_ix = Instruction {
            program_id,
            accounts: cancel_sub_accounts,
            data: cancel_sub_data,
        };

        Ok(vec![revoke_ix, cancel_sub_ix])
    }
}

impl CreateMerchantBuilder {
    /// Create a new create merchant builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the merchant authority
    #[must_use]
    pub const fn authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the USDC mint
    #[must_use]
    pub const fn usdc_mint(mut self, usdc_mint: Pubkey) -> Self {
        self.usdc_mint = Some(usdc_mint);
        self
    }

    /// Set the treasury ATA
    #[must_use]
    pub const fn treasury_ata(mut self, treasury_ata: Pubkey) -> Self {
        self.treasury_ata = Some(treasury_ata);
        self
    }

    /// Set the platform fee basis points
    #[must_use]
    pub const fn platform_fee_bps(mut self, fee_bps: u16) -> Self {
        self.platform_fee_bps = Some(fee_bps);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `init_merchant` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let usdc_mint = self.usdc_mint.ok_or("USDC mint not set")?;
        let treasury_ata = self.treasury_ata.ok_or("Treasury ATA not set")?;
        let platform_fee_bps = self.platform_fee_bps.unwrap_or(0);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let merchant_pda = pda::merchant_address_with_program_id(&authority, &program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(merchant_pda, false),          // merchant (PDA)
            AccountMeta::new(authority, true),              // authority (signer)
            AccountMeta::new_readonly(usdc_mint, false),    // usdc_mint
            AccountMeta::new_readonly(treasury_ata, false), // treasury_ata
            AccountMeta::new_readonly(spl_token::id(), false), // token_program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(system_program::ID, false),                 // system_program
        ];

        let args = InitMerchantArgs {
            usdc_mint,
            treasury_ata,
            platform_fee_bps,
        };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "init_merchant")
            data.extend_from_slice(&[209, 11, 214, 195, 222, 157, 124, 192]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl CreatePlanBuilder {
    /// Create a new create plan builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the merchant authority
    #[must_use]
    pub const fn authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the plan creation arguments
    #[must_use]
    pub fn plan_args(mut self, args: CreatePlanArgs) -> Self {
        self.plan_args = Some(args);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `create_plan` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let _payer = self.payer.unwrap_or(authority);
        let plan_args = self.plan_args.ok_or("Plan args not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let merchant_pda = pda::merchant_address_with_program_id(&authority, &program_id);
        let plan_pda =
            pda::plan_address_with_program_id(&merchant_pda, &plan_args.plan_id_bytes, &program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(plan_pda, false),              // plan (PDA)
            AccountMeta::new_readonly(merchant_pda, false), // merchant
            AccountMeta::new(authority, true),              // authority (signer)
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ];

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "create_plan")
            data.extend_from_slice(&[77, 43, 141, 254, 212, 118, 41, 186]);
            borsh::to_writer(&mut data, &plan_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl UpdatePlanBuilder {
    /// Create a new update plan builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the merchant authority
    #[must_use]
    pub const fn authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the plan account key
    #[must_use]
    pub const fn plan_key(mut self, plan_key: Pubkey) -> Self {
        self.plan_key = Some(plan_key);
        self
    }

    /// Set the plan update arguments
    #[must_use]
    pub fn update_args(mut self, args: UpdatePlanArgs) -> Self {
        self.update_args = Some(args);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Arguments
    /// * `merchant` - The merchant account data for validation
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `update_plan` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self, merchant: &Merchant) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let plan_key = self.plan_key.ok_or("Plan key not set")?;
        let update_args = self.update_args.ok_or("Update args not set")?;

        // Validate that authority matches merchant authority
        if authority != merchant.authority {
            return Err(TallyError::Generic(
                "Authority does not match merchant authority".to_string(),
            ));
        }

        // Validate that at least one field is being updated
        if !update_args.has_updates() {
            return Err(TallyError::Generic(
                "No updates specified in UpdatePlanArgs".to_string(),
            ));
        }

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let merchant_pda = pda::merchant_address_with_program_id(&authority, &program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(plan_key, false),              // plan (PDA, mutable)
            AccountMeta::new_readonly(merchant_pda, false), // merchant
            AccountMeta::new(authority, true),              // authority (signer)
        ];

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "update_plan")
            data.extend_from_slice(&[219, 200, 88, 176, 158, 63, 253, 127]);
            borsh::to_writer(&mut data, &update_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl AdminWithdrawFeesBuilder {
    /// Create a new admin withdraw fees builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the platform authority
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the platform treasury ATA (source of funds)
    #[must_use]
    pub const fn platform_treasury_ata(mut self, platform_treasury_ata: Pubkey) -> Self {
        self.platform_treasury_ata = Some(platform_treasury_ata);
        self
    }

    /// Set the destination ATA (where funds will be sent)
    #[must_use]
    pub const fn destination_ata(mut self, destination_ata: Pubkey) -> Self {
        self.destination_ata = Some(destination_ata);
        self
    }

    /// Set the USDC mint
    #[must_use]
    pub const fn usdc_mint(mut self, usdc_mint: Pubkey) -> Self {
        self.usdc_mint = Some(usdc_mint);
        self
    }

    /// Set the amount to withdraw
    #[must_use]
    pub const fn amount(mut self, amount: u64) -> Self {
        self.amount = Some(amount);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `admin_withdraw_fees` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;
        let platform_treasury_ata = self
            .platform_treasury_ata
            .ok_or("Platform treasury ATA not set")?;
        let destination_ata = self.destination_ata.ok_or("Destination ATA not set")?;
        let usdc_mint = self.usdc_mint.ok_or("USDC mint not set")?;
        let amount = self.amount.ok_or("Amount not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(platform_authority, true),     // platform_authority (signer)
            AccountMeta::new(platform_treasury_ata, false), // platform_treasury_ata (source, mutable)
            AccountMeta::new(destination_ata, false), // platform_destination_ata (destination, mutable)
            AccountMeta::new_readonly(usdc_mint, false), // usdc_mint (readonly)
            AccountMeta::new_readonly(spl_token::id(), false), // token_program (readonly)
        ];

        let args = AdminWithdrawFeesArgs { amount };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "admin_withdraw_fees")
            data.extend_from_slice(&[236, 186, 208, 151, 204, 142, 168, 30]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl InitConfigBuilder {
    /// Create a new initialize config builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the authority (signer)
    #[must_use]
    pub const fn authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the transaction payer
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    /// Set the configuration arguments
    #[must_use]
    pub const fn config_args(mut self, args: InitConfigArgs) -> Self {
        self.config_args = Some(args);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `init_config` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let config_args = self.config_args.ok_or("Config args not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA)
            AccountMeta::new(authority, true),   // authority (signer)
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ];

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "init_config")
            data.extend_from_slice(&[23, 235, 115, 232, 168, 96, 1, 231]);
            borsh::to_writer(&mut data, &config_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl RenewSubscriptionBuilder {
    /// Create a new renew subscription builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the plan PDA
    #[must_use]
    pub const fn plan(mut self, plan: Pubkey) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Set the subscriber pubkey
    #[must_use]
    pub const fn subscriber(mut self, subscriber: Pubkey) -> Self {
        self.subscriber = Some(subscriber);
        self
    }

    /// Set the keeper (transaction caller) who executes the renewal
    #[must_use]
    pub const fn keeper(mut self, keeper: Pubkey) -> Self {
        self.keeper = Some(keeper);
        self
    }

    /// Set the keeper's USDC ATA where keeper fee will be sent
    #[must_use]
    pub const fn keeper_ata(mut self, keeper_ata: Pubkey) -> Self {
        self.keeper_ata = Some(keeper_ata);
        self
    }

    /// Set the token program to use
    #[must_use]
    pub const fn token_program(mut self, token_program: TokenProgram) -> Self {
        self.token_program = Some(token_program);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Arguments
    /// * `merchant` - The merchant account data
    /// * `plan_data` - The plan account data
    /// * `platform_treasury_ata` - Platform treasury ATA address
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `renew_subscription` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(
        self,
        merchant: &Merchant,
        _plan_data: &Plan,
        platform_treasury_ata: &Pubkey,
    ) -> Result<Instruction> {
        let plan = self.plan.ok_or("Plan not set")?;
        let subscriber = self.subscriber.ok_or("Subscriber not set")?;
        let keeper = self.keeper.ok_or("Keeper not set")?;
        let keeper_ata = self.keeper_ata.ok_or("Keeper ATA not set")?;
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let merchant_pda = pda::merchant_address_with_program_id(&merchant.authority, &program_id);
        let subscription_pda =
            pda::subscription_address_with_program_id(&plan, &subscriber, &program_id);
        let delegate_pda = pda::delegate_address_with_program_id(&merchant_pda, &program_id);
        let subscriber_ata = get_associated_token_address_with_program(
            &subscriber,
            &merchant.usdc_mint,
            token_program,
        )?;

        // Create renew_subscription instruction
        let renew_sub_accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(subscription_pda, false),      // subscription (PDA, mutable)
            AccountMeta::new_readonly(plan, false),         // plan
            AccountMeta::new_readonly(merchant_pda, false), // merchant
            AccountMeta::new(subscriber_ata, false),        // subscriber_usdc_ata (mutable)
            AccountMeta::new(merchant.treasury_ata, false), // merchant_treasury_ata (mutable)
            AccountMeta::new(*platform_treasury_ata, false), // platform_treasury_ata (mutable)
            AccountMeta::new(keeper, true),                 // keeper (signer, mutable for fees)
            AccountMeta::new(keeper_ata, false),            // keeper_usdc_ata (mutable)
            AccountMeta::new_readonly(merchant.usdc_mint, false), // usdc_mint
            AccountMeta::new_readonly(delegate_pda, false), // program_delegate
            AccountMeta::new_readonly(token_program.program_id(), false), // token_program
        ];

        let renew_sub_args = crate::program_types::RenewSubscriptionArgs {};
        let renew_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "renew_subscription")
            data.extend_from_slice(&[45, 75, 154, 194, 160, 10, 111, 183]);
            borsh::to_writer(&mut data, &renew_sub_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts: renew_sub_accounts,
            data: renew_sub_data,
        })
    }
}

impl CloseSubscriptionBuilder {
    /// Create a new close subscription builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the plan PDA
    #[must_use]
    pub const fn plan(mut self, plan: Pubkey) -> Self {
        self.plan = Some(plan);
        self
    }

    /// Set the subscriber pubkey
    #[must_use]
    pub const fn subscriber(mut self, subscriber: Pubkey) -> Self {
        self.subscriber = Some(subscriber);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `close_subscription` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let plan = self.plan.ok_or("Plan not set")?;
        let subscriber = self.subscriber.ok_or("Subscriber not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute subscription PDA
        let subscription_pda =
            pda::subscription_address_with_program_id(&plan, &subscriber, &program_id);

        // Create close_subscription instruction
        let close_sub_accounts = vec![
            AccountMeta::new(subscription_pda, false), // subscription (PDA, mutable, will be closed)
            AccountMeta::new(subscriber, true), // subscriber (signer, mutable, receives rent)
        ];

        let close_sub_args = crate::program_types::CloseSubscriptionArgs {};
        let close_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "close_subscription")
            data.extend_from_slice(&[33, 214, 169, 135, 35, 127, 78, 7]);
            borsh::to_writer(&mut data, &close_sub_args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts: close_sub_accounts,
            data: close_sub_data,
        })
    }
}

impl TransferAuthorityBuilder {
    /// Create a new transfer authority builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current platform authority (must be signer)
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the new authority to transfer to
    #[must_use]
    pub const fn new_authority(mut self, new_authority: Pubkey) -> Self {
        self.new_authority = Some(new_authority);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `transfer_authority` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;
        let new_authority = self.new_authority.ok_or("New authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(platform_authority, true), // platform_authority (signer)
        ];

        let args = crate::program_types::TransferAuthorityArgs { new_authority };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:transfer_authority")
            data.extend_from_slice(&[48, 169, 76, 72, 229, 180, 55, 161]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl AcceptAuthorityBuilder {
    /// Create a new accept authority builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the new authority (must be signer and pending authority)
    #[must_use]
    pub const fn new_authority(mut self, new_authority: Pubkey) -> Self {
        self.new_authority = Some(new_authority);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `accept_authority` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let new_authority = self.new_authority.ok_or("New authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(new_authority, true), // new_authority (signer)
        ];

        let args = crate::program_types::AcceptAuthorityArgs::default();

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:accept_authority")
            data.extend_from_slice(&[107, 86, 198, 91, 33, 12, 107, 160]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl CancelAuthorityTransferBuilder {
    /// Create a new cancel authority transfer builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current platform authority (must be signer)
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `cancel_authority_transfer` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(platform_authority, true), // platform_authority (signer)
        ];

        let args = crate::program_types::CancelAuthorityTransferArgs::default();

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:cancel_authority_transfer")
            data.extend_from_slice(&[94, 131, 125, 184, 183, 24, 125, 229]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl PauseBuilder {
    /// Create a new pause builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the platform authority (must be signer)
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `pause` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(platform_authority, true), // platform_authority (signer)
        ];

        let args = crate::program_types::PauseArgs {};

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:pause")
            data.extend_from_slice(&[211, 22, 221, 251, 74, 121, 193, 47]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl UnpauseBuilder {
    /// Create a new unpause builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the platform authority (must be signer)
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `unpause` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(platform_authority, true), // platform_authority (signer)
        ];

        let args = crate::program_types::UnpauseArgs {};

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:unpause")
            data.extend_from_slice(&[169, 144, 4, 38, 10, 141, 188, 255]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl UpdateConfigBuilder {
    /// Create a new update config builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the platform authority (must be signer)
    #[must_use]
    pub const fn platform_authority(mut self, platform_authority: Pubkey) -> Self {
        self.platform_authority = Some(platform_authority);
        self
    }

    /// Set the keeper fee in basis points (0-100)
    #[must_use]
    pub const fn keeper_fee_bps(mut self, keeper_fee_bps: u16) -> Self {
        self.keeper_fee_bps = Some(keeper_fee_bps);
        self
    }

    /// Set the maximum withdrawal amount
    #[must_use]
    pub const fn max_withdrawal_amount(mut self, max_withdrawal_amount: u64) -> Self {
        self.max_withdrawal_amount = Some(max_withdrawal_amount);
        self
    }

    /// Set the maximum grace period in seconds
    #[must_use]
    pub const fn max_grace_period_seconds(mut self, max_grace_period_seconds: u64) -> Self {
        self.max_grace_period_seconds = Some(max_grace_period_seconds);
        self
    }

    /// Set the minimum platform fee in basis points
    #[must_use]
    pub const fn min_platform_fee_bps(mut self, min_platform_fee_bps: u16) -> Self {
        self.min_platform_fee_bps = Some(min_platform_fee_bps);
        self
    }

    /// Set the maximum platform fee in basis points
    #[must_use]
    pub const fn max_platform_fee_bps(mut self, max_platform_fee_bps: u16) -> Self {
        self.max_platform_fee_bps = Some(max_platform_fee_bps);
        self
    }

    /// Set the minimum period in seconds
    #[must_use]
    pub const fn min_period_seconds(mut self, min_period_seconds: u64) -> Self {
        self.min_period_seconds = Some(min_period_seconds);
        self
    }

    /// Set the default allowance periods
    #[must_use]
    pub const fn default_allowance_periods(mut self, default_allowance_periods: u8) -> Self {
        self.default_allowance_periods = Some(default_allowance_periods);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `update_config` instruction
    /// * `Err(TallyError)` - If building fails
    ///
    /// # Validation
    /// * Platform authority must be set
    /// * At least one field must be set for update
    /// * `keeper_fee_bps` <= 100 if provided
    /// * `min_platform_fee_bps` <= `max_platform_fee_bps` if both provided
    /// * All numeric values > 0 where required
    pub fn build_instruction(self) -> Result<Instruction> {
        let platform_authority = self
            .platform_authority
            .ok_or("Platform authority not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Validate at least one field is set for update
        let has_update = self.keeper_fee_bps.is_some()
            || self.max_withdrawal_amount.is_some()
            || self.max_grace_period_seconds.is_some()
            || self.min_platform_fee_bps.is_some()
            || self.max_platform_fee_bps.is_some()
            || self.min_period_seconds.is_some()
            || self.default_allowance_periods.is_some();

        if !has_update {
            return Err("At least one configuration field must be set for update".into());
        }

        // Validate keeper_fee_bps <= 100 if provided
        if let Some(keeper_fee) = self.keeper_fee_bps {
            if keeper_fee > 100 {
                return Err("Keeper fee must be <= 100 basis points (1%)".into());
            }
        }

        // Validate min_platform_fee_bps <= max_platform_fee_bps if both provided
        if let Some(min_fee) = self.min_platform_fee_bps {
            if let Some(max_fee) = self.max_platform_fee_bps {
                if min_fee > max_fee {
                    return Err("Minimum platform fee must be <= maximum platform fee".into());
                }
            }
        }

        // Validate numeric values > 0 where required
        if let Some(max_withdrawal) = self.max_withdrawal_amount {
            if max_withdrawal == 0 {
                return Err("Maximum withdrawal amount must be > 0".into());
            }
        }

        if let Some(max_grace) = self.max_grace_period_seconds {
            if max_grace == 0 {
                return Err("Maximum grace period must be > 0".into());
            }
        }

        if let Some(min_period) = self.min_period_seconds {
            if min_period == 0 {
                return Err("Minimum period must be > 0".into());
            }
        }

        if let Some(allowance_periods) = self.default_allowance_periods {
            if allowance_periods == 0 {
                return Err("Default allowance periods must be > 0".into());
            }
        }

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA, mutable)
            AccountMeta::new_readonly(platform_authority, true), // platform_authority (signer)
        ];

        let args = UpdateConfigArgs {
            keeper_fee_bps: self.keeper_fee_bps,
            max_withdrawal_amount: self.max_withdrawal_amount,
            max_grace_period_seconds: self.max_grace_period_seconds,
            min_platform_fee_bps: self.min_platform_fee_bps,
            max_platform_fee_bps: self.max_platform_fee_bps,
            min_period_seconds: self.min_period_seconds,
            default_allowance_periods: self.default_allowance_periods,
        };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:update_config")
            data.extend_from_slice(&[29, 158, 252, 191, 10, 83, 219, 99]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

impl UpdateMerchantTierBuilder {
    /// Create a new update merchant tier builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the authority (must be either merchant authority OR platform authority)
    #[must_use]
    pub const fn authority(mut self, authority: Pubkey) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Set the merchant PDA to update
    #[must_use]
    pub const fn merchant(mut self, merchant: Pubkey) -> Self {
        self.merchant = Some(merchant);
        self
    }

    /// Set the new tier (0=Free, 1=Pro, 2=Enterprise)
    #[must_use]
    pub const fn new_tier(mut self, new_tier: u8) -> Self {
        self.new_tier = Some(new_tier);
        self
    }

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `update_merchant_tier` instruction
    /// * `Err(TallyError)` - If building fails
    ///
    /// # Validation
    /// * Authority must be set (merchant or platform authority)
    /// * Merchant must be set
    /// * New tier must be set and valid (0-2)
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let merchant = self.merchant.ok_or("Merchant not set")?;
        let new_tier = self.new_tier.ok_or("New tier not set")?;

        // Validate tier value (0=Free, 1=Pro, 2=Enterprise)
        if new_tier > 2 {
            return Err("New tier must be 0 (Free), 1 (Pro), or 2 (Enterprise)".into());
        }

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false), // config (PDA, readonly)
            AccountMeta::new(merchant, false),            // merchant (PDA, mutable)
            AccountMeta::new_readonly(authority, true),   // authority (signer)
        ];

        let args = crate::program_types::UpdateMerchantTierArgs { new_tier };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:update_merchant_tier")
            data.extend_from_slice(&[24, 54, 190, 70, 221, 93, 3, 64]);
            borsh::to_writer(&mut data, &args)
                .map_err(|e| TallyError::Generic(format!("Failed to serialize args: {e}")))?;
            data
        };

        Ok(Instruction {
            program_id,
            accounts,
            data,
        })
    }
}

// Convenience functions for common transaction building patterns

/// Create a start subscription transaction builder
#[must_use]
pub fn start_subscription() -> StartSubscriptionBuilder {
    StartSubscriptionBuilder::new()
}

/// Create a cancel subscription transaction builder
#[must_use]
pub fn cancel_subscription() -> CancelSubscriptionBuilder {
    CancelSubscriptionBuilder::new()
}

/// Create a merchant initialization transaction builder
#[must_use]
pub fn create_merchant() -> CreateMerchantBuilder {
    CreateMerchantBuilder::new()
}

/// Create a plan creation transaction builder
#[must_use]
pub fn create_plan() -> CreatePlanBuilder {
    CreatePlanBuilder::new()
}

/// Create an admin withdraw fees transaction builder
#[must_use]
pub fn admin_withdraw_fees() -> AdminWithdrawFeesBuilder {
    AdminWithdrawFeesBuilder::new()
}

/// Create a config initialization transaction builder
#[must_use]
pub fn init_config() -> InitConfigBuilder {
    InitConfigBuilder::new()
}

/// Create a plan update transaction builder
#[must_use]
pub fn update_plan() -> UpdatePlanBuilder {
    UpdatePlanBuilder::new()
}

/// Create a renew subscription transaction builder
#[must_use]
pub fn renew_subscription() -> RenewSubscriptionBuilder {
    RenewSubscriptionBuilder::new()
}

/// Create a close subscription transaction builder
#[must_use]
pub fn close_subscription() -> CloseSubscriptionBuilder {
    CloseSubscriptionBuilder::new()
}

/// Create a transfer authority transaction builder
#[must_use]
pub fn transfer_authority() -> TransferAuthorityBuilder {
    TransferAuthorityBuilder::new()
}

/// Create an accept authority transaction builder
#[must_use]
pub fn accept_authority() -> AcceptAuthorityBuilder {
    AcceptAuthorityBuilder::new()
}

/// Create a cancel authority transfer transaction builder
#[must_use]
pub fn cancel_authority_transfer() -> CancelAuthorityTransferBuilder {
    CancelAuthorityTransferBuilder::new()
}

/// Create a pause program transaction builder
#[must_use]
pub fn pause() -> PauseBuilder {
    PauseBuilder::new()
}

/// Create an unpause program transaction builder
#[must_use]
pub fn unpause() -> UnpauseBuilder {
    UnpauseBuilder::new()
}

/// Create an update config transaction builder
#[must_use]
pub fn update_config() -> UpdateConfigBuilder {
    UpdateConfigBuilder::new()
}

/// Create an update merchant tier transaction builder
#[must_use]
pub fn update_merchant_tier() -> UpdateMerchantTierBuilder {
    UpdateMerchantTierBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::{Keypair, Signer};
    use std::str::FromStr;

    fn create_test_merchant() -> Merchant {
        Merchant {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            tier: 0, // Free tier
            bump: 255,
        }
    }

    fn create_test_plan() -> Plan {
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let mut plan_id = [0u8; 32];
        plan_id[..12].copy_from_slice(b"premium_plan");
        let mut name = [0u8; 32];
        name[..12].copy_from_slice(b"Premium Plan");

        Plan {
            merchant,
            plan_id,
            price_usdc: 5_000_000,  // 5 USDC
            period_secs: 2_592_000, // 30 days
            grace_secs: 432_000,    // 5 days
            name,
            active: true,
        }
    }

    #[test]
    fn test_start_subscription_builder() {
        let merchant = create_test_merchant();
        let plan_data = create_test_plan();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instructions = start_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .allowance_periods(3) // 3x plan price
            .build_instructions(&merchant, &plan_data, &platform_treasury_ata)
            .unwrap();

        assert_eq!(instructions.len(), 2);

        // First instruction should be approve_checked
        assert_eq!(instructions[0].program_id, spl_token::id());

        // Second instruction should be start_subscription
        let program_id = program_id();
        assert_eq!(instructions[1].program_id, program_id);
    }

    #[test]
    fn test_cancel_subscription_builder() {
        let merchant = create_test_merchant();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instructions = cancel_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .build_instructions(&merchant)
            .unwrap();

        assert_eq!(instructions.len(), 2);

        // First instruction should be revoke
        assert_eq!(instructions[0].program_id, spl_token::id());

        // Second instruction should be cancel_subscription
        let program_id = program_id();
        assert_eq!(instructions[1].program_id, program_id);
    }

    #[test]
    fn test_create_merchant_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = create_merchant()
            .authority(authority)
            .usdc_mint(usdc_mint)
            .treasury_ata(treasury_ata)
            .platform_fee_bps(50)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 8);
    }

    #[test]
    fn test_create_plan_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan_id_bytes = {
            let mut bytes = [0u8; 32];
            let id_bytes = b"premium";
            let len = id_bytes.len().min(32);
            bytes[..len].copy_from_slice(&id_bytes[..len]);
            bytes
        };

        let plan_args = CreatePlanArgs {
            plan_id: "premium".to_string(),
            plan_id_bytes,
            price_usdc: 5_000_000,
            period_secs: 2_592_000,
            grace_secs: 432_000,
            name: "Premium Plan".to_string(),
        };

        let instruction = create_plan()
            .authority(authority)
            .plan_args(plan_args)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 5);
    }

    #[test]
    fn test_builder_missing_required_fields() {
        let merchant = create_test_merchant();
        let plan_data = create_test_plan();
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing plan
        let result = start_subscription()
            .subscriber(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .allowance_periods(3)
            .build_instructions(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plan not set"));

        // Test missing subscriber
        let result = start_subscription()
            .plan(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .allowance_periods(3)
            .build_instructions(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Subscriber not set"));
    }

    #[test]
    fn test_token_program_variants() {
        // Create separate merchants for different token programs to avoid compatibility issues
        let merchant_token = Merchant {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()), // Use a test mint for classic token
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            tier: 0, // Free tier
            bump: 255,
        };

        let merchant_token2022 = Merchant {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()), // Use a different test mint for Token-2022
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            tier: 0, // Free tier
            bump: 255,
        };

        let plan_data = create_test_plan();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test with Token-2022
        let instructions_token2022 = start_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .allowance_periods(3)
            .token_program(TokenProgram::Token2022)
            .build_instructions(&merchant_token2022, &plan_data, &platform_treasury_ata)
            .unwrap();

        // Test with classic Token
        let instructions_token = start_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .allowance_periods(3)
            .token_program(TokenProgram::Token)
            .build_instructions(&merchant_token, &plan_data, &platform_treasury_ata)
            .unwrap();

        // Both should work but have different token program IDs
        assert_eq!(instructions_token2022[0].program_id, spl_token_2022::id());
        assert_eq!(instructions_token[0].program_id, spl_token::id());
    }

    #[test]
    fn test_init_config_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let config_args = InitConfigArgs {
            platform_authority: authority,
            max_platform_fee_bps: 1000,
            fee_basis_points_divisor: 10000,
            min_period_seconds: 86400,
            default_allowance_periods: 3,
        };

        let instruction = init_config()
            .authority(authority)
            .config_args(config_args)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 3);

        // Verify account metas
        assert!(!instruction.accounts[0].is_signer); // config (PDA)
        assert!(instruction.accounts[0].is_writable); // config (PDA)
        assert!(instruction.accounts[1].is_signer); // authority
        assert!(instruction.accounts[1].is_writable); // authority
        assert!(!instruction.accounts[2].is_signer); // system_program
        assert!(!instruction.accounts[2].is_writable); // system_program
    }

    #[test]
    fn test_init_config_missing_required_fields() {
        // Test missing authority
        let result = init_config()
            .config_args(InitConfigArgs {
                platform_authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
                max_platform_fee_bps: 1000,
                fee_basis_points_divisor: 10000,
                min_period_seconds: 86400,
                default_allowance_periods: 3,
            })
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority not set"));

        // Test missing config args
        let result = init_config()
            .authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Config args not set"));
    }

    #[test]
    fn test_update_plan_builder() {
        let merchant = create_test_merchant();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let update_args = UpdatePlanArgs::new()
            .with_name("Updated Plan".to_string())
            .with_active(false)
            .with_price_usdc(10_000_000); // 10 USDC

        let instruction = update_plan()
            .authority(merchant.authority)
            .plan_key(plan_key)
            .update_args(update_args)
            .build_instruction(&merchant)
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 4);

        // Verify account structure
        assert!(!instruction.accounts[0].is_signer); // config (readonly)
        assert!(!instruction.accounts[0].is_writable); // config (readonly)
        assert!(!instruction.accounts[1].is_signer); // plan (mutable, but not signer)
        assert!(instruction.accounts[1].is_writable); // plan (mutable)
        assert!(!instruction.accounts[2].is_signer); // merchant (readonly)
        assert!(!instruction.accounts[2].is_writable); // merchant (readonly)
        assert!(instruction.accounts[3].is_signer); // authority (signer)
        assert!(instruction.accounts[3].is_writable); // authority (mutable for fees)
    }

    #[test]
    fn test_update_plan_builder_missing_required_fields() {
        let merchant = create_test_merchant();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let update_args = UpdatePlanArgs::new().with_name("Test".to_string());

        // Test missing authority
        let result = update_plan()
            .plan_key(plan_key)
            .update_args(update_args.clone())
            .build_instruction(&merchant);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority not set"));

        // Test missing plan key
        let result = update_plan()
            .authority(merchant.authority)
            .update_args(update_args)
            .build_instruction(&merchant);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plan key not set"));

        // Test missing update args
        let result = update_plan()
            .authority(merchant.authority)
            .plan_key(plan_key)
            .build_instruction(&merchant);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Update args not set"));
    }

    #[test]
    fn test_update_plan_builder_validation() {
        let merchant = create_test_merchant();
        let wrong_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test wrong authority
        let update_args = UpdatePlanArgs::new().with_name("Test".to_string());
        let result = update_plan()
            .authority(wrong_authority)
            .plan_key(plan_key)
            .update_args(update_args)
            .build_instruction(&merchant);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority does not match merchant authority"));

        // Test empty update args
        let empty_args = UpdatePlanArgs::new();
        let result = update_plan()
            .authority(merchant.authority)
            .plan_key(plan_key)
            .update_args(empty_args)
            .build_instruction(&merchant);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No updates specified"));
    }

    #[test]
    fn test_update_plan_args_functionality() {
        // Test UpdatePlanArgs builder pattern
        let args = UpdatePlanArgs::new()
            .with_name("New Plan Name".to_string())
            .with_active(true)
            .with_price_usdc(5_000_000)
            .with_period_secs(2_592_000)
            .with_grace_secs(432_000);

        assert!(args.has_updates());
        assert_eq!(args.name, Some("New Plan Name".to_string()));
        assert_eq!(args.active, Some(true));
        assert_eq!(args.price_usdc, Some(5_000_000));
        assert_eq!(args.period_secs, Some(2_592_000));
        assert_eq!(args.grace_secs, Some(432_000));

        // Test name_bytes conversion
        let name_bytes = args.name_bytes().unwrap();
        let expected_name = "New Plan Name";
        assert_eq!(&name_bytes[..expected_name.len()], expected_name.as_bytes());
        // Check that the rest of the array is zero-padded
        for &byte in &name_bytes[expected_name.len()..] {
            assert_eq!(byte, 0);
        }

        // Test empty args
        let empty_args = UpdatePlanArgs::new();
        assert!(!empty_args.has_updates());
        assert!(empty_args.name_bytes().is_none());

        // Test default
        let default_args = UpdatePlanArgs::default();
        assert!(!default_args.has_updates());
    }

    #[test]
    fn test_renew_subscription_builder() {
        let merchant = create_test_merchant();
        let plan_data = create_test_plan();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = renew_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .build_instruction(&merchant, &plan_data, &platform_treasury_ata)
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 12);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[45, 75, 154, 194, 160, 10, 111, 183]
        );

        // Verify readonly accounts
        verify_renew_readonly_accounts(&instruction);
        // Verify mutable accounts
        verify_renew_mutable_accounts(&instruction);
        // Verify signer accounts
        verify_renew_signer_accounts(&instruction);
    }

    fn verify_renew_readonly_accounts(instruction: &Instruction) {
        assert!(!instruction.accounts[0].is_writable); // config
        assert!(!instruction.accounts[2].is_writable); // plan
        assert!(!instruction.accounts[3].is_writable); // merchant
        assert!(!instruction.accounts[9].is_writable); // usdc_mint
        assert!(!instruction.accounts[10].is_writable); // program_delegate
        assert!(!instruction.accounts[11].is_writable); // token_program
    }

    fn verify_renew_mutable_accounts(instruction: &Instruction) {
        assert!(instruction.accounts[1].is_writable); // subscription
        assert!(instruction.accounts[4].is_writable); // subscriber_usdc_ata
        assert!(instruction.accounts[5].is_writable); // merchant_treasury_ata
        assert!(instruction.accounts[6].is_writable); // platform_treasury_ata
        assert!(instruction.accounts[7].is_writable); // keeper
        assert!(instruction.accounts[8].is_writable); // keeper_usdc_ata
    }

    fn verify_renew_signer_accounts(instruction: &Instruction) {
        assert!(!instruction.accounts[0].is_signer); // config
        assert!(!instruction.accounts[1].is_signer); // subscription
        assert!(!instruction.accounts[2].is_signer); // plan
        assert!(!instruction.accounts[3].is_signer); // merchant
        assert!(!instruction.accounts[4].is_signer); // subscriber_usdc_ata
        assert!(!instruction.accounts[5].is_signer); // merchant_treasury_ata
        assert!(!instruction.accounts[6].is_signer); // platform_treasury_ata
        assert!(instruction.accounts[7].is_signer); // keeper (only signer)
        assert!(!instruction.accounts[8].is_signer); // keeper_usdc_ata
        assert!(!instruction.accounts[9].is_signer); // usdc_mint
        assert!(!instruction.accounts[10].is_signer); // program_delegate
        assert!(!instruction.accounts[11].is_signer); // token_program
    }

    #[test]
    fn test_renew_subscription_builder_missing_required_fields() {
        let merchant = create_test_merchant();
        let plan_data = create_test_plan();
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing plan
        let result = renew_subscription()
            .subscriber(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plan not set"));

        // Test missing subscriber
        let result = renew_subscription()
            .plan(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Subscriber not set"));

        // Test missing keeper
        let result = renew_subscription()
            .plan(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .subscriber(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Keeper not set"));

        // Test missing keeper_ata
        let result = renew_subscription()
            .plan(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .subscriber(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Keeper ATA not set"));
    }

    #[test]
    fn test_renew_subscription_token_program_variants() {
        // Create separate merchants for different token programs
        let merchant_token = Merchant {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            tier: 0,
            bump: 255,
        };

        let merchant_token2022 = Merchant {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            tier: 0,
            bump: 255,
        };

        let plan_data = create_test_plan();
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test with Token-2022
        let instruction_token2022 = renew_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .token_program(TokenProgram::Token2022)
            .build_instruction(&merchant_token2022, &plan_data, &platform_treasury_ata)
            .unwrap();

        // Test with classic Token
        let instruction_token = renew_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .token_program(TokenProgram::Token)
            .build_instruction(&merchant_token, &plan_data, &platform_treasury_ata)
            .unwrap();

        // Both should work but have different token program IDs in the accounts
        assert_eq!(
            instruction_token2022.accounts[11].pubkey,
            spl_token_2022::id()
        );
        assert_eq!(instruction_token.accounts[11].pubkey, spl_token::id());
    }

    #[test]
    fn test_close_subscription_builder() {
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(&instruction.data[..8], &[33, 214, 169, 135, 35, 127, 78, 7]);

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // subscription (mutable)
        assert!(!instruction.accounts[0].is_signer); // subscription (not signer, it's a PDA)
        assert!(instruction.accounts[1].is_writable); // subscriber (mutable, receives rent)
        assert!(instruction.accounts[1].is_signer); // subscriber (signer)
    }

    #[test]
    fn test_close_subscription_builder_missing_required_fields() {
        // Test missing plan
        let result = close_subscription()
            .subscriber(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plan not set"));

        // Test missing subscriber
        let result = close_subscription()
            .plan(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Subscriber not set"));
    }

    #[test]
    fn test_close_subscription_builder_custom_program_id() {
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_close_subscription_builder_pda_computation() {
        let plan_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_subscription()
            .plan(plan_key)
            .subscriber(subscriber)
            .build_instruction()
            .unwrap();

        // Verify the computed subscription PDA is correct
        let program_id = program_id();
        let expected_subscription_pda =
            pda::subscription_address_with_program_id(&plan_key, &subscriber, &program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_subscription_pda);
        assert_eq!(instruction.accounts[1].pubkey, subscriber);
    }

    #[test]
    fn test_transfer_authority_builder() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = transfer_authority()
            .platform_authority(platform_authority)
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[48, 169, 76, 72, 229, 180, 55, 161]
        );

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // config (mutable)
        assert!(!instruction.accounts[0].is_signer); // config (not signer, it's a PDA)
        assert!(!instruction.accounts[1].is_writable); // platform_authority (readonly)
        assert!(instruction.accounts[1].is_signer); // platform_authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_transfer_authority_builder_missing_required_fields() {
        // Test missing platform_authority
        let result = transfer_authority()
            .new_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));

        // Test missing new_authority
        let result = transfer_authority()
            .platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New authority not set"));
    }

    #[test]
    fn test_transfer_authority_builder_custom_program_id() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = transfer_authority()
            .platform_authority(platform_authority)
            .new_authority(new_authority)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_transfer_authority_builder_pda_computation() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = transfer_authority()
            .platform_authority(platform_authority)
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_transfer_authority_args_serialization() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = transfer_authority()
            .platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        assert!(instruction.data.len() > 8);

        // Verify we can deserialize the args from the instruction data
        let args_data = &instruction.data[8..];
        let deserialized_args =
            crate::program_types::TransferAuthorityArgs::try_from_slice(args_data).unwrap();
        assert_eq!(deserialized_args.new_authority, new_authority);
    }

    #[test]
    fn test_transfer_authority_builder_clone_debug() {
        let builder = transfer_authority()
            .platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .new_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(
            cloned_builder.platform_authority,
            builder.platform_authority
        );
        assert_eq!(cloned_builder.new_authority, builder.new_authority);

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("TransferAuthorityBuilder"));
    }

    #[test]
    fn test_accept_authority_builder() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = accept_authority()
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[107, 86, 198, 91, 33, 12, 107, 160]
        );

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // config (mutable)
        assert!(!instruction.accounts[0].is_signer); // config (not signer, it's a PDA)
        assert!(!instruction.accounts[1].is_writable); // new_authority (readonly)
        assert!(instruction.accounts[1].is_signer); // new_authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, new_authority);
    }

    #[test]
    fn test_accept_authority_builder_missing_required_fields() {
        // Test missing new_authority
        let result = accept_authority().build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New authority not set"));
    }

    #[test]
    fn test_accept_authority_builder_custom_program_id() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = accept_authority()
            .new_authority(new_authority)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_accept_authority_builder_pda_computation() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = accept_authority()
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, new_authority);
    }

    #[test]
    fn test_accept_authority_args_serialization() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = accept_authority()
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // AcceptAuthorityArgs is empty, so data should be exactly 8 bytes (discriminator only)
        assert_eq!(instruction.data.len(), 8);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[107, 86, 198, 91, 33, 12, 107, 160]
        );
    }

    #[test]
    fn test_accept_authority_builder_clone_debug() {
        let builder =
            accept_authority().new_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(cloned_builder.new_authority, builder.new_authority);

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("AcceptAuthorityBuilder"));
    }

    #[test]
    fn test_accept_authority_builder_default() {
        let builder = AcceptAuthorityBuilder::default();
        assert!(builder.new_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_accept_authority_convenience_function() {
        let new_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = accept_authority()
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = AcceptAuthorityBuilder::new()
            .new_authority(new_authority)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }

    #[test]
    fn test_cancel_authority_transfer_builder() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = cancel_authority_transfer()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[94, 131, 125, 184, 183, 24, 125, 229]
        );

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // config (mutable)
        assert!(!instruction.accounts[0].is_signer); // config (not signer, it's a PDA)
        assert!(!instruction.accounts[1].is_writable); // platform_authority (readonly)
        assert!(instruction.accounts[1].is_signer); // platform_authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_cancel_authority_transfer_builder_missing_required_fields() {
        // Test missing platform_authority
        let result = cancel_authority_transfer().build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));
    }

    #[test]
    fn test_cancel_authority_transfer_builder_custom_program_id() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = cancel_authority_transfer()
            .platform_authority(platform_authority)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_cancel_authority_transfer_builder_pda_computation() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = cancel_authority_transfer()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_cancel_authority_transfer_args_serialization() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = cancel_authority_transfer()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // CancelAuthorityTransferArgs is empty, so data should be exactly 8 bytes (discriminator only)
        assert_eq!(instruction.data.len(), 8);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[94, 131, 125, 184, 183, 24, 125, 229]
        );
    }

    #[test]
    fn test_cancel_authority_transfer_builder_clone_debug() {
        let builder = cancel_authority_transfer()
            .platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(
            cloned_builder.platform_authority,
            builder.platform_authority
        );

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("CancelAuthorityTransferBuilder"));
    }

    #[test]
    fn test_cancel_authority_transfer_builder_default() {
        let builder = CancelAuthorityTransferBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_cancel_authority_transfer_convenience_function() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = cancel_authority_transfer()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = CancelAuthorityTransferBuilder::new()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }

    #[test]
    fn test_pause_builder() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = pause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[211, 22, 221, 251, 74, 121, 193, 47]
        );

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // config (mutable)
        assert!(!instruction.accounts[0].is_signer); // config (not signer, it's a PDA)
        assert!(!instruction.accounts[1].is_writable); // platform_authority (readonly)
        assert!(instruction.accounts[1].is_signer); // platform_authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_pause_builder_missing_required_fields() {
        // Test missing platform_authority
        let result = pause().build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));
    }

    #[test]
    fn test_pause_builder_custom_program_id() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = pause()
            .platform_authority(platform_authority)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_pause_builder_pda_computation() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = pause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_pause_args_serialization() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = pause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // PauseArgs is empty, so data should be exactly 8 bytes (discriminator only)
        assert_eq!(instruction.data.len(), 8);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[211, 22, 221, 251, 74, 121, 193, 47]
        );
    }

    #[test]
    fn test_pause_builder_clone_debug() {
        let builder = pause().platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(
            cloned_builder.platform_authority,
            builder.platform_authority
        );

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("PauseBuilder"));
    }

    #[test]
    fn test_pause_builder_default() {
        let builder = PauseBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_pause_convenience_function() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = pause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = PauseBuilder::new()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }

    #[test]
    fn test_unpause_builder() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = unpause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[169, 144, 4, 38, 10, 141, 188, 255]
        );

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // config (mutable)
        assert!(!instruction.accounts[0].is_signer); // config (not signer, it's a PDA)
        assert!(!instruction.accounts[1].is_writable); // platform_authority (readonly)
        assert!(instruction.accounts[1].is_signer); // platform_authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_unpause_builder_missing_required_fields() {
        // Test missing platform_authority
        let result = unpause().build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));
    }

    #[test]
    fn test_unpause_builder_custom_program_id() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = unpause()
            .platform_authority(platform_authority)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_unpause_builder_pda_computation() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = unpause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_unpause_args_serialization() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = unpause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // UnpauseArgs is empty, so data should be exactly 8 bytes (discriminator only)
        assert_eq!(instruction.data.len(), 8);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[169, 144, 4, 38, 10, 141, 188, 255]
        );
    }

    #[test]
    fn test_unpause_builder_clone_debug() {
        let builder =
            unpause().platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(
            cloned_builder.platform_authority,
            builder.platform_authority
        );

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("UnpauseBuilder"));
    }

    #[test]
    fn test_unpause_builder_default() {
        let builder = UnpauseBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_unpause_convenience_function() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = unpause()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = UnpauseBuilder::new()
            .platform_authority(platform_authority)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }

    // UpdateConfigBuilder tests

    #[test]
    fn test_update_config_builder_basic() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 2);

        // Verify config PDA is first account (mutable)
        let expected_config_pda = pda::config_address_with_program_id(&program_id());
        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert!(instruction.accounts[0].is_writable);
        assert!(!instruction.accounts[0].is_signer);

        // Verify platform authority is second account (signer, read-only)
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
        assert!(!instruction.accounts[1].is_writable);
        assert!(instruction.accounts[1].is_signer);
    }

    #[test]
    fn test_update_config_builder_all_fields() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(50)
            .max_withdrawal_amount(1_000_000_000)
            .max_grace_period_seconds(604_800)
            .min_platform_fee_bps(50)
            .max_platform_fee_bps(1000)
            .min_period_seconds(86_400)
            .default_allowance_periods(5)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 2);
    }

    #[test]
    fn test_update_config_builder_missing_authority() {
        let result = update_config().keeper_fee_bps(25).build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));
    }

    #[test]
    fn test_update_config_builder_no_updates() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one configuration field must be set"));
    }

    #[test]
    fn test_update_config_builder_keeper_fee_too_high() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(101)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Keeper fee must be <= 100"));
    }

    #[test]
    fn test_update_config_builder_min_fee_greater_than_max() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .min_platform_fee_bps(200)
            .max_platform_fee_bps(100)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Minimum platform fee must be <= maximum platform fee"));
    }

    #[test]
    fn test_update_config_builder_zero_max_withdrawal() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .max_withdrawal_amount(0)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Maximum withdrawal amount must be > 0"));
    }

    #[test]
    fn test_update_config_builder_zero_max_grace_period() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .max_grace_period_seconds(0)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Maximum grace period must be > 0"));
    }

    #[test]
    fn test_update_config_builder_zero_min_period() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .min_period_seconds(0)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Minimum period must be > 0"));
    }

    #[test]
    fn test_update_config_builder_zero_allowance_periods() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_config()
            .platform_authority(platform_authority)
            .default_allowance_periods(0)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Default allowance periods must be > 0"));
    }

    #[test]
    fn test_update_config_builder_custom_program_id() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_update_config_builder_pda_computation() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, platform_authority);
    }

    #[test]
    fn test_update_config_args_serialization() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .max_withdrawal_amount(1_000_000)
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        assert!(instruction.data.len() > 8);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[29, 158, 252, 191, 10, 83, 219, 99]
        );
    }

    #[test]
    fn test_update_config_builder_clone_debug() {
        let builder = update_config()
            .platform_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_fee_bps(25);

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(
            cloned_builder.platform_authority,
            builder.platform_authority
        );
        assert_eq!(cloned_builder.keeper_fee_bps, builder.keeper_fee_bps);

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("UpdateConfigBuilder"));
    }

    #[test]
    fn test_update_config_builder_default() {
        let builder = UpdateConfigBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.keeper_fee_bps.is_none());
        assert!(builder.max_withdrawal_amount.is_none());
        assert!(builder.max_grace_period_seconds.is_none());
        assert!(builder.min_platform_fee_bps.is_none());
        assert!(builder.max_platform_fee_bps.is_none());
        assert!(builder.min_period_seconds.is_none());
        assert!(builder.default_allowance_periods.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_update_config_convenience_function() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = UpdateConfigBuilder::new()
            .platform_authority(platform_authority)
            .keeper_fee_bps(25)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }

    #[test]
    fn test_update_config_builder_partial_updates() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test updating only keeper fee
        let instruction1 = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(30)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction1.accounts.len(), 2);

        // Test updating only max withdrawal amount
        let instruction2 = update_config()
            .platform_authority(platform_authority)
            .max_withdrawal_amount(5_000_000)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction2.accounts.len(), 2);

        // Test updating only fee bounds
        let instruction3 = update_config()
            .platform_authority(platform_authority)
            .min_platform_fee_bps(100)
            .max_platform_fee_bps(500)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction3.accounts.len(), 2);
    }

    #[test]
    fn test_update_config_builder_edge_cases() {
        let platform_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test max keeper fee (100 bps = 1%)
        let instruction1 = update_config()
            .platform_authority(platform_authority)
            .keeper_fee_bps(100)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction1.accounts.len(), 2);

        // Test min/max fee equal
        let instruction2 = update_config()
            .platform_authority(platform_authority)
            .min_platform_fee_bps(100)
            .max_platform_fee_bps(100)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction2.accounts.len(), 2);

        // Test minimum valid values
        let instruction3 = update_config()
            .platform_authority(platform_authority)
            .max_withdrawal_amount(1)
            .max_grace_period_seconds(1)
            .min_period_seconds(1)
            .default_allowance_periods(1)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction3.accounts.len(), 2);
    }

    #[test]
    fn test_update_merchant_tier_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1) // Pro tier
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 3);

        // Verify instruction discriminator matches program
        assert_eq!(
            &instruction.data[..8],
            &[24, 54, 190, 70, 221, 93, 3, 64]
        );

        // Verify account structure
        assert!(!instruction.accounts[0].is_writable); // config (readonly)
        assert!(!instruction.accounts[0].is_signer); // config (PDA, not signer)
        assert!(instruction.accounts[1].is_writable); // merchant (mutable)
        assert!(!instruction.accounts[1].is_signer); // merchant (PDA, not signer)
        assert!(!instruction.accounts[2].is_writable); // authority (readonly)
        assert!(instruction.accounts[2].is_signer); // authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, merchant);
        assert_eq!(instruction.accounts[2].pubkey, authority);
    }

    #[test]
    fn test_update_merchant_tier_builder_all_tiers() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test Free tier (0)
        let instruction_free = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(0)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_free.accounts.len(), 3);

        // Test Pro tier (1)
        let instruction_pro = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_pro.accounts.len(), 3);

        // Test Enterprise tier (2)
        let instruction_enterprise = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(2)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_enterprise.accounts.len(), 3);
    }

    #[test]
    fn test_update_merchant_tier_builder_missing_required_fields() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing authority
        let result = update_merchant_tier()
            .merchant(merchant)
            .new_tier(1)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority not set"));

        // Test missing merchant
        let result = update_merchant_tier()
            .authority(authority)
            .new_tier(1)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Merchant not set"));

        // Test missing new_tier
        let result = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New tier not set"));
    }

    #[test]
    fn test_update_merchant_tier_builder_invalid_tier() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test tier > 2 (invalid)
        let result = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(3)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New tier must be 0 (Free), 1 (Pro), or 2 (Enterprise)"));

        // Test tier 255 (invalid)
        let result = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(255)
            .build_instruction();
        assert!(result.is_err());
    }

    #[test]
    fn test_update_merchant_tier_builder_custom_program_id() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    fn test_update_merchant_tier_builder_pda_computation() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, merchant);
        assert_eq!(instruction.accounts[2].pubkey, authority);
    }

    #[test]
    fn test_update_merchant_tier_args_serialization() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1) // Pro tier
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // UpdateMerchantTierArgs has 1 u8 field, so data should be 9 bytes
        assert_eq!(instruction.data.len(), 9);

        // Verify the discriminator matches
        assert_eq!(
            &instruction.data[..8],
            &[24, 54, 190, 70, 221, 93, 3, 64]
        );

        // Verify the tier value is serialized correctly
        assert_eq!(instruction.data[8], 1); // Pro tier
    }

    #[test]
    fn test_update_merchant_tier_builder_clone_debug() {
        let builder = update_merchant_tier()
            .authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .merchant(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .new_tier(1);

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(cloned_builder.authority, builder.authority);
        assert_eq!(cloned_builder.merchant, builder.merchant);
        assert_eq!(cloned_builder.new_tier, builder.new_tier);

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("UpdateMerchantTierBuilder"));
    }

    #[test]
    fn test_update_merchant_tier_builder_default() {
        let builder = UpdateMerchantTierBuilder::default();
        assert!(builder.authority.is_none());
        assert!(builder.merchant.is_none());
        assert!(builder.new_tier.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    fn test_update_merchant_tier_convenience_function() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = update_merchant_tier()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = UpdateMerchantTierBuilder::new()
            .authority(authority)
            .merchant(merchant)
            .new_tier(1)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, direct_instruction.program_id);
        assert_eq!(
            instruction.accounts.len(),
            direct_instruction.accounts.len()
        );
        assert_eq!(instruction.data, direct_instruction.data);
    }
}
