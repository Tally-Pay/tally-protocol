//! Create plan command implementation

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    load_keypair,
    program_types::CreatePlanArgs,
    SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use tracing::info;

/// Arguments for creating a plan
pub struct CreatePlanRequest<'a> {
    pub merchant_str: &'a str,
    pub plan_id: &'a str,
    pub plan_name: &'a str,
    pub price_usdc: u64,
    pub period_secs: i64,
    pub grace_secs: i64,
    pub authority_path: Option<&'a str>,
}

/// Execute the create plan command
///
/// # Errors
/// Returns error if plan creation fails due to invalid parameters, network issues, or Solana program errors
#[allow(clippy::cognitive_complexity)] // Complex validation logic for plan creation
pub async fn execute(
    tally_client: &SimpleTallyClient,
    request: &CreatePlanRequest<'_>,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting plan creation");

    // Parse merchant PDA address (for validation)
    let expected_merchant_pda = Pubkey::from_str(request.merchant_str).map_err(|e| {
        anyhow!(
            "Invalid merchant PDA address '{}': {}",
            request.merchant_str,
            e
        )
    })?;
    info!("Expected merchant PDA: {expected_merchant_pda}");

    // Load authority keypair
    let authority = load_keypair(request.authority_path)?;
    info!("Using authority: {}", authority.pubkey());

    // Validate authority matches the provided merchant PDA
    let authority_pubkey = Pubkey::from(authority.pubkey().to_bytes());
    let computed_merchant_pda = tally_client.merchant_address(&authority_pubkey);
    if expected_merchant_pda != computed_merchant_pda {
        return Err(anyhow!(
            "Authority mismatch: expected merchant PDA {} for authority {}, but got {}",
            computed_merchant_pda,
            authority.pubkey(),
            expected_merchant_pda
        ));
    }

    // Convert period and grace to u64 (validation happens in tally-sdk)
    let period_secs_u64 = u64::try_from(request.period_secs)
        .map_err(|_| anyhow!("Period must be non-negative, got: {}", request.period_secs))?;
    let grace_secs_u64 = u64::try_from(request.grace_secs).map_err(|_| {
        anyhow!(
            "Grace period must be non-negative, got: {}",
            request.grace_secs
        )
    })?;

    // Create plan arguments
    let plan_id_bytes = {
        let mut bytes = [0u8; 32];
        let id_bytes = request.plan_id.as_bytes();
        let len = id_bytes.len().min(32);
        bytes[..len].copy_from_slice(&id_bytes[..len]);
        bytes
    };

    let plan_args = CreatePlanArgs {
        plan_id: request.plan_id.to_string(),
        plan_id_bytes,
        name: request.plan_name.to_string(),
        price_usdc: request.price_usdc,
        period_secs: period_secs_u64,
        grace_secs: grace_secs_u64,
    };

    // Use tally-sdk's high-level convenience method
    let (plan_pda, signature) = tally_client
        .create_plan(&authority, plan_args)
        .map_err(|e| anyhow!("Failed to create plan: {e}"))?;

    info!("Transaction confirmed: {}", signature);

    // Return success message with plan details
    let price_usdc_display = config.format_usdc(request.price_usdc);
    Ok(format!(
        "Plan created successfully!\nPlan PDA: {plan_pda}\nPlan ID: {}\nName: {}\nPrice: {price_usdc_display:.6} USDC\nPeriod: {period_secs_u64} seconds\nGrace: {grace_secs_u64} seconds\nTransaction signature: {signature}",
        request.plan_id,
        request.plan_name
    ))
}
