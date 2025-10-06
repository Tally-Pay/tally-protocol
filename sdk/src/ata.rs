//! Associated Token Account (ATA) computation and token program detection utilities

use crate::{error::Result, TallyError};
use anchor_client::solana_client::rpc_client::RpcClient;
use anchor_client::solana_sdk::commitment_config::CommitmentConfig;
use anchor_client::solana_sdk::pubkey::Pubkey;
use anchor_client::solana_sdk::{account::Account, program_pack::Pack};
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::{Account as TokenAccount, Mint};

/// Token program variants supported by the SDK
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenProgram {
    /// Classic SPL Token program
    Token,
    /// Token Extensions (Token-2022) program
    Token2022,
}

impl TokenProgram {
    /// Get the program ID for this token program variant
    #[must_use]
    pub const fn program_id(&self) -> Pubkey {
        match self {
            Self::Token => spl_token::id(),
            Self::Token2022 => spl_token_2022::id(),
        }
    }
}

/// Get the associated token address for a wallet and mint
///
/// # Arguments
/// * `wallet` - The wallet pubkey
/// * `mint` - The token mint pubkey
///
/// # Returns
/// * `Ok(Pubkey)` - The associated token address
/// * `Err(TallyError)` - If computation fails
pub fn get_associated_token_address_for_mint(wallet: &Pubkey, mint: &Pubkey) -> Result<Pubkey> {
    Ok(get_associated_token_address(wallet, mint))
}

/// Get the associated token address with explicit token program
///
/// # Arguments
/// * `wallet` - The wallet pubkey
/// * `mint` - The token mint pubkey
/// * `token_program` - The token program to use
///
/// # Returns
/// * `Ok(Pubkey)` - The associated token address
/// * `Err(TallyError)` - If computation fails
pub fn get_associated_token_address_with_program(
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: TokenProgram,
) -> Result<Pubkey> {
    Ok(
        spl_associated_token_account::get_associated_token_address_with_program_id(
            wallet,
            mint,
            &token_program.program_id(),
        ),
    )
}

/// Detect the token program used by a mint
///
/// # Arguments
/// * `rpc_client` - RPC client for account queries
/// * `mint` - The mint pubkey to check
///
/// # Returns
/// * `Ok(TokenProgram)` - The detected token program
/// * `Err(TallyError)` - If detection fails
pub fn detect_token_program(rpc_client: &RpcClient, mint: &Pubkey) -> Result<TokenProgram> {
    let account = rpc_client
        .get_account_with_commitment(mint, CommitmentConfig::confirmed())
        .map_err(|e| TallyError::Generic(format!("Failed to fetch mint account: {e}")))?
        .value
        .ok_or_else(|| TallyError::AccountNotFound(mint.to_string()))?;

    // Check the owner to determine token program
    if account.owner == spl_token::id() {
        Ok(TokenProgram::Token)
    } else if account.owner == spl_token_2022::id() {
        Ok(TokenProgram::Token2022)
    } else {
        Err(TallyError::TokenProgramDetectionFailed {
            mint: mint.to_string(),
        })
    }
}

/// Get mint information from SPL Token program
///
/// # Arguments
/// * `account` - The mint account data
///
/// # Returns
/// * `Ok(Mint)` - The parsed mint information
/// * `Err(TallyError)` - If parsing fails
pub fn parse_mint_account(account: &Account) -> Result<Mint> {
    Mint::unpack(&account.data)
        .map_err(|e| TallyError::Generic(format!("Failed to parse SPL Token mint: {e}")))
}

/// Get token account information from SPL Token program
///
/// # Arguments
/// * `account` - The token account data
///
/// # Returns
/// * `Ok(TokenAccount)` - The parsed token account information
/// * `Err(TallyError)` - If parsing fails
pub fn parse_token_account(account: &Account) -> Result<TokenAccount> {
    TokenAccount::unpack(&account.data)
        .map_err(|e| TallyError::Generic(format!("Failed to parse SPL Token account: {e}")))
}

/// Get the correct ATA for a wallet and mint, detecting token program automatically
///
/// # Arguments
/// * `rpc_client` - RPC client for token program detection
/// * `wallet` - The wallet pubkey
/// * `mint` - The token mint pubkey
///
/// # Returns
/// * `Ok((Pubkey, TokenProgram))` - The ATA address and detected token program
/// * `Err(TallyError)` - If detection or computation fails
pub fn get_ata_with_program_detection(
    rpc_client: &RpcClient,
    wallet: &Pubkey,
    mint: &Pubkey,
) -> Result<(Pubkey, TokenProgram)> {
    let token_program = detect_token_program(rpc_client, mint)?;
    let ata = get_associated_token_address_with_program(wallet, mint, token_program)?;
    Ok((ata, token_program))
}

