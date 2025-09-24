//! Withdraw fees command implementation

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    ata::{get_associated_token_address_with_program, TokenProgram},
    get_usdc_mint, load_keypair,
    validate_usdc_token_account, SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use tracing::info;

/// Execute the withdraw fees command
///
/// # Errors
///
/// Returns an error if:
/// - The platform authority keypair cannot be loaded
/// - The destination account is invalid or cannot be parsed
/// - The USDC mint validation fails
/// - The platform treasury account is invalid
/// - The destination account validation fails
/// - The withdrawal transaction fails to be sent or confirmed
#[allow(clippy::cognitive_complexity)] // Complex validation logic for fee withdrawal
pub async fn execute(
    tally_client: &SimpleTallyClient,
    authority_path: Option<&str>,
    amount: u64,
    destination_str: &str,
    usdc_mint_str: Option<&str>,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting platform fee withdrawal");

    // Load platform authority keypair
    let platform_authority = load_keypair(authority_path)?;
    info!("Using platform authority: {}", platform_authority.pubkey());

    // Parse USDC mint using tally-sdk
    let usdc_mint =
        get_usdc_mint(usdc_mint_str).map_err(|e| anyhow!("Failed to parse USDC mint: {e}"))?;
    info!("Using USDC mint: {}", usdc_mint);

    // Parse destination ATA
    let destination_ata = Pubkey::from_str(destination_str).map_err(|e| {
        anyhow!(
            "Invalid destination ATA address '{destination_str}': {e}"
        )
    })?;
    info!("Using destination ATA: {}", destination_ata);

    // Validate destination ATA using tally-sdk
    let platform_authority_pubkey = Pubkey::from(platform_authority.pubkey().to_bytes());
    validate_usdc_token_account(
        tally_client,
        &destination_ata,
        &usdc_mint,
        &platform_authority_pubkey,
        "destination",
    )
    .map_err(|e| anyhow!("Destination ATA validation failed: {e}"))?;

    // Compute platform treasury ATA (this would be the platform's central USDC treasury)
    // NOTE: This assumes the platform treasury ATA is the platform authority's ATA for USDC
    // In a real implementation, this might be a specific PDA or a hardcoded treasury account
    let platform_treasury_ata = get_associated_token_address_with_program(
        &platform_authority_pubkey,
        &usdc_mint,
        TokenProgram::Token,
    )
    .map_err(|e| anyhow!("Failed to compute platform treasury ATA: {e}"))?;
    info!("Using platform treasury ATA: {}", platform_treasury_ata);

    // Use tally-sdk's high-level convenience method
    let signature = tally_client
        .withdraw_platform_fees(
            &platform_authority,
            &platform_treasury_ata,
            &destination_ata,
            &usdc_mint,
            amount,
        )
        .map_err(|e| anyhow!("Failed to withdraw platform fees: {e}"))?;

    info!("Transaction confirmed: {}", signature);

    // Return success message with transaction details
    let amount_usdc = config.format_usdc(amount);
    Ok(format!(
        "Platform fees withdrawn successfully!\nAmount: {amount_usdc:.6} USDC\nDestination ATA: {destination_ata}\nPlatform Authority: {}\nTransaction signature: {signature}",
        platform_authority.pubkey()
    ))
}
