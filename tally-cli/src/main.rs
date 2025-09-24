//! Tally CLI - Command-line interface for the Tally subscription platform
//!
//! A comprehensive CLI tool for managing merchants, subscription plans, and subscriptions
//! on the Tally Solana-native subscription platform.

#![forbid(unsafe_code)]

mod commands;
mod config;
mod utils;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::TallyCliConfig;
use tally_sdk::SimpleTallyClient;
use anchor_lang::prelude::Pubkey;

#[derive(Parser, Debug)]
#[command(
    name = "tally-cli",
    version,
    about = "Command-line interface for the Tally subscription platform",
    author = "Tally Team"
)]
struct Cli {
    /// RPC endpoint URL
    #[arg(long)]
    rpc_url: Option<String>,

    /// Output format
    #[arg(long, value_enum)]
    output: Option<OutputFormat>,

    /// Program ID of the subscription program
    #[arg(long)]
    program_id: Option<String>,

    /// USDC mint address
    #[arg(long)]
    usdc_mint: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize global program configuration (required before any operations)
    InitConfig {
        /// Platform authority pubkey for admin operations
        #[arg(long)]
        platform_authority: String,

        /// Maximum platform fee in basis points (e.g., 1000 = 10%)
        #[arg(long, default_value = "1000")]
        max_platform_fee_bps: u16,

        /// Basis points divisor for percentage calculations
        #[arg(long, default_value = "10000")]
        fee_basis_points_divisor: u16,

        /// Minimum subscription period in seconds
        #[arg(long, default_value = "86400")]
        min_period_seconds: u64,

        /// Default allowance periods multiplier
        #[arg(long, default_value = "3")]
        default_allowance_periods: u8,
    },

    /// Initialize a new merchant account
    InitMerchant {
        /// Authority keypair for the merchant
        #[arg(long)]
        authority: Option<String>,

        /// USDC treasury account for the merchant
        #[arg(long)]
        treasury: String,

        /// Fee basis points (e.g., 50 = 0.5%)
        #[arg(long)]
        fee_bps: u16,
    },

    /// Create a new subscription plan
    CreatePlan {
        /// Merchant account address
        #[arg(long)]
        merchant: String,

        /// Plan identifier
        #[arg(long)]
        id: String,

        /// Plan display name
        #[arg(long)]
        name: String,

        /// Price in USDC micro-units (6 decimals)
        #[arg(long)]
        price: u64,

        /// Billing period in seconds
        #[arg(long)]
        period: i64,

        /// Grace period in seconds
        #[arg(long)]
        grace: i64,

        /// Authority keypair for the merchant
        #[arg(long)]
        authority: Option<String>,
    },

    /// List subscription plans for a merchant
    ListPlans {
        /// Merchant account address
        #[arg(long)]
        merchant: String,
    },

    /// List subscriptions for a plan
    ListSubs {
        /// Plan account address
        #[arg(long)]
        plan: String,
    },

    /// Deactivate a subscription plan
    DeactivatePlan {
        /// Plan account address
        #[arg(long)]
        plan: String,

        /// Authority keypair for the merchant
        #[arg(long)]
        authority: Option<String>,
    },

    /// Withdraw accumulated platform fees (admin only)
    WithdrawFees {
        /// Admin authority keypair
        #[arg(long)]
        authority: Option<String>,

        /// Amount to withdraw in USDC micro-units
        #[arg(long)]
        amount: u64,

        /// Destination account for withdrawn fees
        #[arg(long)]
        destination: String,
    },

    /// Dashboard commands for analytics and monitoring
    Dashboard {
        #[command(subcommand)]
        command: DashboardCommands,
    },

