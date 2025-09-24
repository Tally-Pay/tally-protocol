//! Init merchant command implementation

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    get_usdc_mint, load_keypair,
    validate_usdc_token_account, SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use tracing::info;

/// Execute the init merchant command
///
/// # Errors
/// Returns error if merchant initialization fails due to invalid parameters, network issues, or Solana program errors
#[allow(clippy::cognitive_complexity)] // Complex validation logic for merchant initialization
pub async fn execute(
    tally_client: &SimpleTallyClient,
    authority_path: Option<&str>,
    treasury_str: &str,
    fee_bps: u16,
    usdc_mint_str: Option<&str>,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting merchant initialization");

    // Load authority keypair
    let authority = load_keypair(authority_path)?;
    info!("Using authority: {}", authority.pubkey());

    // Parse USDC mint
    let usdc_mint =
        get_usdc_mint(usdc_mint_str).map_err(|e| anyhow!("Failed to parse USDC mint: {}", e))?;
    info!("Using USDC mint: {}", usdc_mint);

    // Parse treasury ATA
    let treasury_ata = Pubkey::from_str(treasury_str)
        .map_err(|e| anyhow!("Invalid treasury ATA address '{}': {}", treasury_str, e))?;
    info!("Using treasury ATA: {}", treasury_ata);

    // Validate treasury ATA using tally-sdk
    let authority_pubkey = Pubkey::from(authority.pubkey().to_bytes());
    validate_usdc_token_account(
        tally_client,
        &treasury_ata,
        &usdc_mint,
        &authority_pubkey,
        "treasury",
    )
    .map_err(|e| anyhow!("Treasury ATA validation failed: {}", e))?;

    // Use tally-sdk's high-level convenience method
    let (merchant_pda, signature) = tally_client
        .create_merchant(&authority, &usdc_mint, &treasury_ata, fee_bps)
        .map_err(|e| anyhow!("Failed to create merchant: {}", e))?;

    info!("Transaction confirmed: {}", signature);

    // Return success message with merchant PDA and transaction signature
    let fee_percentage = config.format_fee_percentage(fee_bps);
    Ok(format!(
        "Merchant initialized successfully!\nMerchant PDA: {}\nTransaction signature: {}\nAuthority: {}\nTreasury ATA: {}\nPlatform fee: {} bps ({:.1}%)",
        merchant_pda,
        signature,
        authority.pubkey(),
        treasury_ata,
        fee_bps,
        fee_percentage
    ))
}
