//! Init merchant command implementation

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    get_usdc_mint, load_keypair,
    SimpleTallyClient,
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
        get_usdc_mint(usdc_mint_str).map_err(|e| anyhow!("Failed to parse USDC mint: {e}"))?;
    info!("Using USDC mint: {}", usdc_mint);

    // Parse treasury ATA
    let treasury_ata = Pubkey::from_str(treasury_str)
        .map_err(|e| anyhow!("Invalid treasury ATA address '{treasury_str}': {e}"))?;
    info!("Using treasury ATA: {}", treasury_ata);

    // Use the new unified method that handles both ATA existence scenarios
    let (merchant_pda, signature, created_ata) = tally_client
        .initialize_merchant_with_treasury(&authority, &usdc_mint, &treasury_ata, fee_bps)
        .map_err(|e| anyhow!("Failed to initialize merchant: {e}"))?;

    info!(
        "Transaction confirmed: {}, created_ata: {}",
        signature,
        created_ata
    );

    // Return success message with merchant PDA, transaction signature, and ATA creation info
    let fee_percentage = config.format_fee_percentage(fee_bps);
    let ata_message = if created_ata {
        "Treasury ATA created and merchant initialized"
    } else {
        "Merchant initialized with existing treasury ATA"
    };

    Ok(format!(
        "Merchant initialization successful!\n{}\nMerchant PDA: {}\nTransaction signature: {}\nAuthority: {}\nTreasury ATA: {}\nPlatform fee: {} bps ({:.1}%)",
        ata_message,
        merchant_pda,
        signature,
        authority.pubkey(),
        treasury_ata,
        fee_bps,
        fee_percentage
    ))
}
