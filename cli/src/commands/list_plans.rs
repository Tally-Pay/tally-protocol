//! List plans command implementation

use crate::utils::formatting::{format_plans_human, format_plans_json, PlanInfo};
use anyhow::{anyhow, Result};
use clap::ValueEnum;
use std::str::FromStr;
use tally_sdk::SimpleTallyClient;
use anchor_lang::prelude::Pubkey;
use tracing::info;

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Execute the list plans command
///
/// # Errors
/// Returns error if plan listing fails due to network issues or invalid merchant PDA
#[allow(clippy::cognitive_complexity)] // Complex data processing and formatting
pub async fn execute(
    tally_client: &SimpleTallyClient,
    merchant_str: &str,
    output_format: &OutputFormat,
) -> Result<String> {
    info!("Starting plan listing for merchant: {}", merchant_str);

    // Parse merchant PDA address
    let merchant_pda = Pubkey::from_str(merchant_str)
        .map_err(|e| anyhow!("Invalid merchant PDA address '{merchant_str}': {e}"))?;
    info!("Using merchant PDA: {}", merchant_pda);

    // Validate merchant exists
    if !tally_client.account_exists(&merchant_pda)? {
        return Err(anyhow!(
            "Merchant account does not exist at address: {merchant_pda}"
        ));
    }

    info!("Querying plans using tally-sdk...");

    // Use tally-sdk to get all plans for this merchant
    let plan_accounts = tally_client.list_plans(&merchant_pda)?;

    info!("Found {} plan accounts", plan_accounts.len());

    // Parse and format plan data
    let mut plans = Vec::new();
    for (pubkey, plan) in plan_accounts {
        let plan_info = PlanInfo {
            address: pubkey,
            plan_id: plan.plan_id_str(),
            name: plan.name_str(),
            price_usdc: plan.price_usdc_formatted(),
            period: plan.period_formatted(),
            grace_secs: plan.grace_secs,
            active: plan.active,
        };
        info!("Parsed plan: {} ({})", plan_info.plan_id, plan_info.name);
        plans.push(plan_info);
    }

    // Sort plans by plan_id for consistent output
    plans.sort_by(|a, b| a.plan_id.cmp(&b.plan_id));

    // Format output based on requested format
    match output_format {
        OutputFormat::Human => Ok(format_plans_human(&plans, &merchant_pda)),
        OutputFormat::Json => format_plans_json(&plans),
    }
}
