//! Dashboard commands implementation

use crate::config::TallyCliConfig;
use anyhow::Result;
use clap::ValueEnum;
use tally_sdk::SimpleTallyClient;

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

/// Dashboard command execution - temporarily stubbed for event simulation focus
///
/// # Errors
/// Currently returns OK as functionality is stubbed
pub async fn execute<T: std::fmt::Debug + Send + Sync>(
    _tally_client: &SimpleTallyClient,
    _command: &T,
    _output_format: &OutputFormat,
    _rpc_url: &str,
    _config: &TallyCliConfig,
) -> Result<String> {
    // TODO: Re-implement dashboard functionality
    Ok("Dashboard functionality temporarily disabled".to_string())
}
