//! Keypair loading utilities for the Tally SDK

use crate::error::{Result, TallyError};
use anchor_client::solana_sdk::signature::{read_keypair_file, Keypair};

/// Load a keypair from file path or use default Solana CLI keypair
///
/// # Arguments
/// * `keypair_path` - Optional path to keypair file
///
/// # Returns
/// * `Ok(Keypair)` - The loaded keypair
/// * `Err(TallyError)` - If loading fails
pub fn load_keypair(keypair_path: Option<&str>) -> Result<Keypair> {
    if let Some(path) = keypair_path {
        read_keypair_file(path)
            .map_err(|e| TallyError::Generic(format!("Failed to load keypair from {path}: {e}")))
    } else {
        // Use default Solana CLI keypair path
        let default_path = dirs::home_dir()
            .ok_or_else(|| TallyError::Generic("Cannot determine home directory".to_string()))?
            .join(".config")
            .join("solana")
            .join("id.json");

        read_keypair_file(&default_path).map_err(|e| {
            TallyError::Generic(format!(
                "Failed to load default keypair from {}: {}. Use --authority to specify a keypair file.",
                default_path.display(),
                e
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::Signer;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_keypair_from_file() {
        let dir = tempdir().unwrap();
        let keypair_path = dir.path().join("test_keypair.json");

        // Create a test keypair file
        let keypair = Keypair::new();
        let keypair_bytes = keypair.to_bytes();
        let keypair_json = format!(
            "[{}]",
            keypair_bytes
                .iter()
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
        fs::write(&keypair_path, keypair_json).unwrap();

        // Test loading the keypair
        let loaded_keypair = load_keypair(Some(keypair_path.to_str().unwrap())).unwrap();
        assert_eq!(loaded_keypair.pubkey(), keypair.pubkey());
    }

    #[test]
    fn test_load_nonexistent_keypair() {
        let result = load_keypair(Some("/nonexistent/path.json"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to load keypair"));
    }
}
