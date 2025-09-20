//! Simple client for basic Tally SDK operations

use crate::{
    error::{Result, TallyError},
    program_id_string,
    program_types::{Merchant, Plan, Subscription},
};
use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_lang::AnchorDeserialize;
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signer,
    transaction::Transaction,
};
use std::str::FromStr;

/// Simple Tally client for basic operations
pub struct SimpleTallyClient {
    /// RPC client for queries
    pub rpc_client: RpcClient,
    /// Program ID
    pub program_id: Pubkey,
}

impl SimpleTallyClient {
    /// Create a new simple Tally client
    ///
    /// # Arguments
    /// * `cluster_url` - RPC endpoint URL
    ///
    /// # Returns
    /// * `Ok(SimpleTallyClient)` - The client instance
    ///
    /// # Errors
    /// Returns an error if the program ID cannot be parsed or client creation fails
    pub fn new(cluster_url: &str) -> Result<Self> {
        let rpc_client = RpcClient::new_with_commitment(cluster_url, CommitmentConfig::confirmed());
        let program_id = Pubkey::from_str(&program_id_string())
            .map_err(|e| TallyError::Generic(format!("Invalid program ID: {e}")))?;

        Ok(Self {
            rpc_client,
            program_id,
        })
    }

    /// Create a new simple Tally client with custom program ID
    ///
    /// # Arguments
    /// * `cluster_url` - RPC endpoint URL
    /// * `program_id` - Custom program ID to use
    ///
    /// # Returns
    /// * `Ok(SimpleTallyClient)` - The client instance
    ///
    /// # Errors
    /// Returns an error if the program ID cannot be parsed or client creation fails
    pub fn new_with_program_id(cluster_url: &str, program_id: &str) -> Result<Self> {
        let rpc_client = RpcClient::new_with_commitment(cluster_url, CommitmentConfig::confirmed());
        let program_id = Pubkey::from_str(program_id)
            .map_err(|e| TallyError::Generic(format!("Invalid program ID '{program_id}': {e}")))?;

        Ok(Self {
            rpc_client,
            program_id,
        })
    }

    /// Get the program ID
    #[must_use]
    pub const fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// Compute merchant PDA using this client's program ID
    pub fn merchant_address(&self, authority: &Pubkey) -> Pubkey {
        crate::pda::merchant_address_with_program_id(authority, &self.program_id)
    }

    /// Get the RPC client
    pub const fn rpc(&self) -> &RpcClient {
        &self.rpc_client
    }

    /// Check if an account exists
    ///
    /// # Errors
    /// Returns an error if the RPC call to check account existence fails
    pub fn account_exists(&self, address: &Pubkey) -> Result<bool> {
        // First try with confirmed commitment
        match self
            .rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
        {
            Ok(response) => match response.value {
                Some(_) => Ok(true),
                None => Ok(false),
            },
            Err(e) => {
                // If confirmed fails, try with processed commitment (more recent but less reliable)
                match self
                    .rpc_client
                    .get_account_with_commitment(address, CommitmentConfig::processed())
                {
                    Ok(response) => match response.value {
                        Some(_) => Ok(true),
                        None => Ok(false),
                    },
                    Err(processed_err) => Err(TallyError::Generic(format!(
                        "Failed to fetch account with both confirmed and processed commitment. Confirmed error: {e}, Processed error: {processed_err}"
                    ))),
                }
            }
        }
    }