    /// Simulate blockchain events for testing the monitoring system
    SimulateEvents {
        /// Merchant account address
        #[arg(long)]
        merchant: String,

        /// Specific plan to generate events for (optional)
        #[arg(long)]
        plan: Option<String>,

        /// Simulation scenario
        #[arg(long, value_enum, default_value = "normal")]
        scenario: commands::simulate_events::SimulationScenario,

        /// Events per minute
        #[arg(long, default_value = "60")]
        rate: u64,

        /// Duration in seconds
        #[arg(long, default_value = "60")]
        duration: u64,

        /// Events per batch
        #[arg(long, default_value = "10")]
        batch_size: usize,

        /// Output format
        #[arg(long, value_enum, default_value = "stdout")]
        output: commands::simulate_events::OutputFormat,

        /// WebSocket URL for WebSocket output
        #[arg(long)]
        websocket_url: Option<String>,

        /// File path for file output
        #[arg(long)]
        output_file: Option<String>,

        /// Random seed for reproducible results
        #[arg(long)]
        seed: Option<u64>,

        /// Custom event distribution (`subscribed,renewed,canceled,payment_failed`)
        /// Example: 0.2,0.7,0.08,0.02
        #[arg(long)]
        event_distribution: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum DashboardCommands {
    /// Display merchant overview statistics
    Overview {
        /// Merchant account address
        #[arg(long)]
        merchant: String,
    },

    /// Show analytics for a specific plan
    Analytics {
        /// Plan account address
        #[arg(long)]
        plan: String,
    },

    /// Monitor real-time events for a merchant
    Events {
        /// Merchant account address
        #[arg(long)]
        merchant: String,

        /// Only show events since this timestamp
        #[arg(long)]
        since: Option<i64>,
    },

    /// List subscriptions for a merchant with enhanced information
    Subscriptions {
        /// Merchant account address
        #[arg(long)]
        merchant: String,

        /// Only show active subscriptions
        #[arg(long)]
        active_only: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = TallyCliConfig::new();

    // Use configuration with CLI overrides
    let rpc_url = cli.rpc_url.as_deref().unwrap_or(&config.default_rpc_url);
    let default_output_format = parse_output_format(&config.default_output_format)?;
    let output_format = cli.output.as_ref().unwrap_or(&default_output_format);

    // Initialize Tally client with optional program ID override
    let tally_client = if let Some(program_id) = &cli.program_id {
        SimpleTallyClient::new_with_program_id(rpc_url, program_id)?
    } else {
        SimpleTallyClient::new(rpc_url)?
    };

    // Execute command
    let result = execute_command(&cli, &tally_client, &config).await;

    // Handle output formatting
    match result {
        Ok(output) => match output_format {
            OutputFormat::Human => println!("{output}"),
            OutputFormat::Json => {
                let json_output = serde_json::json!({
                    "success": true,
                    "data": output
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
        },
        Err(e) => {
            match output_format {
                OutputFormat::Human => eprintln!("Error: {e}"),
                OutputFormat::Json => {
                    let json_output = serde_json::json!({
                        "success": false,
                        "error": e.to_string()
                    });
                    println!("{}", serde_json::to_string_pretty(&json_output)?);
                }
            }
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Parse output format from string
fn parse_output_format(format_str: &str) -> Result<OutputFormat> {
    match format_str.to_lowercase().as_str() {
        "human" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        _ => Err(anyhow::anyhow!("Invalid output format: {format_str}")),
    }
}

#[allow(clippy::too_many_lines)]
async fn execute_command(
    cli: &Cli,
    tally_client: &SimpleTallyClient,
    config: &TallyCliConfig,
) -> Result<String> {
    match &cli.command {
        Commands::InitConfig {
            platform_authority,
            max_platform_fee_bps,
            fee_basis_points_divisor,
            min_period_seconds,
            default_allowance_periods,
        } => {
            commands::execute_init_config(
                tally_client,
                platform_authority,
                *max_platform_fee_bps,
                *fee_basis_points_divisor,
                *min_period_seconds,
                *default_allowance_periods,
                None, // authority_path - using default wallet
                config,
            )
            .await
        }

        Commands::InitMerchant {
            authority,
            treasury,
            fee_bps,
        } => {
            commands::execute_init_merchant(
                tally_client,
                authority.as_deref(),
                treasury,
                *fee_bps,
                cli.usdc_mint.as_deref(),
                config,
            )
            .await
        }

        Commands::CreatePlan {
            merchant,
            id,
            name,
            price,
            period,
            grace,
            authority,
        } => {
            let request = commands::create_plan::CreatePlanRequest {
                merchant_str: merchant,
                plan_id: id,
                plan_name: name,
                price_usdc: *price,
                period_secs: *period,
                grace_secs: *grace,
                authority_path: authority.as_deref(),
            };
            commands::execute_create_plan(tally_client, &request, config).await
        }

        Commands::ListPlans { merchant } => {
            // Use default output format for now - this will be refactored with other commands
            let output_format = commands::list_plans::OutputFormat::Human;
            commands::execute_list_plans(tally_client, merchant, &output_format).await
        }

        Commands::ListSubs { plan } => {
            // Use default output format for now - this will be refactored with other commands
            let output_format = commands::list_subs::OutputFormat::Human;
            commands::execute_list_subs(tally_client, plan, &output_format, config).await
        }

        Commands::DeactivatePlan { plan, authority } => {
            commands::execute_deactivate_plan(tally_client, plan, authority.as_deref()).await
        }

        Commands::WithdrawFees {
            authority,
            amount,
            destination,
        } => {
            commands::execute_withdraw_fees(
                tally_client,
                authority.as_deref(),
                *amount,
                destination,
                cli.usdc_mint.as_deref(),
                config,
            )
            .await
        }

        Commands::Dashboard { command } => {
            // Use default output format for now - this will be refactored with other commands
            let output_format = commands::dashboard::OutputFormat::Human;
            let rpc_url = cli.rpc_url.as_deref().unwrap_or(&config.default_rpc_url);
            commands::execute_dashboard_command(
                tally_client,
                command,
                &output_format,
                rpc_url,
                config,
            )
            .await
        }

        Commands::SimulateEvents {
            merchant,
            plan,
            scenario,
            rate,
            duration,
            batch_size,
            output,
            websocket_url,
            output_file,
            seed,
            event_distribution,
        } => {
            use commands::simulate_events::{EventDistribution, SimulateEventsCommand};
            use std::str::FromStr;

            // Parse merchant pubkey
            let merchant_pubkey = Pubkey::from_str(merchant)
                .map_err(|e| anyhow::anyhow!("Invalid merchant pubkey '{merchant}': {e}"))?;

            // Parse plan pubkey if provided
            let plan_pubkey =
                if let Some(plan_str) = plan {
                    Some(Pubkey::from_str(plan_str).map_err(|e| {
                        anyhow::anyhow!("Invalid plan pubkey '{plan_str}': {e}")
                    })?)
                } else {
                    None
                };

            // Parse custom distribution if provided
            let custom_distribution = if let Some(dist_str) = event_distribution {
                let parts: Vec<&str> = dist_str.split(',').collect();
                if parts.len() != 4 {
                    return Err(anyhow::anyhow!(
                        "Event distribution must have 4 values: subscribed,renewed,canceled,payment_failed"
                    ));
                }

                let subscribed: f32 = parts[0]
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid subscribed percentage: {e}"))?;
                let renewed: f32 = parts[1]
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid renewed percentage: {e}"))?;
                let canceled: f32 = parts[2]
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid canceled percentage: {e}"))?;
                let payment_failed: f32 = parts[3]
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Invalid payment_failed percentage: {e}"))?;

                Some(EventDistribution {
                    subscribed,
                    renewed,
                    canceled,
                    payment_failed,
                })
            } else {
                None
            };

            let command = SimulateEventsCommand {
                merchant: merchant_pubkey,
                plan: plan_pubkey,
                scenario: scenario.clone(),
                custom_distribution,
                rate: *rate,
                duration: *duration,
                batch_size: *batch_size,
                output_format: output.clone(),
                websocket_url: websocket_url.clone(),
                output_file: output_file.clone(),
                seed: *seed,
            };

            commands::execute_simulate_events(tally_client, command, config).await
        }
    }
}
