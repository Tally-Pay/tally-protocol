//! Transaction building utilities for Tally payment agreement flows

use crate::{
    ata::{get_associated_token_address_with_program, TokenProgram},
    error::{Result, TallyError},
    pda, program_id,
    program_types::{
        PauseAgreementArgs, CreatePaymentTermsArgs,
        StartAgreementArgs, Payee, PaymentTerms, InitPayeeArgs,
    },
};

#[cfg(feature = "platform-admin")]
use crate::program_types::{AdminWithdrawFeesArgs, InitConfigArgs, UpdateConfigArgs};
use anchor_client::solana_sdk::instruction::{AccountMeta, Instruction};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use spl_token::instruction::{approve_checked as approve_checked_token, revoke as revoke_token};
use spl_token_2022::instruction::{
    approve_checked as approve_checked_token2022, revoke as revoke_token2022,
};

/// Builder for start agreement transactions (approve → start flow)
#[derive(Clone, Debug, Default)]
pub struct StartAgreementBuilder {
    payment_terms: Option<Pubkey>,
    payer: Option<Pubkey>,
    allowance_periods: Option<u8>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for pause agreement transactions (revoke → cancel flow)
#[derive(Clone, Debug, Default)]
pub struct PauseAgreementBuilder {
    payment_terms: Option<Pubkey>,
    payer: Option<Pubkey>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for init payee transactions
#[derive(Clone, Debug, Default)]
pub struct InitPayeeBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    usdc_mint: Option<Pubkey>,
    treasury_ata: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for create payment terms transactions
#[derive(Clone, Debug, Default)]
pub struct CreatePaymentTermsBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    payment_terms_args: Option<CreatePaymentTermsArgs>,
    program_id: Option<Pubkey>,
}


/// Builder for admin fee withdrawal transactions
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
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
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct InitConfigBuilder {
    authority: Option<Pubkey>,
    payer: Option<Pubkey>,
    #[cfg(feature = "platform-admin")]
    config_args: Option<InitConfigArgs>,
    #[cfg(not(feature = "platform-admin"))]
    config_args: Option<()>,
    program_id: Option<Pubkey>,
}

/// Builder for execute payment transactions
#[derive(Clone, Debug, Default)]
pub struct ExecutePaymentBuilder {
    payment_terms: Option<Pubkey>,
    payer: Option<Pubkey>,
    keeper: Option<Pubkey>,
    keeper_ata: Option<Pubkey>,
    token_program: Option<TokenProgram>,
    program_id: Option<Pubkey>,
}

/// Builder for close agreement transactions
#[derive(Clone, Debug, Default)]
pub struct CloseAgreementBuilder {
    payment_terms: Option<Pubkey>,
    payer: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for transfer authority transactions
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct TransferAuthorityBuilder {
    platform_authority: Option<Pubkey>,
    new_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for accept authority transactions
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct AcceptAuthorityBuilder {
    new_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for cancel authority transfer transactions
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct CancelAuthorityTransferBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for pause program transactions
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
pub struct PauseBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for unpause program transactions
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
pub struct UnpauseBuilder {
    platform_authority: Option<Pubkey>,
    program_id: Option<Pubkey>,
}

/// Builder for update config transactions
#[cfg_attr(not(feature = "platform-admin"), allow(dead_code))]
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


impl StartAgreementBuilder {
    /// Create a new start agreement builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `payment_terms` PDA
    #[must_use]
    pub const fn payment_terms(mut self, payment_terms: Pubkey) -> Self {
        self.payment_terms = Some(payment_terms);
        self
    }

    /// Set the payer pubkey (also sets as transaction payer)
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
    /// * `payee` - The payee account data
    /// * `payment_terms_data` - The `payment_terms` account data
    /// * `platform_treasury_ata` - Platform treasury ATA address
    ///
    /// # Returns
    /// * `Ok(Vec<Instruction>)` - The transaction instructions (`approve_checked` + `start_payment_agreement`)
    /// * `Err(TallyError)` - If building fails
    #[allow(clippy::similar_names)] // payer and payee are distinct payment domain terms
    pub fn build_instructions(
        self,
        payee: &Payee,
        payment_terms_data: &PaymentTerms,
        platform_treasury_ata: &Pubkey,
    ) -> Result<Vec<Instruction>> {
        let payment_terms = self.payment_terms.ok_or("PaymentTerms not set")?;
        let payer = self.payer.ok_or("Payer not set")?;
        let allowance_periods = self.allowance_periods.unwrap_or(3);
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let payee_pda = pda::payee_address_with_program_id(&payee.authority, &program_id);
        let payment_agreement_pda =
            pda::payment_agreement_address_with_program_id(&payment_terms, &payer, &program_id);
        let delegate_pda = pda::delegate_address_with_program_id(&program_id);
        let payer_ata = get_associated_token_address_with_program(
            &payer,
            &payee.usdc_mint,
            token_program,
        )?;

        // Calculate allowance amount based on payment_terms price and periods
        let allowance_amount = payment_terms_data
            .amount_usdc
            .checked_mul(u64::from(allowance_periods))
            .ok_or_else(|| TallyError::Generic("Arithmetic overflow".to_string()))?;

        // Create approve_checked instruction using the correct token program
        let approve_ix = match token_program {
            TokenProgram::Token => approve_checked_token(
                &token_program.program_id(),
                &payer_ata,
                &payee.usdc_mint,
                &delegate_pda, // Program delegate PDA
                &payer,        // Payer as owner
                &[],           // No additional signers
                allowance_amount,
                6, // USDC decimals
            )?,
            TokenProgram::Token2022 => approve_checked_token2022(
                &token_program.program_id(),
                &payer_ata,
                &payee.usdc_mint,
                &delegate_pda, // Program delegate PDA
                &payer,        // Payer as owner
                &[],           // No additional signers
                allowance_amount,
                6, // USDC decimals
            )?,
        };

        // Create start_payment_agreement instruction
        let start_sub_accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(payment_agreement_pda, false),      // payment agreement (PDA)
            AccountMeta::new_readonly(payment_terms, false),         // payment_terms
            AccountMeta::new_readonly(payee_pda, false), // payee
            AccountMeta::new(payer, true),                  // payer (signer)
            AccountMeta::new(payer_ata, false),        // payer_usdc_ata
            AccountMeta::new(payee.treasury_ata, false), // payee_treasury_ata
            AccountMeta::new(*platform_treasury_ata, false), // platform_treasury_ata
            AccountMeta::new_readonly(payee.usdc_mint, false), // usdc_mint
            AccountMeta::new_readonly(delegate_pda, false), // program_delegate
            AccountMeta::new_readonly(token_program.program_id(), false), // token_program
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ];

        let start_sub_args = StartAgreementArgs {
            allowance_periods,
        };
        let start_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "start_agreement")
            data.extend_from_slice(&[174, 25, 237, 147, 127, 156, 238, 34]);
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

impl PauseAgreementBuilder {
    /// Create a new pause agreement builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `payment_terms` PDA
    #[must_use]
    pub const fn payment_terms(mut self, payment_terms: Pubkey) -> Self {
        self.payment_terms = Some(payment_terms);
        self
    }

    /// Set the payer pubkey (also sets as transaction payer)
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
    /// * `payee` - The payee account data
    ///
    /// # Returns
    /// * `Ok(Vec<Instruction>)` - The transaction instructions (revoke + `cancel_payment_agreement`)
    /// * `Err(TallyError)` - If building fails
    #[allow(clippy::similar_names)] // payer and payee are distinct payment domain terms
    pub fn build_instructions(self, payee: &Payee) -> Result<Vec<Instruction>> {
        let payment_terms = self.payment_terms.ok_or("PaymentTerms not set")?;
        let payer = self.payer.ok_or("Payer not set")?;
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let payment_agreement_pda =
            pda::payment_agreement_address_with_program_id(&payment_terms, &payer, &program_id);
        let payer_ata = get_associated_token_address_with_program(
            &payer,
            &payee.usdc_mint,
            token_program,
        )?;

        // Create revoke instruction using the correct token program
        let revoke_ix = match token_program {
            TokenProgram::Token => revoke_token(
                &token_program.program_id(),
                &payer_ata,
                &payer,      // Payer as owner
                &[],         // No additional signers
            )?,
            TokenProgram::Token2022 => revoke_token2022(
                &token_program.program_id(),
                &payer_ata,
                &payer,      // Payer as owner
                &[],         // No additional signers
            )?,
        };

        // Create cancel_payment_agreement instruction
        let payee_pda = pda::payee_address_with_program_id(&payee.authority, &program_id);
        let cancel_sub_accounts = vec![
            AccountMeta::new(payment_agreement_pda, false), // payment agreement (PDA)
            AccountMeta::new_readonly(payment_terms, false),    // payment_terms
            AccountMeta::new_readonly(payee_pda, false), // payee
            AccountMeta::new_readonly(payer, true),  // payer (signer)
        ];

        let cancel_sub_args = PauseAgreementArgs {};
        let cancel_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "pause_agreement")
            data.extend_from_slice(&[130, 90, 85, 99, 205, 60, 132, 245]);
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

impl InitPayeeBuilder {
    /// Create a new init payee builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the payee authority
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

    /// Set the program ID to use
    #[must_use]
    pub const fn program_id(mut self, program_id: Pubkey) -> Self {
        self.program_id = Some(program_id);
        self
    }

    /// Build the transaction instruction
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `init_payee` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let usdc_mint = self.usdc_mint.ok_or("USDC mint not set")?;
        let treasury_ata = self.treasury_ata.ok_or("Treasury ATA not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let payee_pda = pda::payee_address_with_program_id(&authority, &program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(payee_pda, false),          // payee (PDA)
            AccountMeta::new(authority, true),              // authority (signer)
            AccountMeta::new_readonly(usdc_mint, false),    // usdc_mint
            AccountMeta::new_readonly(treasury_ata, false), // treasury_ata
            AccountMeta::new_readonly(spl_token::id(), false), // token_program
            AccountMeta::new_readonly(spl_associated_token_account::id(), false), // associated_token_program
            AccountMeta::new_readonly(system_program::ID, false),                 // system_program
        ];

        let args = InitPayeeArgs {
            usdc_mint,
            treasury_ata,
        };

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "init_payee")
            data.extend_from_slice(&[145, 253, 226, 173, 120, 41, 140, 49]);
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

impl CreatePaymentTermsBuilder {
    /// Create a new create payment terms builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the payee authority
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

    /// Set the `payment_terms` creation arguments
    #[must_use]
    pub fn payment_terms_args(mut self, args: CreatePaymentTermsArgs) -> Self {
        self.payment_terms_args = Some(args);
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
    /// * `Ok(Instruction)` - The `create_payment_terms` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let authority = self.authority.ok_or("Authority not set")?;
        let _payer = self.payer.unwrap_or(authority);
        let payment_terms_args = self.payment_terms_args.ok_or("PaymentTerms args not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let payee_pda = pda::payee_address_with_program_id(&authority, &program_id);
        let payment_terms_pda =
            pda::payment_terms_address_with_program_id(&payee_pda, &payment_terms_args.terms_id_bytes, &program_id);

        let accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(payment_terms_pda, false),              // payment_terms (PDA)
            AccountMeta::new_readonly(payee_pda, false), // payee
            AccountMeta::new(authority, true),              // authority (signer)
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ];

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "create_payment_terms")
            data.extend_from_slice(&[220, 74, 165, 113, 140, 252, 204, 241]);
            borsh::to_writer(&mut data, &payment_terms_args)
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


#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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

        // Compute program data address for upgrade authority validation
        let (program_data_address, _) = Pubkey::find_program_address(
            &[program_id.as_ref()],
            &anchor_lang::solana_program::bpf_loader_upgradeable::id(),
        );

        // Compute platform treasury ATA
        let platform_treasury_ata = crate::ata::get_associated_token_address_for_mint(
            &config_args.platform_authority,
            &config_args.allowed_mint,
        )?;

        let accounts = vec![
            AccountMeta::new(config_pda, false),                      // config (PDA)
            AccountMeta::new(authority, true),                        // authority (signer)
            AccountMeta::new_readonly(program_data_address, false),   // program_data
            AccountMeta::new_readonly(platform_treasury_ata, false),  // platform_treasury_ata
            AccountMeta::new_readonly(spl_token::ID, false),          // token_program
            AccountMeta::new_readonly(system_program::ID, false),     // system_program
        ];

        let data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "global:init_config")
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

impl ExecutePaymentBuilder {
    /// Create a new execute payment builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `payment_terms` PDA
    #[must_use]
    pub const fn payment_terms(mut self, payment_terms: Pubkey) -> Self {
        self.payment_terms = Some(payment_terms);
        self
    }

    /// Set the payer pubkey
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
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
    /// * `payee` - The payee account data
    /// * `payment_terms_data` - The `payment_terms` account data
    /// * `platform_treasury_ata` - Platform treasury ATA address
    ///
    /// # Returns
    /// * `Ok(Instruction)` - The `renew_payment_agreement` instruction
    /// * `Err(TallyError)` - If building fails
    #[allow(clippy::similar_names)] // payer and payee are distinct payment domain terms
    pub fn build_instruction(
        self,
        payee: &Payee,
        _payment_terms_data: &PaymentTerms,
        platform_treasury_ata: &Pubkey,
    ) -> Result<Instruction> {
        let payment_terms = self.payment_terms.ok_or("PaymentTerms not set")?;
        let payer = self.payer.ok_or("Payer not set")?;
        let keeper = self.keeper.ok_or("Keeper not set")?;
        let keeper_ata = self.keeper_ata.ok_or("Keeper ATA not set")?;
        let token_program = self.token_program.unwrap_or(TokenProgram::Token);

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute required PDAs
        let config_pda = pda::config_address_with_program_id(&program_id);
        let payee_pda = pda::payee_address_with_program_id(&payee.authority, &program_id);
        let payment_agreement_pda =
            pda::payment_agreement_address_with_program_id(&payment_terms, &payer, &program_id);
        let delegate_pda = pda::delegate_address_with_program_id(&program_id);
        let payer_ata = get_associated_token_address_with_program(
            &payer,
            &payee.usdc_mint,
            token_program,
        )?;

        // Create renew_payment_agreement instruction
        let renew_sub_accounts = vec![
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(payment_agreement_pda, false),      // payment agreement (PDA, mutable)
            AccountMeta::new_readonly(payment_terms, false),         // payment_terms
            AccountMeta::new_readonly(payee_pda, false), // payee
            AccountMeta::new(payer_ata, false),        // payer_usdc_ata (mutable)
            AccountMeta::new(payee.treasury_ata, false), // payee_treasury_ata (mutable)
            AccountMeta::new(*platform_treasury_ata, false), // platform_treasury_ata (mutable)
            AccountMeta::new(keeper, true),                 // keeper (signer, mutable for fees)
            AccountMeta::new(keeper_ata, false),            // keeper_usdc_ata (mutable)
            AccountMeta::new_readonly(payee.usdc_mint, false), // usdc_mint
            AccountMeta::new_readonly(delegate_pda, false), // program_delegate
            AccountMeta::new_readonly(token_program.program_id(), false), // token_program
        ];

        let renew_sub_args = crate::program_types::ExecutePaymentArgs {};
        let renew_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "execute_payment")
            data.extend_from_slice(&[86, 4, 7, 7, 120, 139, 232, 139]);
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

impl CloseAgreementBuilder {
    /// Create a new close agreement builder
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the `payment_terms` PDA
    #[must_use]
    pub const fn payment_terms(mut self, payment_terms: Pubkey) -> Self {
        self.payment_terms = Some(payment_terms);
        self
    }

    /// Set the payer pubkey
    #[must_use]
    pub const fn payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
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
    /// * `Ok(Instruction)` - The `close_payment_agreement` instruction
    /// * `Err(TallyError)` - If building fails
    pub fn build_instruction(self) -> Result<Instruction> {
        let payment_terms = self.payment_terms.ok_or("PaymentTerms not set")?;
        let payer = self.payer.ok_or("Payer not set")?;

        let program_id = self.program_id.unwrap_or_else(program_id);

        // Compute payment agreement PDA
        let payment_agreement_pda =
            pda::payment_agreement_address_with_program_id(&payment_terms, &payer, &program_id);

        // Create close_payment_agreement instruction
        let close_sub_accounts = vec![
            AccountMeta::new(payment_agreement_pda, false), // payment agreement (PDA, mutable, will be closed)
            AccountMeta::new(payer, true), // payer (signer, mutable, receives rent)
        ];

        let close_sub_args = crate::program_types::CloseAgreementArgs {};
        let close_sub_data = {
            let mut data = Vec::new();
            // Instruction discriminator (computed from "close_agreement")
            data.extend_from_slice(&[48, 34, 42, 18, 144, 209, 198, 55]);
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

#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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

#[cfg(feature = "platform-admin")]
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


// Convenience functions for common transaction building patterns

/// Create a start agreement transaction builder
#[must_use]
pub fn start_agreement() -> StartAgreementBuilder {
    StartAgreementBuilder::new()
}

/// Create a pause agreement transaction builder
#[must_use]
pub fn pause_agreement() -> PauseAgreementBuilder {
    PauseAgreementBuilder::new()
}

/// Create a payee initialization transaction builder
#[must_use]
pub fn init_payee() -> InitPayeeBuilder {
    InitPayeeBuilder::new()
}

/// Create a `payment_terms` creation transaction builder
#[must_use]
pub fn create_payment_terms() -> CreatePaymentTermsBuilder {
    CreatePaymentTermsBuilder::new()
}

/// Create an admin withdraw fees transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn admin_withdraw_fees() -> AdminWithdrawFeesBuilder {
    AdminWithdrawFeesBuilder::new()
}

/// Create a config initialization transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn init_config() -> InitConfigBuilder {
    InitConfigBuilder::new()
}


/// Create a execute payment transaction builder
#[must_use]
pub fn execute_payment() -> ExecutePaymentBuilder {
    ExecutePaymentBuilder::new()
}

/// Create a close agreement transaction builder
#[must_use]
pub fn close_agreement() -> CloseAgreementBuilder {
    CloseAgreementBuilder::new()
}

/// Create a transfer authority transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn transfer_authority() -> TransferAuthorityBuilder {
    TransferAuthorityBuilder::new()
}

/// Create an accept authority transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn accept_authority() -> AcceptAuthorityBuilder {
    AcceptAuthorityBuilder::new()
}

/// Create a cancel authority transfer transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn cancel_authority_transfer() -> CancelAuthorityTransferBuilder {
    CancelAuthorityTransferBuilder::new()
}

/// Create a pause program transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn pause() -> PauseBuilder {
    PauseBuilder::new()
}

/// Create an unpause program transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn unpause() -> UnpauseBuilder {
    UnpauseBuilder::new()
}

/// Create an update config transaction builder
#[must_use]
#[cfg(feature = "platform-admin")]
pub fn update_config() -> UpdateConfigBuilder {
    UpdateConfigBuilder::new()
}


#[cfg(test)]
mod tests {
    #[cfg(feature = "platform-admin")]
    use super::*;
    #[cfg(feature = "platform-admin")]
    use anchor_client::solana_sdk::signature::{Keypair, Signer};
    #[cfg(feature = "platform-admin")]
    use std::str::FromStr;

