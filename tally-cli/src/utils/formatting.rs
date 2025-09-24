//! Output formatting utilities for the Tally CLI

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use serde_json;
use std::time::{SystemTime, UNIX_EPOCH};
use anchor_lang::prelude::Pubkey;

/// Plan information for display
#[derive(Debug)]
pub struct PlanInfo {
    pub address: Pubkey,
    pub plan_id: String,
    pub name: String,
    pub price_usdc: f64,
    pub period: String,
    pub grace_secs: u64,
    pub active: bool,
}

/// Subscription information for display
#[derive(Debug)]
pub struct SubscriptionInfo {
    pub address: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub next_renewal_ts: i64,
    pub active: bool,
    pub renewals: u32,
    pub created_ts: i64,
    pub last_amount: u64,
}

/// Format plans for human-readable output
#[must_use]
pub fn format_plans_human(plans: &[PlanInfo], merchant_pda: &Pubkey) -> String {
    use std::fmt::Write;

    if plans.is_empty() {
        return format!("No plans found for merchant: {merchant_pda}");
    }

    let mut output = format!("Plans for merchant: {merchant_pda}\n\n");

    // Use write! to avoid extra allocations
    writeln!(
        &mut output,
        "{:<15} {:<20} {:<12} {:<15} {:<10} {:<8} {:<44}",
        "Plan ID", "Name", "Price (USDC)", "Period", "Grace (s)", "Active", "Address"
    )
    .unwrap();
    output.push_str(&"-".repeat(140));
    output.push('\n');

    for plan in plans {
        writeln!(
            &mut output,
            "{:<15} {:<20} {:<12.6} {:<15} {:<10} {:<8} {}",
            plan.plan_id,
            plan.name,
            plan.price_usdc,
            plan.period,
            plan.grace_secs,
            if plan.active { "Yes" } else { "No" },
            plan.address
        )
        .unwrap();
    }

    write!(&mut output, "\nTotal plans: {}", plans.len()).unwrap();
    output
}

/// Format plans for JSON output
///
/// # Errors
///
/// Returns an error if JSON serialization fails
pub fn format_plans_json(plans: &[PlanInfo]) -> Result<String> {
    let json_plans: Vec<serde_json::Value> = plans
        .iter()
        .map(|plan| {
            serde_json::json!({
                "address": plan.address.to_string(),
                "plan_id": plan.plan_id,
                "name": plan.name,
                "price_usdc": plan.price_usdc,
                "period": plan.period,
                "grace_secs": plan.grace_secs,
                "active": plan.active
            })
        })
        .collect();

    serde_json::to_string_pretty(&json_plans)
        .map_err(|e| anyhow!("Failed to serialize plans to JSON: {e}"))
}

/// Format subscriptions for human-readable output
#[must_use]
pub fn format_subscriptions_human(
    subscriptions: &[SubscriptionInfo],
    plan_pda: &Pubkey,
    config: &TallyCliConfig,
) -> String {
    use std::fmt::Write;

    if subscriptions.is_empty() {
        return format!("No subscriptions found for plan: {plan_pda}");
    }

    let mut output = format!("Subscriptions for plan: {plan_pda}\n\n");
    writeln!(
        &mut output,
        "{:<44} {:<8} {:<9} {:<20} {:<20} {:<12} {:<44}",
        "Subscriber", "Status", "Renewals", "Next Renewal", "Created", "Last Amount", "Address"
    )
    .unwrap();
    output.push_str(&"-".repeat(175));
    output.push('\n');

    for sub in subscriptions {
        let next_renewal = format_timestamp(sub.next_renewal_ts);
        let created = format_timestamp(sub.created_ts);
        // Use config for USDC conversion - format as float for better display
        let last_amount_usdc = config.format_usdc(sub.last_amount);

        writeln!(
            &mut output,
            "{:<44} {:<8} {:<9} {:<20} {:<20} {:<12.6} {}",
            sub.subscriber,
            if sub.active { "Active" } else { "Inactive" },
            sub.renewals,
            next_renewal,
            created,
            last_amount_usdc,
            sub.address
        )
        .unwrap();
    }

    write!(
        &mut output,
        "\nTotal subscriptions: {}",
        subscriptions.len()
    )
    .unwrap();
    output
}

/// Format subscriptions for JSON output
///
/// # Errors
///
/// Returns an error if JSON serialization fails
pub fn format_subscriptions_json(
    subscriptions: &[SubscriptionInfo],
    config: &TallyCliConfig,
) -> Result<String> {
    let json_subscriptions: Vec<serde_json::Value> = subscriptions
        .iter()
        .map(|sub| {
            serde_json::json!({
                "address": sub.address.to_string(),
                "plan": sub.plan.to_string(),
                "subscriber": sub.subscriber.to_string(),
                "next_renewal_ts": sub.next_renewal_ts,
                "next_renewal": format_timestamp(sub.next_renewal_ts),
                "active": sub.active,
                "renewals": sub.renewals,
                "created_ts": sub.created_ts,
                "created": format_timestamp(sub.created_ts),
                "last_amount": sub.last_amount,
                "last_amount_usdc": config.format_usdc(sub.last_amount)
            })
        })
        .collect();

    serde_json::to_string_pretty(&json_subscriptions)
        .map_err(|e| anyhow!("Failed to serialize subscriptions to JSON: {e}"))
}

/// Format unix timestamp to human-readable date
///
/// # Panics
///
/// Panics if the datetime cannot calculate duration since `UNIX_EPOCH`,
/// which should not happen in normal circumstances since we validate
/// the timestamp and use checked arithmetic
#[must_use]
pub fn format_timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "N/A".to_string();
    }

    // Safely convert i64 to u64, handling negative timestamps
    let timestamp_u64 = u64::try_from(timestamp).unwrap_or(0);

    SystemTime::UNIX_EPOCH
        .checked_add(std::time::Duration::from_secs(timestamp_u64))
        .map_or_else(
            || "Invalid".to_string(),
            |datetime| {
                // Simple formatting without external dependencies
                let duration_since_epoch = datetime.duration_since(UNIX_EPOCH).unwrap();
                let secs = duration_since_epoch.as_secs();
                let days = secs / 86400;
                let hours = (secs % 86400) / 3600;
                let minutes = (secs % 3600) / 60;
                let seconds = secs % 60;

                // Calculate approximate date (this is a simplified calculation)
                let years_since_1970 = days / 365;
                let remaining_days = days % 365;
                let months = remaining_days / 30;
                let day_of_month = remaining_days % 30;

                format!(
                    "{}-{:02}-{:02} {:02}:{:02}:{:02}",
                    1970 + years_since_1970,
                    months + 1,
                    day_of_month + 1,
                    hours,
                    minutes,
                    seconds
                )
            },
        )
}