/// Check if a token account exists and get its information
///
/// # Arguments
/// * `rpc_client` - RPC client for account queries
/// * `token_account` - The token account pubkey to check
///
/// # Returns
/// * `Ok(Some((TokenAccount, TokenProgram)))` - If account exists and can be parsed
/// * `Ok(None)` - If account doesn't exist
/// * `Err(TallyError)` - If fetching or parsing fails
pub fn get_token_account_info(
    rpc_client: &RpcClient,
    token_account: &Pubkey,
) -> Result<Option<(TokenAccount, TokenProgram)>> {
    let Some(account) = rpc_client
        .get_account_with_commitment(token_account, CommitmentConfig::confirmed())
        .map_err(|e| TallyError::Generic(format!("Failed to fetch token account: {e}")))?
        .value
    else {
        return Ok(None);
    };

    // Detect token program from account owner
    let token_program = if account.owner == spl_token::id() {
        TokenProgram::Token
    } else if account.owner == spl_token_2022::id() {
        TokenProgram::Token2022
    } else {
        return Err(TallyError::InvalidTokenProgram {
            expected: "SPL Token or Token-2022".to_string(),
            found: account.owner.to_string(),
        });
    };

    // For now, only parse SPL Token accounts
    if token_program == TokenProgram::Token {
        let token_account_data = parse_token_account(&account)?;
        Ok(Some((token_account_data, token_program)))
    } else {
        Err(TallyError::Generic(
            "Token-2022 account parsing not yet implemented".to_string(),
        ))
    }
}

/// Create instruction data for creating an associated token account
///
/// # Arguments
/// * `payer` - The account that will pay for account creation
/// * `wallet` - The wallet that will own the token account
/// * `mint` - The token mint
/// * `token_program` - The token program to use
///
/// # Returns
/// * `Ok(anchor_client::solana_sdk::instruction::Instruction)` - The create ATA instruction
/// * `Err(TallyError)` - If instruction creation fails
pub fn create_associated_token_account_instruction(
    payer: &Pubkey,
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: TokenProgram,
) -> Result<anchor_client::solana_sdk::instruction::Instruction> {
    Ok(
        spl_associated_token_account::instruction::create_associated_token_account(
            payer,
            wallet,
            mint,
            &token_program.program_id(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_client::solana_sdk::signature::{Keypair, Signer};
    use std::str::FromStr;

    #[test]
    fn test_token_program_program_id() {
        assert_eq!(TokenProgram::Token.program_id(), spl_token::id());
        assert_eq!(TokenProgram::Token2022.program_id(), spl_token_2022::id());
    }

    #[test]
    fn test_get_associated_token_address() {
        let wallet = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let mint = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let ata = get_associated_token_address_for_mint(&wallet, &mint).unwrap();

        // Should be deterministic
        let ata2 = get_associated_token_address_for_mint(&wallet, &mint).unwrap();
        assert_eq!(ata, ata2);

        // Different wallets should have different ATAs
        let wallet2 = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let ata3 = get_associated_token_address_for_mint(&wallet2, &mint).unwrap();
        assert_ne!(ata, ata3);
    }

    #[test]
    fn test_get_associated_token_address_with_program() {
        let wallet = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let mint = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let ata_token =
            get_associated_token_address_with_program(&wallet, &mint, TokenProgram::Token).unwrap();
        let ata_token2022 =
            get_associated_token_address_with_program(&wallet, &mint, TokenProgram::Token2022)
                .unwrap();

        // Different token programs should produce different ATAs
        assert_ne!(ata_token, ata_token2022);
    }

    #[test]
    fn test_usdc_mainnet_ata() {
        // Test with real USDC mint on mainnet
        let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap();
        let wallet = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let ata = get_associated_token_address_for_mint(&wallet, &usdc_mint).unwrap();

        // Should be a valid pubkey
        assert!(ata != Pubkey::default());

        // Test with specific token program
        let ata_token =
            get_associated_token_address_with_program(&wallet, &usdc_mint, TokenProgram::Token)
                .unwrap();

        // USDC is on classic token program, so should match
        assert_eq!(ata, ata_token);
    }

    #[test]
    fn test_create_ata_instruction() {
        let payer = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let wallet = Pubkey::from(Keypair::new().pubkey().to_bytes());
        let mint = Pubkey::from(Keypair::new().pubkey().to_bytes());

        let ix_token = create_associated_token_account_instruction(
            &payer,
            &wallet,
            &mint,
            TokenProgram::Token,
        )
        .unwrap();

        let ix_token2022 = create_associated_token_account_instruction(
            &payer,
            &wallet,
            &mint,
            TokenProgram::Token2022,
        )
        .unwrap();

        // Instructions should have same program ID (ATA program)
        assert_eq!(ix_token.program_id, spl_associated_token_account::id());
        assert_eq!(ix_token2022.program_id, spl_associated_token_account::id());

        // The data might be the same since the ATA program handles both token programs
        // but the accounts will include different token program IDs
        assert!(!ix_token.accounts.is_empty());
        assert!(!ix_token2022.accounts.is_empty());
    }
}