    #[cfg(feature = "platform-admin")]
    fn create_test_payee() -> Payee {
        Payee {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            volume_tier: 0, // Standard tier
            monthly_volume_usdc: 0,
            last_volume_update_ts: 0,
            bump: 255,
        }
    }

    #[cfg(feature = "platform-admin")]
    fn create_test_payment_terms() -> PaymentTerms {
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let mut terms_id = [0u8; 32];
        terms_id[..12].copy_from_slice(b"premium_payment_terms");
        let mut name = [0u8; 32];
        name[..12].copy_from_slice(b"Premium PaymentTerms");

        PaymentTerms {
            payee,
            terms_id,
            amount_usdc: 5_000_000,  // 5 USDC
            period_secs: 2_592_000, // 30 days
            grace_secs: 432_000,    // 5 days
            name,
            active: true,
        }
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_start_payment_agreement_builder() {
        let payee = create_test_payee();
        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instructions = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(3) // 3x payment_terms price
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        assert_eq!(instructions.len(), 2, "Should have 2 instructions: approve_checked and start_payment_agreement");

        // ========== Validate approve_checked instruction ==========
        let approve_ix = &instructions[0];
        assert_eq!(approve_ix.program_id, spl_token::id(), "First instruction must be SPL Token program");

        // Validate allowance calculation (3 periods × payment_terms price)
        let expected_allowance = payment_terms_data.amount_usdc.checked_mul(3).expect("Allowance overflow");

        // Decode approve_checked data to validate amount
        // approve_checked format: [discriminator(1 byte), amount(8 bytes), decimals(1 byte)]
        assert!(approve_ix.data.len() >= 10, "approve_checked data should be at least 10 bytes");

        // The first byte should be the ApproveChecked discriminator (13)
        assert_eq!(approve_ix.data[0], 13, "approve_checked instruction discriminator should be 13");

        // Extract amount (bytes 1-8, little-endian u64)
        let amount_bytes: [u8; 8] = approve_ix.data[1..9].try_into().unwrap();
        let actual_amount = u64::from_le_bytes(amount_bytes);
        assert_eq!(actual_amount, expected_allowance, "Allowance amount should be 3x payment_terms price");

        // Decimals byte (byte 9) should be 6 for USDC
        assert_eq!(approve_ix.data[9], 6, "USDC decimals should be 6");

        // Validate approve_checked accounts structure
        assert_eq!(approve_ix.accounts.len(), 4, "approve_checked requires 4 accounts");

        // Account 0: Source account (payer's token account, writable)
        assert!(approve_ix.accounts[0].is_writable, "Source account must be writable");

        // Account 1: Mint (USDC mint, readonly)
        assert!(!approve_ix.accounts[1].is_writable, "Mint must be readonly");
        assert_eq!(approve_ix.accounts[1].pubkey, payee.usdc_mint, "Second account must be USDC mint");

        // Account 2: Delegate (program delegate PDA, readonly)
        assert!(!approve_ix.accounts[2].is_writable, "Delegate must be readonly");

        // Account 3: Owner (payer, signer)
        assert!(approve_ix.accounts[3].is_signer, "Owner must be signer");

        // ========== Validate start_payment_agreement instruction ==========
        let start_sub_ix = &instructions[1];
        let expected_program_id = program_id();
        assert_eq!(start_sub_ix.program_id, expected_program_id, "Second instruction must be Tally program");

        // Validate start_payment_agreement discriminator
        // start_payment_agreement discriminator: [167, 59, 160, 222, 194, 175, 3, 13]
        assert!(start_sub_ix.data.len() >= 8, "Instruction data should include discriminator");
        assert_eq!(&start_sub_ix.data[0..8], &[167, 59, 160, 222, 194, 175, 3, 13],
            "start_payment_agreement discriminator mismatch");

        // Validate account count for start_payment_agreement
        assert_eq!(start_sub_ix.accounts.len(), 12, "start_payment_agreement requires 12 accounts");

        // Validate key accounts are present (specific indices)
        // Account indices based on actual start_payment_agreement builder:
        // 0: config, 1: payment agreement, 2: payment_terms, 3: payee, 4: payer,
        // 5: payer_ata, 6: payee_treasury_ata, 7: platform_treasury_ata,
        // 8: usdc_mint, 9: delegate, 10: token_program, 11: system_program

        assert_eq!(start_sub_ix.accounts[2].pubkey, payment_terms_key, "PaymentTerms account mismatch");
        assert_eq!(start_sub_ix.accounts[8].pubkey, payee.usdc_mint, "USDC mint account mismatch");
        assert_eq!(start_sub_ix.accounts[7].pubkey, platform_treasury_ata, "Platform treasury ATA mismatch");

        // Payer must be signer and writable (pays for payment agreement account creation)
        assert!(start_sub_ix.accounts[4].is_signer, "Payer must be signer");
        assert!(start_sub_ix.accounts[4].is_writable, "Payer must be writable");

        // PaymentAgreement PDA must be writable (created in instruction)
        assert!(start_sub_ix.accounts[1].is_writable, "PaymentAgreement PDA must be writable");
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_start_payment_agreement_allowance_edge_cases() {
        let payee = create_test_payee();
        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test with 1 period (minimum)
        let instructions_min = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(1)
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        let approve_ix_min = &instructions_min[0];
        let amount_bytes_min: [u8; 8] = approve_ix_min.data[1..9].try_into().unwrap();
        let amount_min = u64::from_le_bytes(amount_bytes_min);
        assert_eq!(amount_min, payment_terms_data.amount_usdc, "1 period should equal payment_terms price");

        // Test with 10 periods
        let instructions_max = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(10)
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        let approve_ix_max = &instructions_max[0];
        let amount_bytes_max: [u8; 8] = approve_ix_max.data[1..9].try_into().unwrap();
        let amount_max = u64::from_le_bytes(amount_bytes_max);
        assert_eq!(amount_max, payment_terms_data.amount_usdc * 10, "10 periods should be 10x payment_terms price");
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_start_payment_agreement_with_token2022() {
        let mut payee = create_test_payee();
        payee.usdc_mint = Pubkey::from(Keypair::new().pubkey().to_bytes()); // Custom Token2022 mint

        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instructions = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(3)
            .token_program(TokenProgram::Token2022)
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        // Verify Token2022 program is used
        assert_eq!(instructions[0].program_id, spl_token_2022::id(), "Should use Token2022 program");
        assert_eq!(instructions[1].accounts[10].pubkey, spl_token_2022::id(),
            "start_payment_agreement should reference Token2022");
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_cancel_payment_agreement_builder() {
        let payee = create_test_payee();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instructions = pause_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .build_instructions(&payee)
            .unwrap();

        assert_eq!(instructions.len(), 2);

        // First instruction should be revoke
        assert_eq!(instructions[0].program_id, spl_token::id());

        // Second instruction should be cancel_payment_agreement
        let program_id = program_id();
        assert_eq!(instructions[1].program_id, program_id);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_create_payee_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = init_payee()
            .authority(authority)
            .usdc_mint(usdc_mint)
            .treasury_ata(treasury_ata)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 8);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_create_payment_terms_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let terms_id_bytes = {
            let mut bytes = [0u8; 32];
            let id_bytes = b"premium";
            let len = id_bytes.len().min(32);
            bytes[..len].copy_from_slice(&id_bytes[..len]);
            bytes
        };

        let payment_terms_args = CreatePaymentTermsArgs {
            terms_id: "premium".to_string(),
            terms_id_bytes,
            amount_usdc: 5_000_000,
            period_secs: 2_592_000,
            grace_secs: 432_000,
            name: "Premium PaymentTerms".to_string(),
        };

        let instruction = create_payment_terms()
            .authority(authority)
            .payment_terms_args(payment_terms_args)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 5);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_builder_missing_required_fields() {
        let payee = create_test_payee();
        let payment_terms_data = create_test_payment_terms();
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing payment_terms
        let result = start_agreement()
            .payer(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .allowance_periods(3)
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PaymentTerms not set"));

        // Test missing payer
        let result = start_agreement()
            .payment_terms(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .allowance_periods(3)
            .build_instructions(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Payer not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_token_program_variants() {
        // Create separate payees for different token programs to avoid compatibility issues
        let payee_token = Payee {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()), // Use a test mint for classic token
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            volume_tier: 0, // Standard tier
            monthly_volume_usdc: 0,
            last_volume_update_ts: 0,
            bump: 255,
        };

        let payee_token2022 = Payee {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()), // Use a different test mint for Token-2022
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            volume_tier: 0, // Standard tier
            monthly_volume_usdc: 0,
            last_volume_update_ts: 0,
            bump: 255,
        };

        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test with Token-2022
        let instructions_token2022 = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(3)
            .token_program(TokenProgram::Token2022)
            .build_instructions(&payee_token2022, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        // Test with classic Token
        let instructions_token = start_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .allowance_periods(3)
            .token_program(TokenProgram::Token)
            .build_instructions(&payee_token, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        // Both should work but have different token program IDs
        assert_eq!(instructions_token2022[0].program_id, spl_token_2022::id());
        assert_eq!(instructions_token[0].program_id, spl_token::id());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_init_config_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let config_args = InitConfigArgs {
            platform_authority: authority,
            max_platform_fee_bps: 1000,
            min_platform_fee_bps: 50,
            min_period_seconds: 86400,
            default_allowance_periods: 3,
            allowed_mint: usdc_mint,
            max_withdrawal_amount: 1_000_000_000,
            max_grace_period_seconds: 2_592_000,
            keeper_fee_bps: 50,
        };

        let instruction = init_config()
            .authority(authority)
            .config_args(config_args)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        // Updated to reflect actual account count (config, authority, payer, platform_treasury, allowed_mint, system_program)
        assert_eq!(instruction.accounts.len(), 6);

        // Verify key account metas
        assert!(!instruction.accounts[0].is_signer); // config (PDA)
        assert!(instruction.accounts[0].is_writable); // config (PDA)
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_init_config_missing_required_fields() {
        // Test missing authority
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let result = init_config()
            .config_args(InitConfigArgs {
                platform_authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
                max_platform_fee_bps: 1000,
                min_platform_fee_bps: 50,
                min_period_seconds: 86400,
                default_allowance_periods: 3,
                allowed_mint: usdc_mint,
                max_withdrawal_amount: 1_000_000_000,
                max_grace_period_seconds: 2_592_000,
                keeper_fee_bps: 50,
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
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_builder() {
        let payee = create_test_payee();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let update_args = UpdatePaymentTermsArgs::new()
            .with_name("Updated PaymentTerms".to_string())
            .with_active(false)
            .with_amount_usdc(10_000_000); // 10 USDC

        let instruction = update_payment_terms()
            .authority(payee.authority)
            .payment_terms_key(payment_terms_key)
            .update_args(update_args)
            .build_instruction(&payee)
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 4);

        // Verify account structure
        assert!(!instruction.accounts[0].is_signer); // config (readonly)
        assert!(!instruction.accounts[0].is_writable); // config (readonly)
        assert!(!instruction.accounts[1].is_signer); // payment_terms (mutable, but not signer)
        assert!(instruction.accounts[1].is_writable); // payment_terms (mutable)
        assert!(!instruction.accounts[2].is_signer); // payee (readonly)
        assert!(!instruction.accounts[2].is_writable); // payee (readonly)
        assert!(instruction.accounts[3].is_signer); // authority (signer)
        assert!(instruction.accounts[3].is_writable); // authority (mutable for fees)
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_builder_missing_required_fields() {
        let payee = create_test_payee();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing authority
        let result = update_payment_terms()
            .payment_terms_key(payment_terms_key)
            .update_args(update_args.clone())
            .build_instruction(&payee);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority not set"));

        // Test missing payment_terms key
        let result = update_payment_terms()
            .authority(payee.authority)
            .update_args(update_args)
            .build_instruction(&payee);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PaymentTerms key not set"));

        // Test missing update args
        let result = update_payment_terms()
            .authority(payee.authority)
            .payment_terms_key(payment_terms_key)
            .build_instruction(&payee);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Update args not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_builder_validation() {
        let payee = create_test_payee();
        let wrong_authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test wrong authority
        let result = update_payment_terms()
            .authority(wrong_authority)
            .payment_terms_key(payment_terms_key)
            .update_args(update_args)
            .build_instruction(&payee);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority does not match payee authority"));

        // Test empty update args
        let result = update_payment_terms()
            .authority(payee.authority)
            .payment_terms_key(payment_terms_key)
            .update_args(empty_args)
            .build_instruction(&payee);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No updates specified"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_args_functionality() {
        let args = UpdatePaymentTermsArgs::new()
            .with_name("New PaymentTerms Name".to_string())
            .with_active(true)
            .with_amount_usdc(5_000_000)
            .with_period_secs(2_592_000)
            .with_grace_secs(432_000);

        assert!(args.has_updates());
        assert_eq!(args.name, Some("New PaymentTerms Name".to_string()));
        assert_eq!(args.active, Some(true));
        assert_eq!(args.amount_usdc, Some(5_000_000));
        assert_eq!(args.period_secs, Some(2_592_000));
        assert_eq!(args.grace_secs, Some(432_000));

        // Test name_bytes conversion
        let name_bytes = args.name_bytes().unwrap();
        let expected_name = "New PaymentTerms Name";
        assert_eq!(&name_bytes[..expected_name.len()], expected_name.as_bytes());
        // Check that the rest of the array is zero-padded
        for &byte in &name_bytes[expected_name.len()..] {
            assert_eq!(byte, 0);
        }

        // Test empty args
        assert!(!empty_args.has_updates());
        assert!(empty_args.name_bytes().is_none());

        // Test default
        assert!(!default_args.has_updates());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_renew_payment_agreement_builder() {
        let payee = create_test_payee();
        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = execute_payment()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .build_instruction(&payee, &payment_terms_data, &platform_treasury_ata)
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

    #[cfg(feature = "platform-admin")]
    fn verify_renew_readonly_accounts(instruction: &Instruction) {
        assert!(!instruction.accounts[0].is_writable); // config
        assert!(!instruction.accounts[2].is_writable); // payment_terms
        assert!(!instruction.accounts[3].is_writable); // payee
        assert!(!instruction.accounts[9].is_writable); // usdc_mint
        assert!(!instruction.accounts[10].is_writable); // program_delegate
        assert!(!instruction.accounts[11].is_writable); // token_program
    }

    #[cfg(feature = "platform-admin")]
    fn verify_renew_mutable_accounts(instruction: &Instruction) {
        assert!(instruction.accounts[1].is_writable); // payment agreement
        assert!(instruction.accounts[4].is_writable); // payer_usdc_ata
        assert!(instruction.accounts[5].is_writable); // payee_treasury_ata
        assert!(instruction.accounts[6].is_writable); // platform_treasury_ata
        assert!(instruction.accounts[7].is_writable); // keeper
        assert!(instruction.accounts[8].is_writable); // keeper_usdc_ata
    }

    #[cfg(feature = "platform-admin")]
    fn verify_renew_signer_accounts(instruction: &Instruction) {
        assert!(!instruction.accounts[0].is_signer); // config
        assert!(!instruction.accounts[1].is_signer); // payment agreement
        assert!(!instruction.accounts[2].is_signer); // payment_terms
        assert!(!instruction.accounts[3].is_signer); // payee
        assert!(!instruction.accounts[4].is_signer); // payer_usdc_ata
        assert!(!instruction.accounts[5].is_signer); // payee_treasury_ata
        assert!(!instruction.accounts[6].is_signer); // platform_treasury_ata
        assert!(instruction.accounts[7].is_signer); // keeper (only signer)
        assert!(!instruction.accounts[8].is_signer); // keeper_usdc_ata
        assert!(!instruction.accounts[9].is_signer); // usdc_mint
        assert!(!instruction.accounts[10].is_signer); // program_delegate
        assert!(!instruction.accounts[11].is_signer); // token_program
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_renew_payment_agreement_builder_missing_required_fields() {
        let payee = create_test_payee();
        let payment_terms_data = create_test_payment_terms();
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing payment_terms
        let result = execute_payment()
            .payer(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PaymentTerms not set"));

        // Test missing payer
        let result = execute_payment()
            .payment_terms(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Payer not set"));

        // Test missing keeper
        let result = execute_payment()
            .payment_terms(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .payer(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper_ata(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Keeper not set"));

        // Test missing keeper_ata
        let result = execute_payment()
            .payment_terms(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .payer(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .keeper(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction(&payee, &payment_terms_data, &platform_treasury_ata);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Keeper ATA not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_renew_payment_agreement_token_program_variants() {
        // Create separate payees for different token programs
        let payee_token = Payee {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            volume_tier: 0, // Standard tier
            monthly_volume_usdc: 0,
            last_volume_update_ts: 0,
            bump: 255,
        };

        let payee_token2022 = Payee {
            authority: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            usdc_mint: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            treasury_ata: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            platform_fee_bps: 50,
            volume_tier: 0, // Standard tier
            monthly_volume_usdc: 0,
            last_volume_update_ts: 0,
            bump: 255,
        };

        let payment_terms_data = create_test_payment_terms();
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let keeper_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let platform_treasury_ata = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test with Token-2022
        let instruction_token2022 = execute_payment()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .token_program(TokenProgram::Token2022)
            .build_instruction(&payee_token2022, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        // Test with classic Token
        let instruction_token = execute_payment()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .keeper(keeper)
            .keeper_ata(keeper_ata)
            .token_program(TokenProgram::Token)
            .build_instruction(&payee_token, &payment_terms_data, &platform_treasury_ata)
            .unwrap();

        // Both should work but have different token program IDs in the accounts
        assert_eq!(
            instruction_token2022.accounts[11].pubkey,
            spl_token_2022::id()
        );
        assert_eq!(instruction_token.accounts[11].pubkey, spl_token::id());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_close_payment_agreement_builder() {
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 2);

        // Verify instruction discriminator matches program
        assert_eq!(&instruction.data[..8], &[33, 214, 169, 135, 35, 127, 78, 7]);

        // Verify account structure
        assert!(instruction.accounts[0].is_writable); // payment agreement (mutable)
        assert!(!instruction.accounts[0].is_signer); // payment agreement (not signer, it's a PDA)
        assert!(instruction.accounts[1].is_writable); // payer (mutable, receives rent)
        assert!(instruction.accounts[1].is_signer); // payer (signer)
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_close_payment_agreement_builder_missing_required_fields() {
        // Test missing payment_terms
        let result = close_agreement()
            .payer(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PaymentTerms not set"));

        // Test missing payer
        let result = close_agreement()
            .payment_terms(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Payer not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_close_payment_agreement_builder_custom_program_id() {
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_close_payment_agreement_builder_pda_computation() {
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = close_agreement()
            .payment_terms(payment_terms_key)
            .payer(payer)
            .build_instruction()
            .unwrap();

        // Verify the computed payment agreement PDA is correct
        let program_id = program_id();
        let expected_payment_agreement_pda =
            pda::payment_agreement_address_with_program_id(&payment_terms_key, &payer, &program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_payment_agreement_pda);
        assert_eq!(instruction.accounts[1].pubkey, payer);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_accept_authority_builder_default() {
        let builder = AcceptAuthorityBuilder::default();
        assert!(builder.new_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_cancel_authority_transfer_builder_default() {
        let builder = CancelAuthorityTransferBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_pause_builder_default() {
        let builder = PauseBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_unpause_builder_default() {
        let builder = UnpauseBuilder::default();
        assert!(builder.platform_authority.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_update_config_builder_missing_authority() {
        let result = update_config().keeper_fee_bps(25).build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Platform authority not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
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
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_payee_tier()
            .authority(authority)
            .payee(payee)
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
        assert!(instruction.accounts[1].is_writable); // payee (mutable)
        assert!(!instruction.accounts[1].is_signer); // payee (PDA, not signer)
        assert!(!instruction.accounts[2].is_writable); // authority (readonly)
        assert!(instruction.accounts[2].is_signer); // authority (signer)

        // Verify account addresses
        assert_eq!(
            instruction.accounts[0].pubkey,
            pda::config_address_with_program_id(&program_id)
        );
        assert_eq!(instruction.accounts[1].pubkey, payee);
        assert_eq!(instruction.accounts[2].pubkey, authority);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_all_tiers() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test Free tier (0)
        let instruction_free = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(0)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_free.accounts.len(), 3);

        // Test Pro tier (1)
        let instruction_pro = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(1)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_pro.accounts.len(), 3);

        // Test Enterprise tier (2)
        let instruction_enterprise = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(2)
            .build_instruction()
            .unwrap();
        assert_eq!(instruction_enterprise.accounts.len(), 3);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_missing_required_fields() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test missing authority
        let result = update_payee_tier()
            .payee(payee)
            .new_tier(1)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Authority not set"));

        // Test missing payee
        let result = update_payee_tier()
            .authority(authority)
            .new_tier(1)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Payee not set"));

        // Test missing new_tier
        let result = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New tier not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_invalid_tier() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test tier > 2 (invalid)
        let result = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(3)
            .build_instruction();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("New tier must be 0 (Free), 1 (Pro), or 2 (Enterprise)"));

        // Test tier 255 (invalid)
        let result = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(255)
            .build_instruction();
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_custom_program_id() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let custom_program_id = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(1)
            .program_id(custom_program_id)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, custom_program_id);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_pda_computation() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(1)
            .build_instruction()
            .unwrap();

        // Verify the computed config PDA is correct
        let program_id = program_id();
        let expected_config_pda = pda::config_address_with_program_id(&program_id);

        assert_eq!(instruction.accounts[0].pubkey, expected_config_pda);
        assert_eq!(instruction.accounts[1].pubkey, payee);
        assert_eq!(instruction.accounts[2].pubkey, authority);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_args_serialization() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test that args can be serialized and included in instruction data
        let instruction = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(1) // Pro tier
            .build_instruction()
            .unwrap();

        // Verify the data contains the discriminator (8 bytes) followed by serialized args
        // UpdatePayeeTierArgs has 1 u8 field, so data should be 9 bytes
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
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_clone_debug() {
        let builder = update_payee_tier()
            .authority(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .payee(Pubkey::from(Keypair::new().pubkey().to_bytes()))
            .new_tier(1);

        // Test Clone trait
        let cloned_builder = builder.clone();
        assert_eq!(cloned_builder.authority, builder.authority);
        assert_eq!(cloned_builder.payee, builder.payee);
        assert_eq!(cloned_builder.new_tier, builder.new_tier);

        // Test Debug trait
        let debug_str = format!("{builder:?}");
        assert!(debug_str.contains("UpdatePayeeTierBuilder"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_builder_default() {
        let builder = UpdatePayeeTierBuilder::default();
        assert!(builder.authority.is_none());
        assert!(builder.payee.is_none());
        assert!(builder.new_tier.is_none());
        assert!(builder.program_id.is_none());
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payee_tier_convenience_function() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payee = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test using convenience function
        let instruction = update_payee_tier()
            .authority(authority)
            .payee(payee)
            .new_tier(1)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = UpdatePayeeTierBuilder::new()
            .authority(authority)
            .payee(payee)
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

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_builder_all_fields() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(10_000_000) // 10 USDC
            .period_secs(2_592_000) // 30 days
            .grace_secs(777_600)    // 9 days (< 30% of 30 days = 7.776 days)
            .name("Updated PaymentTerms".to_string())
            .build_instruction()
            .unwrap();

        let program_id = program_id();
        assert_eq!(instruction.program_id, program_id);
        assert_eq!(instruction.accounts.len(), 4);

        // Verify discriminator is correct for "global:update_payment_terms_terms"
        assert_eq!(&instruction.data[..8], &[224, 68, 224, 41, 169, 52, 124, 221]);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_builder_single_field() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        // Test updating only price
        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(20_000_000)
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 4);

        // Test updating only period
        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .period_secs(604_800) // 7 days
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 4);

        // Test updating only grace
        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .grace_secs(86_400) // 1 day
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 4);

        // Test updating only name
        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .name("New Name".to_string())
            .build_instruction()
            .unwrap();

        assert_eq!(instruction.program_id, program_id());
        assert_eq!(instruction.accounts.len(), 4);
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_no_fields() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one field must be set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_missing_authority() {
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .payment_terms_key(payment_terms_key)
            .amount_usdc(10_000_000)
            .build_instruction();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Authority not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_missing_payment_terms() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .amount_usdc(10_000_000)
            .build_instruction();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PaymentTerms key not set"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_zero_price() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(0)
            .build_instruction();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Price must be > 0"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_max_price() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(1_000_000_000_001) // Just over 1 million USDC
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Price must be <= 1,000,000 USDC"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_grace_exceeds_30_percent() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .period_secs(2_592_000) // 30 days
            .grace_secs(800_000)    // > 30% of 30 days (should be <= 777,600)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Grace period must be <= 30%"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_empty_name() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .name(String::new())
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Name must not be empty"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_validation_name_too_long() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let long_name = "a".repeat(33); // 33 bytes, > 32

        let result = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .name(long_name)
            .build_instruction();

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Name must be <= 32 bytes"));
    }

    #[test]
    #[cfg(feature = "platform-admin")]
    fn test_update_payment_terms_terms_convenience_function() {
        let authority = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let payment_terms_key = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let instruction = update_payment_terms_terms()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(15_000_000)
            .build_instruction()
            .unwrap();

        // Verify it works the same as using the builder directly
        let direct_instruction = UpdatePaymentTermsTermsBuilder::new()
            .authority(authority)
            .payment_terms_key(payment_terms_key)
            .amount_usdc(15_000_000)
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
