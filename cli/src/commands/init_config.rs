//! Init config command implementation

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    load_keypair,
    program_types::InitConfigArgs,
    transaction_builder::init_config,
    SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use tracing::info;

/// Execute the init config command
///
/// # Errors
/// Returns error if configuration initialization fails due to invalid parameters, network issues, or Solana program errors
#[allow(clippy::cognitive_complexity, clippy::too_many_arguments)]
pub async fn execute(
    tally_client: &SimpleTallyClient,
    platform_authority_str: &str,
    max_platform_fee_bps: u16,
    fee_basis_points_divisor: u16,
    min_period_seconds: u64,
    default_allowance_periods: u8,
    authority_path: Option<&str>,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting global config initialization");

    // Load authority keypair
    let authority = load_keypair(authority_path)?;
    info!("Using authority: {}", authority.pubkey());

    // Parse platform authority
    let platform_authority = Pubkey::from_str(platform_authority_str).map_err(|e| {
        anyhow!(
            "Invalid platform authority address '{platform_authority_str}': {e}"
        )
    })?;
    info!("Using platform authority: {}", platform_authority);

    // Validate parameters
    if max_platform_fee_bps > 10000 {
        return Err(anyhow!(
            "Max platform fee basis points cannot exceed 10000 (100%)"
        ));
    }
    if fee_basis_points_divisor == 0 {
        return Err(anyhow!("Fee basis points divisor cannot be zero"));
    }
    if min_period_seconds < 3600 {
        return Err(anyhow!(
            "Minimum period seconds should be at least 3600 (1 hour)"
        ));
    }
    if default_allowance_periods == 0 {
        return Err(anyhow!("Default allowance periods cannot be zero"));
    }

    // Check if config already exists
    let config_pda = tally_sdk::pda::config_address()?;
    if tally_client.account_exists(&config_pda)? {
        return Err(anyhow!(
            "Config account already exists at address: {config_pda}"
        ));
    }

    // Create config args
    let config_args = InitConfigArgs {
        platform_authority,
        max_platform_fee_bps,
        fee_basis_points_divisor,
        min_period_seconds,
        default_allowance_periods,
    };

    // Build and submit instruction
    let instruction = init_config()
        .authority(authority.pubkey())
        .payer(authority.pubkey())
        .config_args(config_args)
        .build_instruction()?;

    let signature = tally_client.submit_instruction(instruction, &[&authority])?;

    info!("Transaction confirmed: {}", signature);

    // Return success message with config PDA and transaction signature
    let fee_percentage = config.format_fee_percentage(max_platform_fee_bps);
    Ok(format!(
        "Global config initialized successfully!\nConfig PDA: {config_pda}\nTransaction signature: {signature}\nPlatform authority: {platform_authority}\nMax platform fee: {max_platform_fee_bps} bps ({fee_percentage:.1}%)\nFee divisor: {fee_basis_points_divisor}\nMin period: {min_period_seconds} seconds\nDefault allowance periods: {default_allowance_periods}"
    ))
}
