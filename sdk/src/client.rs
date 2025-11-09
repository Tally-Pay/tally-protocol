//! Tally client for interacting with the payment agreement program

use crate::{
    error::{Result, TallyError},
    program_types::*,
    program_id_string,
};
use anchor_client::{
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        signature::Keypair,
    },
    Client, Cluster, Program,
};
use anchor_lang::{prelude::*, Discriminator};
use std::{rc::Rc, str::FromStr};

/// Tally client for program interaction
pub struct TallyClient {
    /// Anchor client
    pub client: Client<Rc<Keypair>>,
    /// Program interface
    pub program: Program<Rc<Keypair>>,
    /// RPC client for direct queries
    pub rpc_client: RpcClient,
}

/// Program account fetchers and parsers
impl TallyClient {
    /// Create a new Tally client
    ///
    /// # Arguments
    /// * `cluster_url` - RPC endpoint URL
    ///
    /// # Returns
    /// * `Ok(TallyClient)` - The client instance
    /// * `Err(TallyError)` - If client creation fails
    pub fn new(cluster_url: String) -> Result<Self> {
        // Create a dummy keypair for the client (it won't be used for signing)
        let payer = Rc::new(Keypair::new());

        // Determine cluster from URL
        let cluster = if cluster_url.contains("devnet") {
            Cluster::Devnet
        } else if cluster_url.contains("testnet") {
            Cluster::Testnet
        } else if cluster_url.contains("mainnet") || cluster_url.contains("api.mainnet-beta") {
            Cluster::Mainnet
        } else {
            Cluster::Localnet
        };

        // Create anchor client
        let client = Client::new_with_options(cluster, payer.clone(), CommitmentConfig::confirmed());

        // Get program ID
        let program_id = Pubkey::from_str(&program_id_string())
            .map_err(|e| TallyError::Generic(format!("Invalid program ID: {e}")))?;

        // Create program interface (simplified without IDL for now)
        let program = client.program(program_id)
            .map_err(|e| TallyError::Generic(format!("Failed to create program interface: {e}")))?;

        // Create RPC client for direct queries
        let rpc_client = RpcClient::new_with_commitment(&cluster_url, CommitmentConfig::confirmed());

        Ok(Self {
            client,
            program,
            rpc_client,
        })
    }

    /// Create a new Tally client with custom payer
    ///
    /// # Arguments
    /// * `cluster_url` - RPC endpoint URL
    /// * `payer` - Keypair to use as payer/signer
    ///
    /// # Returns
    /// * `Ok(TallyClient)` - The client instance
    /// * `Err(TallyError)` - If client creation fails
    pub fn new_with_payer(cluster_url: String, payer: Keypair) -> Result<Self> {
        let payer_rc = Rc::new(payer);

        // Determine cluster from URL
        let cluster = if cluster_url.contains("devnet") {
            Cluster::Devnet
        } else if cluster_url.contains("testnet") {
            Cluster::Testnet
        } else if cluster_url.contains("mainnet") || cluster_url.contains("api.mainnet-beta") {
            Cluster::Mainnet
        } else {
            Cluster::Localnet
        };

        // Create anchor client
        let client = Client::new_with_options(cluster, payer_rc.clone(), CommitmentConfig::confirmed());

        // Get program ID
        let program_id = Pubkey::from_str(&program_id_string())
            .map_err(|e| TallyError::Generic(format!("Invalid program ID: {e}")))?;

        // Create program interface (simplified without IDL for now)
        let program = client.program(program_id)
            .map_err(|e| TallyError::Generic(format!("Failed to create program interface: {e}")))?;

        // Create RPC client for direct queries
        let rpc_client = RpcClient::new_with_commitment(&cluster_url, CommitmentConfig::confirmed());

        Ok(Self {
            client,
            program,
            rpc_client,
        })
    }


    /// Get the program ID
    #[must_use]
    pub fn program_id(&self) -> Pubkey {
        self.program.id()
    }

    /// Get the RPC client
    pub fn rpc(&self) -> &RpcClient {
        &self.rpc_client
    }

    /// Fetch and deserialize a Payee account
    ///
    /// # Arguments
    /// * `address` - The payee account address
    ///
    /// # Returns
    /// * `Ok(Payee)` - The payee account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_payee(&self, address: &Pubkey) -> Result<Payee> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payee account: {e}")))?
            .value
            .ok_or_else(|| TallyError::AccountNotFound(address.to_string()))?;

        // Verify account owner
        if account.owner != self.program_id() {
            return Err(TallyError::Generic(
                "Account not owned by Tally program".to_string(),
            ));
        }

        // Check discriminator
        if account.data.len() < 8 {
            return Err(TallyError::Generic("Account data too short".to_string()));
        }

