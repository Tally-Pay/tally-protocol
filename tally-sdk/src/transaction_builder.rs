//! Transaction building utilities for Tally subscription flows

use crate::{
    ata::{get_associated_token_address_with_program, TokenProgram},
    error::{Result, TallyError},
    pda,
    program_types::{
        AdminWithdrawFeesArgs, CancelSubscriptionArgs, CreatePlanArgs, InitConfigArgs,
        InitMerchantArgs, Merchant, Plan, StartSubscriptionArgs,
    },
    program_id,
};
use anchor_lang::prelude::*;
#[allow(deprecated)]
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
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
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RenewSubscriptionBuilder {
    plan: Option<Pubkey>,
    subscriber: Option<Pubkey>,
    payer: Option<Pubkey>,
    expected_renewal_ts: Option<i64>,
    token_program: Option<TokenProgram>,
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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

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
            AccountMeta::new_readonly(system_program::id(), false), // system_program
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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

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
            AccountMeta::new_readonly(system_program::id(), false),               // system_program
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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

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
            AccountMeta::new_readonly(system_program::id(), false), // system_program
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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

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

        let program_id = self.program_id.unwrap_or_else(|| {
            program_id()
        });

        // Compute config PDA
        let config_pda = pda::config_address_with_program_id(&program_id);

        let accounts = vec![
            AccountMeta::new(config_pda, false), // config (PDA)
            AccountMeta::new(authority, true),   // authority (signer)
            AccountMeta::new_readonly(system_program::id(), false), // system_program
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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{Keypair, Signer};
    use std::str::FromStr;

    fn create_test_merchant() -> Merchant {
        Merchant {
            authority: Keypair::new().pubkey(),
            usdc_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            treasury_ata: Keypair::new().pubkey(),
            platform_fee_bps: 50,
            bump: 255,
        }
    }

    fn create_test_plan() -> Plan {
        let merchant = Keypair::new().pubkey();
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
        let plan_key = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();
        let platform_treasury_ata = Keypair::new().pubkey();

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
        let plan_key = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

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
        let authority = Keypair::new().pubkey();
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let treasury_ata = Keypair::new().pubkey();

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
        let authority = Keypair::new().pubkey();
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
        let platform_treasury_ata = Keypair::new().pubkey();

        // Test missing plan
        let result = start_subscription()
            .subscriber(Keypair::new().pubkey())
            .allowance_periods(3)
            .build_instructions(&merchant, &plan_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Plan not set"));

        // Test missing subscriber
        let result = start_subscription()
            .plan(Keypair::new().pubkey())
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
            authority: Keypair::new().pubkey(),
            usdc_mint: Keypair::new().pubkey(), // Use a test mint for classic token
            treasury_ata: Keypair::new().pubkey(),
            platform_fee_bps: 50,
            bump: 255,
        };

        let merchant_token2022 = Merchant {
            authority: Keypair::new().pubkey(),
            usdc_mint: Keypair::new().pubkey(), // Use a different test mint for Token-2022
            treasury_ata: Keypair::new().pubkey(),
            platform_fee_bps: 50,
            bump: 255,
        };

        let plan_data = create_test_plan();
        let plan_key = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();
        let platform_treasury_ata = Keypair::new().pubkey();

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
        let authority = Keypair::new().pubkey();
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
                platform_authority: Keypair::new().pubkey(),
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
            .authority(Keypair::new().pubkey())
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Config args not set"));
    }
}
