//! Dashboard commands implementation

use crate::{config::TallyCliConfig, DashboardCommands};
use anyhow::{anyhow, Result};
use clap::ValueEnum;
use std::str::FromStr;
use tally_sdk::{dashboard::DashboardClient, solana_sdk::pubkey::Pubkey, SimpleTallyClient};
use tracing::info;

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Execute dashboard commands
pub async fn execute(
    _tally_client: &SimpleTallyClient,
    command: &DashboardCommands,
    output_format: &OutputFormat,
    rpc_url: &str,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Executing dashboard command: {:?}", command);

    // Create dashboard client
    let dashboard_client = DashboardClient::new(rpc_url)?;

    match command {
        DashboardCommands::Overview { merchant } => {
            execute_overview(&dashboard_client, merchant, output_format)
        }
        DashboardCommands::Analytics { plan } => {
            execute_analytics(&dashboard_client, plan, output_format)
        }
        DashboardCommands::Events { merchant, since } => {
            execute_events(&dashboard_client, merchant, *since, output_format, config)
        }
        DashboardCommands::Subscriptions {
            merchant,
            active_only,
        } => execute_subscriptions(&dashboard_client, merchant, *active_only, output_format),
    }
}

/// Execute the overview command
fn execute_overview(
    dashboard_client: &DashboardClient,
    merchant_str: &str,
    output_format: &OutputFormat,
) -> Result<String> {
    info!("Starting overview for merchant: {}", merchant_str);

    // Parse merchant PDA address
    let merchant_pda = Pubkey::from_str(merchant_str)
        .map_err(|e| anyhow!("Invalid merchant PDA address '{}': {}", merchant_str, e))?;

    // Validate merchant exists
    if !dashboard_client.merchant_exists(&merchant_pda)? {
        return Err(anyhow!(
            "Merchant account does not exist at address: {}",
            merchant_pda
        ));
    }

    // Get overview data
    let overview = dashboard_client.get_merchant_overview(&merchant_pda)?;

    // Format output
    match output_format {
        OutputFormat::Human => Ok(format_overview_human(&overview, &merchant_pda)),
        OutputFormat::Json => format_overview_json(&overview),
    }
}

/// Execute the analytics command
fn execute_analytics(
    dashboard_client: &DashboardClient,
    plan_str: &str,
    output_format: &OutputFormat,
) -> Result<String> {
    info!("Starting analytics for plan: {}", plan_str);

    // Parse plan PDA address
    let plan_pda = Pubkey::from_str(plan_str)
        .map_err(|e| anyhow!("Invalid plan PDA address '{}': {}", plan_str, e))?;

    // Validate plan exists
    if !dashboard_client.plan_exists(&plan_pda)? {
        return Err(anyhow!(
            "Plan account does not exist at address: {}",
            plan_pda
        ));
    }

    // Get analytics data
    let analytics = dashboard_client.get_plan_analytics(&plan_pda)?;

    // Format output
    match output_format {
        OutputFormat::Human => Ok(format_analytics_human(&analytics)),
        OutputFormat::Json => format_analytics_json(&analytics),
    }
}

/// Execute the events command
fn execute_events(
    dashboard_client: &DashboardClient,
    merchant_str: &str,
    since: Option<i64>,
    output_format: &OutputFormat,
    config: &TallyCliConfig,
) -> Result<String> {
    info!("Starting events monitoring for merchant: {}", merchant_str);

    // Parse merchant PDA address
    let merchant_pda = Pubkey::from_str(merchant_str)
        .map_err(|e| anyhow!("Invalid merchant PDA address '{}': {}", merchant_str, e))?;

    // Validate merchant exists
    if !dashboard_client.merchant_exists(&merchant_pda)? {
        return Err(anyhow!(
            "Merchant account does not exist at address: {}",
            merchant_pda
        ));
    }

    // Get events
    let since_timestamp = since.unwrap_or_else(|| {
        config.default_events_since_timestamp(DashboardClient::current_timestamp())
    });
    let events = dashboard_client.poll_recent_events(&merchant_pda, since_timestamp)?;

    // Format output
    match output_format {
        OutputFormat::Human => Ok(format_events_human(&events, &merchant_pda, since_timestamp)),
        OutputFormat::Json => format_events_json(&events),
    }
}