        let expected_discriminator = Payee::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid payee account discriminator".to_string()));
        }

        // Deserialize account data
        Payee::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payee: {e}")))
    }

    /// Fetch and deserialize a PaymentTerms account
    ///
    /// # Arguments
    /// * `address` - The payment terms account address
    ///
    /// # Returns
    /// * `Ok(PaymentTerms)` - The payment terms account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_payment_terms(&self, address: &Pubkey) -> Result<PaymentTerms> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment terms account: {e}")))?
            .value
            .ok_or_else(|| TallyError::AccountNotFound(address.to_string()))?;

        // Verify account owner
        if account.owner != self.program_id() {
            return Err(TallyError::Generic(
                "Account not owned by Tally program".to_string(),
            ));
        }

        // Check discriminator
        if account.data.len() < 8 {
            return Err(TallyError::Generic("Account data too short".to_string()));
        }

        let expected_discriminator = PaymentTerms::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid payment terms account discriminator".to_string()));
        }

        // Deserialize account data
        PaymentTerms::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payment terms: {e}")))
    }

    /// Fetch and deserialize a PaymentAgreement account
    ///
    /// # Arguments
    /// * `address` - The payment agreement account address
    ///
    /// # Returns
    /// * `Ok(PaymentAgreement)` - The payment agreement account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_payment_agreement(&self, address: &Pubkey) -> Result<PaymentAgreement> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment agreement account: {e}")))?
            .value
            .ok_or_else(|| TallyError::AccountNotFound(address.to_string()))?;

        // Verify account owner
        if account.owner != self.program_id() {
            return Err(TallyError::Generic(
                "Account not owned by Tally program".to_string(),
            ));
        }

        // Check discriminator
        if account.data.len() < 8 {
            return Err(TallyError::Generic("Account data too short".to_string()));
        }

        let expected_discriminator = PaymentAgreement::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid payment agreement account discriminator".to_string()));
        }

        // Deserialize account data
        PaymentAgreement::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize payment agreement: {e}")))
    }

    /// Get all payees (this is a potentially expensive operation)
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, Payee)>)` - List of payee addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_all_payees(&self) -> Result<Vec<(Pubkey, Payee)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payees: {e}")))?;

        let mut payees = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for Payee
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == Payee::DISCRIMINATOR {
                    if let Ok(payee) = Payee::try_deserialize(&mut &account.data[8..]) {
                        payees.push((pubkey, payee));
                    }
                }
            }
        }

        Ok(payees)
    }

    /// Get all payment terms for a specific payee
    ///
    /// # Arguments
    /// * `payee_address` - The payee PDA
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, PaymentTerms)>)` - List of payment terms addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_payee_payment_terms(&self, payee_address: &Pubkey) -> Result<Vec<(Pubkey, PaymentTerms)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment terms: {e}")))?;

        let mut payment_terms = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for PaymentTerms
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == PaymentTerms::DISCRIMINATOR {
                    if let Ok(terms) = PaymentTerms::try_deserialize(&mut &account.data[8..]) {
                        if terms.payee == *payee_address {
                            payment_terms.push((pubkey, terms));
                        }
                    }
                }
            }
        }

        Ok(payment_terms)
    }

    /// Get all payment agreements for specific payment terms
    ///
    /// # Arguments
    /// * `payment_terms_address` - The payment terms PDA
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, PaymentAgreement)>)` - List of payment agreement addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_payment_terms_agreements(&self, payment_terms_address: &Pubkey) -> Result<Vec<(Pubkey, PaymentAgreement)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch payment agreements: {e}")))?;

        let mut agreements = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for PaymentAgreement
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == PaymentAgreement::DISCRIMINATOR {
                    if let Ok(agreement) = PaymentAgreement::try_deserialize(&mut &account.data[8..]) {
                        if agreement.payment_terms == *payment_terms_address {
                            agreements.push((pubkey, agreement));
                        }
                    }
                }
            }
        }

        Ok(agreements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::Keypair;

    #[test]
    fn test_client_creation() {
        let client = TallyClient::new("http://localhost:8899".to_string()).unwrap();
        assert_eq!(client.program_id().to_string(), program_id_string());
    }

    #[test]
    fn test_client_creation_with_different_clusters() {
        // Test devnet detection
        let devnet_client = TallyClient::new("https://api.devnet.solana.com".to_string()).unwrap();
        assert_eq!(devnet_client.program_id().to_string(), program_id_string());

        // Test mainnet detection
        let mainnet_client = TallyClient::new("https://api.mainnet-beta.solana.com".to_string()).unwrap();
        assert_eq!(mainnet_client.program_id().to_string(), program_id_string());

        // Test localnet (default)
        let local_client = TallyClient::new("http://127.0.0.1:8899".to_string()).unwrap();
        assert_eq!(local_client.program_id().to_string(), program_id_string());
    }

    #[test]
    fn test_client_with_custom_payer() {
        let payer = Keypair::new();
        let payer_pubkey = payer.pubkey();

        let client = TallyClient::new_with_payer("http://localhost:8899".to_string(), payer).unwrap();
        assert_eq!(client.program_id().to_string(), program_id_string());
        assert_eq!(client.client.payer().pubkey(), payer_pubkey);
    }

    #[test]
    fn test_client_with_payer() {
        let payer = Keypair::new();
        let client = TallyClient::new_with_payer("http://localhost:8899".to_string(), payer).unwrap();
        assert_eq!(client.program_id().to_string(), program_id_string());
    }

    #[test]
    fn test_program_id() {
        let client = TallyClient::new("http://localhost:8899".to_string()).unwrap();
        let expected = crate::program_id();
        assert_eq!(client.program_id(), expected);
    }
}