    /// Get merchant account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_merchant(&self, merchant_address: &Pubkey) -> Result<Option<Merchant>> {
        let account_data = match self
            .rpc_client
            .get_account_with_commitment(merchant_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch merchant account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic(
                "Invalid merchant account data".to_string(),
            ));
        }

        let merchant = Merchant::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize merchant: {e}")))?;

        Ok(Some(merchant))
    }

    /// Get plan account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_plan(&self, plan_address: &Pubkey) -> Result<Option<Plan>> {
        let account_data = match self
            .rpc_client
            .get_account_with_commitment(plan_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch plan account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic("Invalid plan account data".to_string()));
        }

        let plan = Plan::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize plan: {e}")))?;

        Ok(Some(plan))
    }

    /// List all plans for a merchant
    ///
    /// # Errors
    /// Returns an error if the RPC query fails or accounts can't be deserialized
    pub fn list_plans(&self, merchant_address: &Pubkey) -> Result<Vec<(Pubkey, Plan)>> {
        // Create filter to match merchant field in Plan account data
        // Plan account layout: 8 bytes discriminator + Plan struct
        // Plan struct: merchant (32 bytes) at offset 8
        let filters = vec![
            RpcFilterType::DataSize(129), // Filter by Plan account size
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                8,
                merchant_address.to_bytes().to_vec(),
            )),
        ];

        let config = RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: None,
                commitment: Some(CommitmentConfig::confirmed()),
                min_context_slot: None,
            },
            with_context: Some(false),
            sort_results: None,
        };

        let plan_accounts = self
            .rpc_client
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| TallyError::Generic(format!("Failed to query plan accounts: {e}")))?;

        let mut plans = Vec::new();
        for (pubkey, account) in plan_accounts {
            if account.data.len() < 8 {
                continue;
            }

            if let Ok(plan) = Plan::try_from_slice(&account.data[8..]) {
                plans.push((pubkey, plan));
            }
            // Skip invalid accounts
        }

        Ok(plans)
    }

    /// List all subscriptions for a plan
    ///
    /// # Errors
    /// Returns an error if the RPC query fails or accounts can't be deserialized
    pub fn list_subscriptions(&self, plan_address: &Pubkey) -> Result<Vec<(Pubkey, Subscription)>> {
        // Create filter to match plan field in Subscription account data
        // Subscription account layout: 8 bytes discriminator + Subscription struct
        // Subscription struct: plan (32 bytes) at offset 8
        let filters = vec![
            RpcFilterType::DataSize(105), // Filter by Subscription account size (8 + 32 + 32 + 8 + 8 + 8 + 8 + 1)
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, plan_address.to_bytes().to_vec())),
        ];

        let config = RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: None,
                commitment: Some(CommitmentConfig::confirmed()),
                min_context_slot: None,
            },
            with_context: Some(false),
            sort_results: None,
        };

        let subscription_accounts = self
            .rpc_client
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| {
                TallyError::Generic(format!("Failed to query subscription accounts: {e}"))
            })?;

        let mut subscriptions = Vec::new();
        for (pubkey, account) in subscription_accounts {
            if account.data.len() < 8 {
                continue;
            }

            if let Ok(subscription) = Subscription::try_from_slice(&account.data[8..]) {
                subscriptions.push((pubkey, subscription));
            }
            // Skip invalid accounts
        }

        Ok(subscriptions)
    }

    /// Submit and confirm a transaction
    ///
    /// # Errors
    /// Returns an error if transaction submission or confirmation fails
    pub fn submit_transaction<T: Signer>(
        &self,
        transaction: &mut Transaction,
        signers: &[&T],
    ) -> Result<String> {
        // Get recent blockhash
        let recent_blockhash = self
            .rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to get recent blockhash: {e}")))?
            .0;

        // Sign transaction
        transaction.sign(signers, recent_blockhash);

        // Submit and confirm transaction
        let signature = self
            .rpc_client
            .send_and_confirm_transaction_with_spinner(transaction)
            .map_err(|e| TallyError::Generic(format!("Transaction failed: {e}")))?;

        Ok(signature.to_string())
    }

    /// Submit instruction with automatic transaction handling
    ///
    /// # Errors
    /// Returns an error if transaction submission or confirmation fails
    pub fn submit_instruction<T: Signer>(
        &self,
        instruction: solana_sdk::instruction::Instruction,
        signers: &[&T],
    ) -> Result<String> {
        let payer = signers.first().ok_or("At least one signer is required")?;
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
        self.submit_transaction(&mut transaction, signers)
    }

    /// High-level method to create a merchant account
    ///
    /// # Errors
    /// Returns an error if merchant creation fails
    pub fn create_merchant<T: Signer>(
        &self,
        authority: &T,
        usdc_mint: &Pubkey,
        treasury_ata: &Pubkey,
        platform_fee_bps: u16,
    ) -> Result<(Pubkey, String)> {
        // Validate parameters
        crate::validation::validate_platform_fee_bps(platform_fee_bps)?;

        // Check if merchant already exists
        let merchant_pda = self.merchant_address(&authority.pubkey());
        if self.account_exists(&merchant_pda)? {
            return Err(TallyError::Generic(format!(
                "Merchant account already exists at address: {merchant_pda}"
            )));
        }

        // Build instruction using transaction builder with this client's program ID
        let instruction = crate::transaction_builder::create_merchant()
            .authority(authority.pubkey())
            .usdc_mint(*usdc_mint)
            .treasury_ata(*treasury_ata)
            .platform_fee_bps(platform_fee_bps)
            .program_id(self.program_id)
            .build_instruction()?;

        let signature = self.submit_instruction(instruction, &[authority])?;

        Ok((merchant_pda, signature))
    }

    /// High-level method to create a subscription plan
    ///
    /// # Errors
    /// Returns an error if plan creation fails
    pub fn create_plan<T: Signer>(
        &self,
        authority: &T,
        plan_args: crate::program_types::CreatePlanArgs,
    ) -> Result<(Pubkey, String)> {
        use crate::transaction_builder::create_plan;

        // Validate plan parameters - ensure values can be safely cast to i64
        let period_i64 = i64::try_from(plan_args.period_secs)
            .map_err(|_| TallyError::Generic("Period seconds too large".to_string()))?;
        let grace_i64 = i64::try_from(plan_args.grace_secs)
            .map_err(|_| TallyError::Generic("Grace seconds too large".to_string()))?;

        crate::validation::validate_plan_parameters(plan_args.price_usdc, period_i64, grace_i64)?;

        // Validate merchant exists
        let merchant_pda = self.merchant_address(&authority.pubkey());
        if !self.account_exists(&merchant_pda)? {
            return Err(TallyError::Generic(format!(
                "Merchant account does not exist at address: {merchant_pda}"
            )));
        }

        // Check if plan already exists
        let plan_pda = crate::pda::plan_address_with_program_id(
            &merchant_pda,
            &plan_args.plan_id_bytes,
            &self.program_id,
        );
        if self.account_exists(&plan_pda)? {
            return Err(TallyError::Generic(format!(
                "Plan already exists at address: {plan_pda}"
            )));
        }

        let instruction = create_plan()
            .authority(authority.pubkey())
            .payer(authority.pubkey())
            .plan_args(plan_args)
            .program_id(self.program_id)
            .build_instruction()?;

        let signature = self.submit_instruction(instruction, &[authority])?;

        Ok((plan_pda, signature))
    }

    /// High-level method to withdraw platform fees
    ///
    /// # Errors
    /// Returns an error if fee withdrawal fails
    pub fn withdraw_platform_fees<T: Signer>(
        &self,
        platform_authority: &T,
        platform_treasury_ata: &Pubkey,
        destination_ata: &Pubkey,
        usdc_mint: &Pubkey,
        amount: u64,
    ) -> Result<String> {
        use crate::transaction_builder::admin_withdraw_fees;

        // Validate withdrawal amount
        crate::validation::validate_withdrawal_amount(amount)?;

        // Validate platform treasury ATA exists and has sufficient balance
        let treasury_info = crate::ata::get_token_account_info(self.rpc(), platform_treasury_ata)?
            .ok_or_else(|| {
                TallyError::Generic(format!(
                    "Platform treasury ATA {platform_treasury_ata} does not exist"
                ))
            })?;

        let (treasury_account, _token_program) = treasury_info;
        if treasury_account.amount < amount {
            // Use integer division to avoid precision loss in error messages
            let has_usdc = treasury_account.amount / 1_000_000;
            let requested_usdc = amount / 1_000_000;
            return Err(TallyError::Generic(format!(
                "Insufficient balance in platform treasury: has {has_usdc} USDC, requested {requested_usdc} USDC"
            )));
        }

        let instruction = admin_withdraw_fees()
            .platform_authority(platform_authority.pubkey())
            .platform_treasury_ata(*platform_treasury_ata)
            .destination_ata(*destination_ata)
            .usdc_mint(*usdc_mint)
            .amount(amount)
            .program_id(self.program_id)
            .build_instruction()?;

        self.submit_instruction(instruction, &[platform_authority])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_client_creation() {
        let client = SimpleTallyClient::new("http://localhost:8899").unwrap();
        assert_eq!(client.program_id().to_string(), program_id_string());
    }
}