/// Execute the subscriptions command
fn execute_subscriptions(
    dashboard_client: &DashboardClient,
    merchant_str: &str,
    active_only: bool,
    output_format: &OutputFormat,
) -> Result<String> {
    info!(
        "Starting subscriptions list for merchant: {} (active_only: {})",
        merchant_str, active_only
    );

    // Parse merchant PDA address
    let merchant_pda = Pubkey::from_str(merchant_str)
        .map_err(|e| anyhow!("Invalid merchant PDA address '{}': {}", merchant_str, e))?;

    // Validate merchant exists
    if !dashboard_client.merchant_exists(&merchant_pda)? {
        return Err(anyhow!(
            "Merchant account does not exist at address: {}",
            merchant_pda
        ));
    }

    // Get subscriptions
    let mut subscriptions = dashboard_client.get_live_subscriptions(&merchant_pda)?;

    // Filter if requested
    if active_only {
        subscriptions.retain(|sub| sub.subscription.active);
    }

    // Sort by plan and then by creation date
    subscriptions.sort_by(|a, b| {
        a.plan_address
            .cmp(&b.plan_address)
            .then(b.subscription.created_ts.cmp(&a.subscription.created_ts))
    });

    // Format output
    match output_format {
        OutputFormat::Human => Ok(format_subscriptions_human(&subscriptions, &merchant_pda)),
        OutputFormat::Json => format_subscriptions_json(&subscriptions),
    }
}

// Human-readable formatting functions
fn format_overview_human(
    overview: &tally_sdk::dashboard_types::Overview,
    merchant_pda: &Pubkey,
) -> String {
    use std::fmt::Write;

    let mut output = format!("Merchant Overview ({merchant_pda})\n");
    output.push_str(&"=".repeat(70));
    output.push('\n');

    writeln!(
        &mut output,
        "Total Revenue:     {:>12.2} USDC",
        overview.total_revenue_formatted()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Active Subs:       {:>12}",
        overview.active_subscriptions
    )
    .unwrap();
    writeln!(
        &mut output,
        "Inactive Subs:     {:>12}",
        overview.inactive_subscriptions
    )
    .unwrap();
    writeln!(
        &mut output,
        "Total Plans:       {:>12}",
        overview.total_plans
    )
    .unwrap();
    writeln!(
        &mut output,
        "Monthly Revenue:   {:>12.2} USDC",
        overview.monthly_revenue_formatted()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Monthly New Subs:  {:>12}",
        overview.monthly_new_subscriptions
    )
    .unwrap();
    writeln!(
        &mut output,
        "Churn Rate:        {:>12.1}%",
        overview.churn_rate()
    )
    .unwrap();
    writeln!(
        &mut output,
        "ARPU:              {:>12.2} USDC",
        overview.average_revenue_per_user_formatted()
    )
    .unwrap();

    output
}

fn format_analytics_human(analytics: &tally_sdk::dashboard_types::PlanAnalytics) -> String {
    use std::fmt::Write;

    let mut output = format!("Plan Analytics: {}\n", analytics.plan.name_str());
    output.push_str(&"=".repeat(50));
    output.push('\n');

    writeln!(
        &mut output,
        "Plan ID:           {}",
        analytics.plan.plan_id_str()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Price:             {:.2} USDC",
        analytics.plan.price_usdc_formatted()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Revenue:           {:>12.2} USDC",
        analytics.total_revenue_formatted()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Active Subs:       {:>12}",
        analytics.active_count
    )
    .unwrap();
    writeln!(
        &mut output,
        "Inactive Subs:     {:>12}",
        analytics.inactive_count
    )
    .unwrap();
    writeln!(
        &mut output,
        "Monthly Growth:    {:>12.1}%",
        analytics.monthly_growth_rate()
    )
    .unwrap();
    writeln!(
        &mut output,
        "Avg Duration:      {:>12.1} days",
        analytics.average_duration_days
    )
    .unwrap();
    writeln!(
        &mut output,
        "Churn Rate:        {:>12.1}%",
        analytics.churn_rate()
    )
    .unwrap();

    if let Some(conversion) = analytics.conversion_rate {
        writeln!(&mut output, "Conversion Rate:   {conversion:>12.1}%").unwrap();
    }

    output
}

