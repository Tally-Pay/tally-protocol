//! Tally client for interacting with the subscription program

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

    /// Fetch and deserialize a Merchant account
    ///
    /// # Arguments
    /// * `address` - The merchant account address
    ///
    /// # Returns
    /// * `Ok(Merchant)` - The merchant account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_merchant(&self, address: &Pubkey) -> Result<Merchant> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch merchant account: {e}")))?
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

        let expected_discriminator = Merchant::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid merchant account discriminator".to_string()));
        }

        // Deserialize account data
        Merchant::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize merchant: {e}")))
    }

    /// Fetch and deserialize a Plan account
    ///
    /// # Arguments
    /// * `address` - The plan account address
    ///
    /// # Returns
    /// * `Ok(Plan)` - The plan account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_plan(&self, address: &Pubkey) -> Result<Plan> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch plan account: {e}")))?
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

        let expected_discriminator = Plan::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid plan account discriminator".to_string()));
        }

        // Deserialize account data
        Plan::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize plan: {e}")))
    }

    /// Fetch and deserialize a Subscription account
    ///
    /// # Arguments
    /// * `address` - The subscription account address
    ///
    /// # Returns
    /// * `Ok(Subscription)` - The subscription account data
    /// * `Err(TallyError)` - If fetching or parsing fails
    pub async fn fetch_subscription(&self, address: &Pubkey) -> Result<Subscription> {
        let account = self.rpc_client
            .get_account_with_commitment(address, CommitmentConfig::confirmed())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch subscription account: {e}")))?
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

        let expected_discriminator = Subscription::DISCRIMINATOR;
        if &account.data[..8] != expected_discriminator {
            return Err(TallyError::Generic("Invalid subscription account discriminator".to_string()));
        }

        // Deserialize account data
        Subscription::try_deserialize(&mut &account.data[8..])
            .map_err(|e| TallyError::Generic(format!("Failed to deserialize subscription: {e}")))
    }

    /// Get all merchants (this is a potentially expensive operation)
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, Merchant)>)` - List of merchant addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_all_merchants(&self) -> Result<Vec<(Pubkey, Merchant)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch merchants: {e}")))?;

        let mut merchants = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for Merchant
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == Merchant::DISCRIMINATOR {
                    if let Ok(merchant) = Merchant::try_deserialize(&mut &account.data[8..]) {
                        merchants.push((pubkey, merchant));
                    }
                }
            }
        }

        Ok(merchants)
    }

    /// Get all plans for a specific merchant
    ///
    /// # Arguments
    /// * `merchant_address` - The merchant PDA
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, Plan)>)` - List of plan addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_merchant_plans(&self, merchant_address: &Pubkey) -> Result<Vec<(Pubkey, Plan)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch plans: {e}")))?;

        let mut plans = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for Plan
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == Plan::DISCRIMINATOR {
                    if let Ok(plan) = Plan::try_deserialize(&mut &account.data[8..]) {
                        if plan.merchant == *merchant_address {
                            plans.push((pubkey, plan));
                        }
                    }
                }
            }
        }

        Ok(plans)
    }

    /// Get all subscriptions for a specific plan
    ///
    /// # Arguments
    /// * `plan_address` - The plan PDA
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, Subscription)>)` - List of subscription addresses and data
    /// * `Err(TallyError)` - If fetching fails
    pub async fn get_plan_subscriptions(&self, plan_address: &Pubkey) -> Result<Vec<(Pubkey, Subscription)>> {
        let accounts = self.rpc_client
            .get_program_accounts(&self.program_id())
            .map_err(|e| TallyError::Generic(format!("Failed to fetch subscriptions: {e}")))?;

        let mut subscriptions = Vec::new();
        for (pubkey, account) in accounts {
            // Check if account has the correct discriminator for Subscription
            if account.data.len() >= 8 {
                let discriminator = &account.data[..8];
                if discriminator == Subscription::DISCRIMINATOR {
                    if let Ok(subscription) = Subscription::try_deserialize(&mut &account.data[8..]) {
                        if subscription.plan == *plan_address {
                            subscriptions.push((pubkey, subscription));
                        }
                    }
                }
            }
        }

        Ok(subscriptions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::Keypair;

    #[test]
    fn test_load_idl() {
        let idl = TallyClient::load_idl().unwrap();
        assert_eq!(idl.metadata.name, "subs");
    }

    #[test]
    fn test_client_creation() {
        let client = TallyClient::new("http://localhost:8899".to_string()).unwrap();
        assert_eq!(client.program_id().to_string(), program_id_string());
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