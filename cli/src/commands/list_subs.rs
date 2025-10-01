//! List subscriptions command implementation

use crate::{
    config::TallyCliConfig,
    utils::formatting::{format_subscriptions_human, format_subscriptions_json, SubscriptionInfo},
};
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

/// Execute the list subscriptions command
///
/// # Errors
/// Returns error if subscription listing fails due to network issues or invalid plan PDA
#[allow(clippy::cognitive_complexity)] // Complex data processing and formatting
pub async fn execute(
    tally_client: &SimpleTallyClient,
    plan_str: &str,
    output_format: &OutputFormat,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting subscription listing for plan: {}", plan_str);

    // Parse plan PDA address
    let plan_pda = Pubkey::from_str(plan_str)
        .map_err(|e| anyhow!("Invalid plan PDA address '{plan_str}': {e}"))?;
    info!("Using plan PDA: {}", plan_pda);

    // Validate plan exists
    if !tally_client.account_exists(&plan_pda)? {
        return Err(anyhow!(
            "Plan account does not exist at address: {plan_pda}"
        ));
    }

    info!("Querying subscriptions using tally-sdk...");

    // Use tally-sdk to get all subscriptions for this plan
    let subscription_accounts = tally_client.list_subscriptions(&plan_pda)?;

    info!(
        "Found {} subscription accounts",
        subscription_accounts.len()
    );

    // Parse and format subscription data
    let mut subscriptions = Vec::new();
    for (pubkey, subscription) in subscription_accounts {
        let sub_info = SubscriptionInfo {
            address: pubkey,
            plan: subscription.plan,
            subscriber: subscription.subscriber,
            next_renewal_ts: subscription.next_renewal_ts,
            active: subscription.active,
            renewals: subscription.renewals,
            created_ts: subscription.created_ts,
            last_amount: subscription.last_amount,
        };
        info!(
            "Parsed subscription: {} (subscriber: {})",
            pubkey, subscription.subscriber
        );
        subscriptions.push(sub_info);
    }

    // Sort subscriptions by created timestamp for consistent output
    subscriptions.sort_by(|a, b| a.created_ts.cmp(&b.created_ts));

    // Format output based on requested format
    match output_format {
        OutputFormat::Human => Ok(format_subscriptions_human(
            &subscriptions,
            &plan_pda,
            config,
        )),
        OutputFormat::Json => format_subscriptions_json(&subscriptions, config),
    }
}
