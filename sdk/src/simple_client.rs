//! Simple client for basic Tally SDK operations

use crate::{
    error::{Result, TallyError},
    program_id_string,
    program_types::{Payee, PaymentTerms, PaymentAgreement},
};
use anchor_client::solana_account_decoder::UiAccountEncoding;
use anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::solana_client::rpc_config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcTransactionConfig,
};
use anchor_client::solana_client::rpc_filter::{Memcmp, RpcFilterType};
use anchor_client::solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::{signature::Signer, transaction::Transaction};
use anchor_lang::AnchorDeserialize;
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

    /// Compute payee PDA using this client's program ID
    pub fn payee_address(&self, authority: &Pubkey) -> Pubkey {
        crate::pda::payee_address_with_program_id(authority, &self.program_id)
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

    /// Get payee account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_payee(&self, payee_address: &Pubkey) -> Result<Option<Payee>> {
        let account_data = match self
            .rpc_client
            .get_account_with_commitment(payee_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payee account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic(
                "Invalid payee account data".to_string(),
            ));
        }

        let payee = Payee::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payee: {e}")))?;

        Ok(Some(payee))
    }

    /// Get payment terms account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_payment_terms(&self, payment_terms_address: &Pubkey) -> Result<Option<PaymentTerms>> {
        let account_data = match self
            .rpc_client
            .get_account_with_commitment(payment_terms_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment terms account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic("Invalid payment terms account data".to_string()));
        }

        let payment_terms = PaymentTerms::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payment terms: {e}")))?;

        Ok(Some(payment_terms))
    }

    /// Get config account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_config(&self) -> Result<Option<crate::program_types::Config>> {
        let config_address = crate::pda::config_address_with_program_id(&self.program_id);

        let account_data = match self
            .rpc_client
            .get_account_with_commitment(&config_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch config account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic("Invalid config account data".to_string()));
        }

        let config = crate::program_types::Config::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize config: {e}")))?;

        Ok(Some(config))
    }

    /// Get payment agreement account data
    ///
    /// # Errors
    /// Returns an error if the account doesn't exist or can't be deserialized
    pub fn get_payment_agreement(&self, payment_agreement_address: &Pubkey) -> Result<Option<PaymentAgreement>> {
        let account_data = match self
            .rpc_client
            .get_account_with_commitment(payment_agreement_address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment agreement account: {e}")))?
            .value
        {
            Some(account) => account.data,
            None => return Ok(None),
        };

        if account_data.len() < 8 {
            return Err(TallyError::Generic(
                "Invalid payment agreement account data".to_string(),
            ));
        }

        let payment_agreement = PaymentAgreement::try_from_slice(&account_data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payment agreement: {e}")))?;

        Ok(Some(payment_agreement))
    }

    /// List all payment terms for a payee
    ///
    /// # Errors
    /// Returns an error if the RPC query fails or accounts can't be deserialized
    pub fn list_payment_terms(&self, payee_address: &Pubkey) -> Result<Vec<(Pubkey, PaymentTerms)>> {
        // Create filter to match payee field in PaymentTerms account data
        // PaymentTerms account layout: 8 bytes discriminator + PaymentTerms struct
        // PaymentTerms struct: payee (32 bytes) at offset 8
        let filters = vec![
            RpcFilterType::DataSize(129), // Filter by PaymentTerms account size
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                8,
                payee_address.to_bytes().to_vec(),
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

        let payment_terms_accounts = self
            .rpc_client
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| TallyError::Generic(format!("Failed to query payment terms accounts: {e}")))?;

        let mut payment_terms_list = Vec::new();
        for (pubkey, account) in payment_terms_accounts {
            if account.data.len() < 8 {
                continue;
            }

            if let Ok(payment_terms) = PaymentTerms::try_from_slice(&account.data[8..]) {
                payment_terms_list.push((pubkey, payment_terms));
            }
            // Skip invalid accounts
        }

        Ok(payment_terms_list)
    }

    /// List all payment agreements for payment terms
    ///
    /// # Errors
    /// Returns an error if the RPC query fails or accounts can't be deserialized
    pub fn list_payment_agreements(&self, payment_terms_address: &Pubkey) -> Result<Vec<(Pubkey, PaymentAgreement)>> {
        // Create filter to match payment_terms field in PaymentAgreement account data
        // PaymentAgreement account layout: 8 bytes discriminator + PaymentAgreement struct
        // PaymentAgreement struct: payment_terms (32 bytes) at offset 8
        let filters = vec![
            RpcFilterType::DataSize(105), // Filter by PaymentAgreement account size (8 + 32 + 32 + 8 + 8 + 8 + 8 + 1)
            RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, payment_terms_address.to_bytes().to_vec())),
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

        let payment_agreement_accounts = self
            .rpc_client
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| {
                TallyError::Generic(format!("Failed to query payment agreement accounts: {e}"))
            })?;

        let mut payment_agreements = Vec::new();
        for (pubkey, account) in payment_agreement_accounts {
            if account.data.len() < 8 {
                continue;
            }

            if let Ok(payment_agreement) = PaymentAgreement::try_from_slice(&account.data[8..]) {
                payment_agreements.push((pubkey, payment_agreement));
            }
            // Skip invalid accounts
        }

        Ok(payment_agreements)
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
        instruction: anchor_client::solana_sdk::instruction::Instruction,
        signers: &[&T],
    ) -> Result<String> {
        let payer = signers.first().ok_or("At least one signer is required")?;
        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
        self.submit_transaction(&mut transaction, signers)
    }

    /// Get latest blockhash
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn get_latest_blockhash(&self) -> Result<anchor_client::solana_sdk::hash::Hash> {
        self.rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .map(|(hash, _slot)| hash)
            .map_err(|e| TallyError::Generic(format!("Failed to get latest blockhash: {e}")))
    }

    /// Get latest blockhash with commitment
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn get_latest_blockhash_with_commitment(
        &self,
        commitment: CommitmentConfig,
    ) -> Result<(anchor_client::solana_sdk::hash::Hash, u64)> {
        self.rpc_client
            .get_latest_blockhash_with_commitment(commitment)
            .map_err(|e| TallyError::Generic(format!("Failed to get latest blockhash: {e}")))
    }

    /// High-level method to create a payee account
    ///
    /// # Errors
    /// Returns an error if payee creation fails
    pub fn init_payee<T: Signer>(
        &self,
        authority: &T,
        usdc_mint: &Pubkey,
        treasury_ata: &Pubkey,
    ) -> Result<(Pubkey, String)> {
        // Check if payee already exists
        let payee_pda = self.payee_address(&authority.pubkey());
        if self.account_exists(&payee_pda)? {
            return Err(TallyError::Generic(format!(
                "Payee account already exists at address: {payee_pda}"
            )));
        }

        // Build instruction using transaction builder with this client's program ID
        // Platform fee is automatically set to Free tier (2.0%) by the program
        let instruction = crate::transaction_builder::init_payee()
            .authority(authority.pubkey())
            .usdc_mint(*usdc_mint)
            .treasury_ata(*treasury_ata)
            .program_id(self.program_id)
            .build_instruction()?;

        let signature = self.submit_instruction(instruction, &[authority])?;

        Ok((payee_pda, signature))
    }

    /// High-level method to initialize payee with treasury management
    ///
    /// This method handles both cases:
    /// - Treasury ATA exists + Payee missing → Create payee only
    /// - Treasury ATA missing + Payee missing → Create both ATA and payee
    ///
    /// # Arguments
    /// * `authority` - The wallet that will own the payee account and treasury ATA
    /// * `usdc_mint` - The USDC mint address
    /// * `treasury_ata` - The expected treasury ATA address
    ///
    /// # Returns
    /// * `Ok((payee_pda, signature, created_ata))` - The payee PDA, transaction signature, and whether ATA was created
    /// * `Err(TallyError)` - If payee already exists or other validation/execution failures
    ///
    /// # Errors
    /// Returns an error if payee already exists, validation fails, or transaction execution fails
    pub fn init_payee_with_treasury<T: Signer>(
        &self,
        authority: &T,
        usdc_mint: &Pubkey,
        treasury_ata: &Pubkey,
    ) -> Result<(Pubkey, String, bool)> {
        use anchor_client::solana_sdk::transaction::Transaction;

        // Check if payee already exists
        let payee_pda = self.payee_address(&authority.pubkey());
        if self.account_exists(&payee_pda)? {
            return Err(TallyError::Generic(format!(
                "Payee account already exists at address: {payee_pda}"
            )));
        }

        // Check if treasury ATA exists
        let treasury_exists =
            crate::ata::get_token_account_info(self.rpc(), treasury_ata)?.is_some();

        let mut instructions = Vec::new();
        let created_ata = !treasury_exists;

        // If treasury ATA doesn't exist, add create ATA instruction
        if treasury_exists {
            // Validate existing treasury ATA
            crate::validation::validate_usdc_token_account(
                self,
                treasury_ata,
                usdc_mint,
                &authority.pubkey(),
                "treasury",
            )?;
        } else {
            // Validate the expected ATA address matches computed ATA
            let computed_ata =
                crate::ata::get_associated_token_address_for_mint(&authority.pubkey(), usdc_mint)?;
            if computed_ata != *treasury_ata {
                return Err(TallyError::Generic(format!(
                    "Treasury ATA mismatch: expected {treasury_ata}, computed {computed_ata}"
                )));
            }

            // Detect token program and create ATA instruction
            let token_program = crate::ata::detect_token_program(self.rpc(), usdc_mint)?;
            let create_ata_ix = crate::ata::create_associated_token_account_instruction(
                &authority.pubkey(), // payer
                &authority.pubkey(), // wallet owner
                usdc_mint,
                token_program,
            )?;
            instructions.push(create_ata_ix);
        }

        // Always add the create payee instruction
        // Platform fee is automatically set to Free tier (2.0%) by the program
        let create_payee_ix = crate::transaction_builder::init_payee()
            .authority(authority.pubkey())
            .usdc_mint(*usdc_mint)
            .treasury_ata(*treasury_ata)
            .program_id(self.program_id)
            .build_instruction()?;
        instructions.push(create_payee_ix);

        // Submit transaction with all instructions
        let mut transaction = Transaction::new_with_payer(&instructions, Some(&authority.pubkey()));
        let signature = self.submit_transaction(&mut transaction, &[authority])?;

        Ok((payee_pda, signature, created_ata))
    }

    /// High-level method to create payment terms
    ///
    /// # Errors
    /// Returns an error if payment terms creation fails
    pub fn create_payment_terms<T: Signer>(
        &self,
        authority: &T,
        payment_terms_args: crate::program_types::CreatePaymentTermsArgs,
    ) -> Result<(Pubkey, String)> {
        use crate::transaction_builder::create_payment_terms;

        // Validate payment terms parameters - ensure values can be safely cast to i64
        let period_i64 = i64::try_from(payment_terms_args.period_secs)
            .map_err(|_| TallyError::Generic("Period seconds too large".to_string()))?;

        crate::validation::validate_payment_terms_parameters(payment_terms_args.amount_usdc, period_i64)?;

        // Validate payee exists
        let payee_pda = self.payee_address(&authority.pubkey());
        if !self.account_exists(&payee_pda)? {
            return Err(TallyError::Generic(format!(
                "Payee account does not exist at address: {payee_pda}"
            )));
        }

        // Check if payment terms already exist
        let payment_terms_pda = crate::pda::payment_terms_address_with_program_id(
            &payee_pda,
            &payment_terms_args.terms_id_bytes,
            &self.program_id,
        );
        if self.account_exists(&payment_terms_pda)? {
            return Err(TallyError::Generic(format!(
                "PaymentTerms already exists at address: {payment_terms_pda}"
            )));
        }

        let instruction = create_payment_terms()
            .authority(authority.pubkey())
            .payer(authority.pubkey())
            .payment_terms_args(payment_terms_args)
            .program_id(self.program_id)
            .build_instruction()?;

        let signature = self.submit_instruction(instruction, &[authority])?;

        Ok((payment_terms_pda, signature))
    }

    /// High-level method to withdraw platform fees
    ///
    /// # Errors
    /// Returns an error if fee withdrawal fails
    #[cfg(feature = "platform-admin")]
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

    /// Get confirmed signatures for a program address
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn get_confirmed_signatures_for_address(
        &self,
        address: &Pubkey,
        config: Option<GetConfirmedSignaturesForAddress2Config>,
    ) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
        self.rpc_client
            .get_signatures_for_address_with_config(address, config.unwrap_or_default())
            .map_err(|e| {
                TallyError::Generic(format!(
                    "Failed to get signatures for address {address}: {e}"
                ))
            })
    }

    /// Get transaction details
    ///
    /// # Errors
    /// Returns an error if RPC call fails or transaction not found
    pub fn get_transaction(
        &self,
        signature: &anchor_client::solana_sdk::signature::Signature,
    ) -> Result<serde_json::Value> {
        self.rpc_client
            .get_transaction_with_config(signature, RpcTransactionConfig::default())
            .map(|tx| serde_json::to_value(tx).unwrap_or_default())
            .map_err(|e| TallyError::Generic(format!("Failed to get transaction {signature}: {e}")))
    }

    /// Get multiple transactions in batch
    ///
    /// # Errors
    /// Returns an error if any RPC calls fail
    pub fn get_transactions(
        &self,
        signatures: &[anchor_client::solana_sdk::signature::Signature],
    ) -> Result<Vec<Option<serde_json::Value>>> {
        // Process transactions in chunks to avoid overwhelming the RPC
        const CHUNK_SIZE: usize = 10;
        let mut results = Vec::new();

        for chunk in signatures.chunks(CHUNK_SIZE) {
            for signature in chunk {
                let transaction_result = self
                    .rpc_client
                    .get_transaction_with_config(signature, RpcTransactionConfig::default());
                match transaction_result {
                    Ok(tx) => results.push(Some(serde_json::to_value(tx).unwrap_or_default())),
                    Err(_) => results.push(None), // Transaction not found or other error
                }
            }
        }

        Ok(results)
    }

    /// Submit and confirm a pre-signed transaction
    ///
    /// # Errors
    /// Returns an error if transaction submission or confirmation fails
    pub fn send_and_confirm_transaction(
        &self,
        transaction: &anchor_client::solana_sdk::transaction::VersionedTransaction,
    ) -> Result<anchor_client::solana_sdk::signature::Signature> {
        self.rpc_client
            .send_and_confirm_transaction(transaction)
            .map_err(|e| TallyError::Generic(format!("Transaction submission failed: {e}")))
    }

    /// Get current slot
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn get_slot(&self) -> Result<u64> {
        self.rpc_client
            .get_slot()
            .map_err(|e| TallyError::Generic(format!("Failed to get slot: {e}")))
    }

    /// Get health status
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn get_health(&self) -> Result<()> {
        self.rpc_client
            .get_health()
            .map_err(|e| TallyError::Generic(format!("Health check failed: {e}")))
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
