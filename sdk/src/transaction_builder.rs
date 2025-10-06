//! Transaction building utilities for Tally subscription flows

use crate::{
    ata::{get_associated_token_address_with_program, TokenProgram},
    error::{Result, TallyError},
    pda, program_id,
    program_types::{
        AdminWithdrawFeesArgs, CancelSubscriptionArgs, CreatePlanArgs, InitConfigArgs,
        InitMerchantArgs, Merchant, Plan, StartSubscriptionArgs, UpdatePlanArgs,
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
            AccountMeta::new(config_pda, false),           // config (PDA, mutable)
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
            AccountMeta::new(config_pda, false),           // config (PDA, mutable)
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
        let builder = accept_authority()
            .new_authority(Pubkey::from(Keypair::new().pubkey().to_bytes()));

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
        assert_eq!(instruction.accounts.len(), direct_instruction.accounts.len());
        assert_eq!(instruction.data, direct_instruction.data);
    }
}