fn format_events_human(
    events: &[tally_sdk::dashboard_types::DashboardEvent],
    merchant_pda: &Pubkey,
    since_timestamp: i64,
) -> String {
    use std::fmt::Write;

    let mut output = format!("Recent Events for Merchant: {merchant_pda}\n");
    writeln!(
        &mut output,
        "Since: {} (timestamp: {})\n",
        crate::utils::formatting::format_timestamp(since_timestamp),
        since_timestamp
    )
    .unwrap();

    if events.is_empty() {
        output.push_str("No recent events found.\n");
        return output;
    }

    output.push_str(&"-".repeat(120));
    output.push('\n');
    writeln!(
        &mut output,
        "{:<20} {:<12} {:<20} {:<44} {:<10}",
        "Event Type", "Amount", "Timestamp", "Transaction", "Plan"
    )
    .unwrap();
    output.push_str(&"-".repeat(120));
    output.push('\n');

    for event in events {
        let amount_str = event
            .amount_formatted()
            .map_or("N/A".to_string(), |amt| format!("{amt:.2} USDC"));
        let timestamp_str = crate::utils::formatting::format_timestamp(event.timestamp);
        let tx_sig = event
            .transaction_signature
            .as_deref()
            .unwrap_or("N/A")
            .chars()
            .take(40)
            .collect::<String>();
        let plan_str = event.plan_address.map_or("N/A".to_string(), |p| {
            p.to_string().chars().take(8).collect::<String>() + "..."
        });

        writeln!(
            &mut output,
            "{:<20} {:<12} {:<20} {:<44} {:<10}",
            format!("{:?}", event.event_type),
            amount_str,
            timestamp_str,
            tx_sig,
            plan_str
        )
        .unwrap();
    }

    writeln!(&mut output, "\nTotal events: {}", events.len()).unwrap();
    output
}

fn format_subscriptions_human(
    subscriptions: &[tally_sdk::dashboard_types::DashboardSubscription],
    merchant_pda: &Pubkey,
) -> String {
    use std::fmt::Write;

    if subscriptions.is_empty() {
        return format!("No subscriptions found for merchant: {merchant_pda}");
    }

    let mut output = format!("Subscriptions for Merchant: {merchant_pda}\n\n");

    writeln!(
        &mut output,
        "{:<44} {:<20} {:<8} {:<9} {:<15} {:<12} {:<10}",
        "Subscriber", "Plan", "Status", "Renewals", "Next Renewal", "Total Paid", "Days Left"
    )
    .unwrap();
    output.push_str(&"-".repeat(130));
    output.push('\n');

    for sub in subscriptions {
        let next_renewal = if sub.subscription.active {
            crate::utils::formatting::format_timestamp(sub.subscription.next_renewal_ts)
        } else {
            "N/A".to_string()
        };

        let days_left = sub.days_until_renewal.map_or("N/A".to_string(), |days| {
            if days > 0 {
                format!("{days}")
            } else {
                "Overdue".to_string()
            }
        });

        let plan_name = sub.plan.name_str().chars().take(18).collect::<String>();
        let plan_display = if sub.plan.name_str().len() > 18 {
            format!("{plan_name}...")
        } else {
            plan_name
        };

        writeln!(
            &mut output,
            "{:<44} {:<20} {:<8} {:<9} {:<15} {:<12.2} {:<10}",
            sub.subscription.subscriber,
            plan_display,
            format!("{:?}", sub.status),
            sub.subscription.renewals,
            next_renewal,
            sub.total_paid_formatted(),
            days_left
        )
        .unwrap();
    }

    writeln!(
        &mut output,
        "\nTotal subscriptions: {}",
        subscriptions.len()
    )
    .unwrap();
    output
}

// JSON formatting functions
fn format_overview_json(overview: &tally_sdk::dashboard_types::Overview) -> Result<String> {
    serde_json::to_string_pretty(overview)
        .map_err(|e| anyhow!("Failed to serialize overview to JSON: {}", e))
}

fn format_analytics_json(analytics: &tally_sdk::dashboard_types::PlanAnalytics) -> Result<String> {
    serde_json::to_string_pretty(analytics)
        .map_err(|e| anyhow!("Failed to serialize analytics to JSON: {}", e))
}

fn format_events_json(events: &[tally_sdk::dashboard_types::DashboardEvent]) -> Result<String> {
    serde_json::to_string_pretty(events)
        .map_err(|e| anyhow!("Failed to serialize events to JSON: {}", e))
}

fn format_subscriptions_json(
    subscriptions: &[tally_sdk::dashboard_types::DashboardSubscription],
) -> Result<String> {
    serde_json::to_string_pretty(subscriptions)
        .map_err(|e| anyhow!("Failed to serialize subscriptions to JSON: {}", e))
}
