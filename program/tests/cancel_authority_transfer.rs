//! Integration tests for the `cancel_authority_transfer` instruction (L-9)
//!
//! This test suite validates the L-9 security fix through comprehensive integration tests.
//! For unit tests, see the tests module in `cancel_authority_transfer.rs`.
//!
//! Test coverage:
//! - Cancellation of pending authority transfers by current authority
//! - Security validations (authorization checks, pending transfer requirement)
//! - Edge cases (no pending transfer, unauthorized access)
//! - Integration tests (full lifecycle: initiate → cancel → verify)
//!
//! Security Context (L-9):
//! The critical security fix implements a cancellation mechanism for the two-step
//! authority transfer process. Previously, once a transfer was initiated, only the
//! new authority could clear it by accepting. This created a security risk where
//! an erroneous transfer could lock the system. The implementation:
//! 1. Validates caller is current `platform_authority` via `has_one` constraint
//! 2. Validates a pending transfer exists before allowing cancellation
//! 3. Atomically clears `pending_authority` field to cancel the transfer
//! 4. Emits clear log message for off-chain tracking
//! 5. Prevents unauthorized cancellation attempts
//!
//! The cancellation logic occurs at `cancel_authority_transfer.rs`:
//! ```rust
//! #[derive(Accounts)]
//! pub struct CancelAuthorityTransfer<'info> {
//!     #[account(
//!         mut,
//!         seeds = [b"config"],
//!         bump = config.bump,
//!         has_one = platform_authority @ RecurringPaymentError::Unauthorized
//!     )]
//!     pub config: Account<'info, Config>,
//!
//!     pub platform_authority: Signer<'info>,
//! }
//! ```
//!
//! Security guarantees:
//! 1. Only current platform authority can cancel pending transfers
//! 2. Requires a pending transfer to exist (prevents spurious cancellations)
//! 3. Atomically clears `pending_authority` field
//! 4. PDA derivation ensures correct config account is modified
//! 5. Full auditability through program logs
//!
//! Note: These are integration tests that run with the Anchor BPF runtime.
//! For unit tests, see the tests module in the source file.

#![allow(unexpected_cfgs)]
#![cfg(feature = "test-sbf")]

use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use anchor_spl::token::{Mint, Token, TokenAccount};
use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use subs::errors::RecurringPaymentError;
use subs::state::Config;
use subs::{
    AcceptAuthorityArgs, CancelAuthorityTransferArgs, InitConfigArgs, TransferAuthorityArgs,
};

/// Helper to create InitConfig instruction
fn create_init_config_instruction(
    payer: &Pubkey,
    platform_authority: &Pubkey,
    program_id: &Pubkey,
    upgrade_authority: &Pubkey,
    usdc_mint: &Pubkey,
    platform_treasury_ata: &Pubkey,
    program_data: &Pubkey,
) -> Instruction {
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

    let args = InitConfigArgs {
        max_platform_fee_bps: 1000,
        min_platform_fee_bps: 50,
        min_period_seconds: 86400,
        default_allowance_periods: 3,
        allowed_mint: *usdc_mint,
        max_withdrawal_amount: 1_000_000_000_000,
        max_grace_period_seconds: 604800,
    };

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*platform_authority, false),
            AccountMeta::new_readonly(*upgrade_authority, true),
            AccountMeta::new_readonly(*usdc_mint, false),
            AccountMeta::new_readonly(*platform_treasury_ata, false),
            AccountMeta::new_readonly(*program_data, false),
            AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        ],
        data: subs::instruction::InitConfig { args }.data(),
    }
}

/// Helper to create TransferAuthority instruction
fn create_transfer_authority_instruction(
    platform_authority: &Pubkey,
    new_authority: &Pubkey,
    program_id: &Pubkey,
) -> Instruction {
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

    let args = TransferAuthorityArgs {
        new_authority: *new_authority,
    };

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*platform_authority, true),
        ],
        data: subs::instruction::TransferAuthority { args }.data(),
    }
}

/// Helper to create CancelAuthorityTransfer instruction
fn create_cancel_authority_transfer_instruction(
    platform_authority: &Pubkey,
    program_id: &Pubkey,
) -> Instruction {
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

    let args = CancelAuthorityTransferArgs::default();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*platform_authority, true),
        ],
        data: subs::instruction::CancelAuthorityTransfer { args }.data(),
    }
}

/// Helper to create AcceptAuthority instruction
fn create_accept_authority_instruction(
    new_authority: &Pubkey,
    program_id: &Pubkey,
) -> Instruction {
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], program_id);

    let args = AcceptAuthorityArgs::default();

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(*new_authority, true),
        ],
        data: subs::instruction::AcceptAuthority { args }.data(),
    }
}

/// Test successful cancellation of pending authority transfer
///
/// This test validates the core functionality:
/// 1. Initialize config with platform authority
/// 2. Initiate transfer to new authority
/// 3. Cancel the transfer as current authority
/// 4. Verify pending_authority is cleared
#[tokio::test]
async fn test_cancel_authority_transfer_success() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("subs", program_id, processor!(subs::entry));

    // Create keypairs
    let payer = Keypair::new();
    let platform_authority = Keypair::new();
    let upgrade_authority = Keypair::new();
    let new_authority = Keypair::new();

    // Add accounts
    program_test.add_account(
        payer.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    // Create USDC mint
    let usdc_mint = Keypair::new();
    program_test.add_account(
        usdc_mint.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![0; Mint::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create platform treasury ATA
    let platform_treasury_ata = Keypair::new();
    program_test.add_account(
        platform_treasury_ata.pubkey(),
        solana_sdk::account::Account {
            lamports: 2_039_280,
            data: vec![0; TokenAccount::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create program data account
    let program_data = Keypair::new();
    program_test.add_account(
        program_data.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            ..Default::default()
        },
    );

    let (mut banks_client, payer_keypair, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize config
    let init_ix = create_init_config_instruction(
        &payer.pubkey(),
        &platform_authority.pubkey(),
        &program_id,
        &upgrade_authority.pubkey(),
        &usdc_mint.pubkey(),
        &platform_treasury_ata.pubkey(),
        &program_data.pubkey(),
    );

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &upgrade_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 2: Initiate authority transfer
    let transfer_ix = create_transfer_authority_instruction(
        &platform_authority.pubkey(),
        &new_authority.pubkey(),
        &program_id,
    );

    let mut transaction = Transaction::new_with_payer(&[transfer_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Verify pending_authority is set
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &program_id);
    let config_account = banks_client.get_account(config_pda).await.unwrap().unwrap();
    let config: Config = Config::try_deserialize(&mut &config_account.data[..]).unwrap();
    assert_eq!(config.pending_authority, Some(new_authority.pubkey()));

    // Step 3: Cancel the transfer
    let cancel_ix =
        create_cancel_authority_transfer_instruction(&platform_authority.pubkey(), &program_id);

    let mut transaction = Transaction::new_with_payer(&[cancel_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 4: Verify pending_authority is cleared
    let config_account = banks_client.get_account(config_pda).await.unwrap().unwrap();
    let config: Config = Config::try_deserialize(&mut &config_account.data[..]).unwrap();
    assert_eq!(
        config.pending_authority, None,
        "Pending authority should be cleared after cancellation"
    );
    assert_eq!(
        config.platform_authority,
        platform_authority.pubkey(),
        "Platform authority should remain unchanged"
    );
}

/// Test cancellation fails when no pending transfer exists
///
/// This validates the security constraint that prevents spurious cancellations.
#[tokio::test]
async fn test_cancel_authority_transfer_fails_no_pending_transfer() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("subs", program_id, processor!(subs::entry));

    // Create keypairs
    let payer = Keypair::new();
    let platform_authority = Keypair::new();
    let upgrade_authority = Keypair::new();

    // Add accounts
    program_test.add_account(
        payer.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    // Create USDC mint
    let usdc_mint = Keypair::new();
    program_test.add_account(
        usdc_mint.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![0; Mint::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create platform treasury ATA
    let platform_treasury_ata = Keypair::new();
    program_test.add_account(
        platform_treasury_ata.pubkey(),
        solana_sdk::account::Account {
            lamports: 2_039_280,
            data: vec![0; TokenAccount::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create program data account
    let program_data = Keypair::new();
    program_test.add_account(
        program_data.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            ..Default::default()
        },
    );

    let (mut banks_client, payer_keypair, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize config (no pending transfer)
    let init_ix = create_init_config_instruction(
        &payer.pubkey(),
        &platform_authority.pubkey(),
        &program_id,
        &upgrade_authority.pubkey(),
        &usdc_mint.pubkey(),
        &platform_treasury_ata.pubkey(),
        &program_data.pubkey(),
    );

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &upgrade_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 2: Attempt to cancel (should fail - no pending transfer)
    let cancel_ix =
        create_cancel_authority_transfer_instruction(&platform_authority.pubkey(), &program_id);

    let mut transaction = Transaction::new_with_payer(&[cancel_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should fail when no pending transfer exists");

    // Verify error is NoPendingTransfer
    let error = result.unwrap_err().unwrap();
    match error {
        solana_sdk::transaction::TransactionError::InstructionError(_, inner) => {
            assert!(
                matches!(
                    inner,
                    solana_sdk::instruction::InstructionError::Custom(code)
                    if code == RecurringPaymentError::NoPendingTransfer as u32 + anchor_lang::error::ERROR_CODE_OFFSET
                ),
                "Expected NoPendingTransfer error, got: {:?}",
                inner
            );
        }
        _ => panic!("Expected InstructionError, got: {:?}", error),
    }
}

/// Test cancellation fails when unauthorized signer attempts it
///
/// This validates the security constraint that only the current platform
/// authority can cancel a pending transfer.
#[tokio::test]
async fn test_cancel_authority_transfer_fails_unauthorized() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("subs", program_id, processor!(subs::entry));

    // Create keypairs
    let payer = Keypair::new();
    let platform_authority = Keypair::new();
    let upgrade_authority = Keypair::new();
    let new_authority = Keypair::new();
    let unauthorized = Keypair::new();

    // Add accounts
    program_test.add_account(
        payer.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    program_test.add_account(
        unauthorized.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    // Create USDC mint
    let usdc_mint = Keypair::new();
    program_test.add_account(
        usdc_mint.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![0; Mint::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create platform treasury ATA
    let platform_treasury_ata = Keypair::new();
    program_test.add_account(
        platform_treasury_ata.pubkey(),
        solana_sdk::account::Account {
            lamports: 2_039_280,
            data: vec![0; TokenAccount::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create program data account
    let program_data = Keypair::new();
    program_test.add_account(
        program_data.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            ..Default::default()
        },
    );

    let (mut banks_client, payer_keypair, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize config
    let init_ix = create_init_config_instruction(
        &payer.pubkey(),
        &platform_authority.pubkey(),
        &program_id,
        &upgrade_authority.pubkey(),
        &usdc_mint.pubkey(),
        &platform_treasury_ata.pubkey(),
        &program_data.pubkey(),
    );

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &upgrade_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 2: Initiate authority transfer
    let transfer_ix = create_transfer_authority_instruction(
        &platform_authority.pubkey(),
        &new_authority.pubkey(),
        &program_id,
    );

    let mut transaction = Transaction::new_with_payer(&[transfer_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 3: Attempt to cancel as unauthorized user (should fail)
    let cancel_ix =
        create_cancel_authority_transfer_instruction(&unauthorized.pubkey(), &program_id);

    let mut transaction = Transaction::new_with_payer(&[cancel_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &unauthorized], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_err(),
        "Should fail when unauthorized user attempts cancellation"
    );

    // Verify error is Unauthorized
    let error = result.unwrap_err().unwrap();
    match error {
        solana_sdk::transaction::TransactionError::InstructionError(_, inner) => {
            assert!(
                matches!(
                    inner,
                    solana_sdk::instruction::InstructionError::Custom(code)
                    if code == RecurringPaymentError::Unauthorized as u32 + anchor_lang::error::ERROR_CODE_OFFSET
                ),
                "Expected Unauthorized error, got: {:?}",
                inner
            );
        }
        _ => panic!("Expected InstructionError, got: {:?}", error),
    }
}

/// Test full lifecycle: initiate → cancel → initiate again
///
/// This validates that after cancellation, a new transfer can be initiated.
#[tokio::test]
async fn test_cancel_authority_transfer_allows_new_transfer() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("subs", program_id, processor!(subs::entry));

    // Create keypairs
    let payer = Keypair::new();
    let platform_authority = Keypair::new();
    let upgrade_authority = Keypair::new();
    let new_authority_1 = Keypair::new();
    let new_authority_2 = Keypair::new();

    // Add accounts
    program_test.add_account(
        payer.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    // Create USDC mint
    let usdc_mint = Keypair::new();
    program_test.add_account(
        usdc_mint.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![0; Mint::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create platform treasury ATA
    let platform_treasury_ata = Keypair::new();
    program_test.add_account(
        platform_treasury_ata.pubkey(),
        solana_sdk::account::Account {
            lamports: 2_039_280,
            data: vec![0; TokenAccount::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create program data account
    let program_data = Keypair::new();
    program_test.add_account(
        program_data.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            ..Default::default()
        },
    );

    let (mut banks_client, payer_keypair, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize config
    let init_ix = create_init_config_instruction(
        &payer.pubkey(),
        &platform_authority.pubkey(),
        &program_id,
        &upgrade_authority.pubkey(),
        &usdc_mint.pubkey(),
        &platform_treasury_ata.pubkey(),
        &program_data.pubkey(),
    );

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &upgrade_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 2: Initiate first transfer
    let transfer_ix_1 = create_transfer_authority_instruction(
        &platform_authority.pubkey(),
        &new_authority_1.pubkey(),
        &program_id,
    );

    let mut transaction = Transaction::new_with_payer(&[transfer_ix_1], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 3: Cancel first transfer
    let cancel_ix =
        create_cancel_authority_transfer_instruction(&platform_authority.pubkey(), &program_id);

    let mut transaction = Transaction::new_with_payer(&[cancel_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 4: Initiate second transfer (should succeed)
    let transfer_ix_2 = create_transfer_authority_instruction(
        &platform_authority.pubkey(),
        &new_authority_2.pubkey(),
        &program_id,
    );

    let mut transaction = Transaction::new_with_payer(&[transfer_ix_2], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 5: Verify new pending authority is set
    let (config_pda, _) = Pubkey::find_program_address(&[b"config"], &program_id);
    let config_account = banks_client.get_account(config_pda).await.unwrap().unwrap();
    let config: Config = Config::try_deserialize(&mut &config_account.data[..]).unwrap();
    assert_eq!(
        config.pending_authority,
        Some(new_authority_2.pubkey()),
        "Should allow new transfer after cancellation"
    );
}

/// Test that pending authority cannot cancel (only current authority can)
///
/// This validates that the pending authority has no special privileges
/// until the transfer is accepted.
#[tokio::test]
async fn test_pending_authority_cannot_cancel() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new("subs", program_id, processor!(subs::entry));

    // Create keypairs
    let payer = Keypair::new();
    let platform_authority = Keypair::new();
    let upgrade_authority = Keypair::new();
    let new_authority = Keypair::new();

    // Add accounts
    program_test.add_account(
        payer.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    program_test.add_account(
        new_authority.pubkey(),
        solana_sdk::account::Account {
            lamports: 10_000_000_000,
            ..Default::default()
        },
    );

    // Create USDC mint
    let usdc_mint = Keypair::new();
    program_test.add_account(
        usdc_mint.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            data: vec![0; Mint::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create platform treasury ATA
    let platform_treasury_ata = Keypair::new();
    program_test.add_account(
        platform_treasury_ata.pubkey(),
        solana_sdk::account::Account {
            lamports: 2_039_280,
            data: vec![0; TokenAccount::LEN],
            owner: Token::id(),
            ..Default::default()
        },
    );

    // Create program data account
    let program_data = Keypair::new();
    program_test.add_account(
        program_data.pubkey(),
        solana_sdk::account::Account {
            lamports: 1_000_000_000,
            ..Default::default()
        },
    );

    let (mut banks_client, payer_keypair, recent_blockhash) = program_test.start().await;

    // Step 1: Initialize config
    let init_ix = create_init_config_instruction(
        &payer.pubkey(),
        &platform_authority.pubkey(),
        &program_id,
        &upgrade_authority.pubkey(),
        &usdc_mint.pubkey(),
        &platform_treasury_ata.pubkey(),
        &program_data.pubkey(),
    );

    let mut transaction = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &upgrade_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 2: Initiate transfer
    let transfer_ix = create_transfer_authority_instruction(
        &platform_authority.pubkey(),
        &new_authority.pubkey(),
        &program_id,
    );

    let mut transaction = Transaction::new_with_payer(&[transfer_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &platform_authority], recent_blockhash);
    banks_client.process_transaction(transaction).await.unwrap();

    // Step 3: Attempt cancellation by pending authority (should fail)
    let cancel_ix =
        create_cancel_authority_transfer_instruction(&new_authority.pubkey(), &program_id);

    let mut transaction = Transaction::new_with_payer(&[cancel_ix], Some(&payer.pubkey()));
    transaction.sign(&[&payer_keypair, &new_authority], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_err(),
        "Pending authority should NOT be able to cancel transfer"
    );

    // Verify error is Unauthorized
    let error = result.unwrap_err().unwrap();
    match error {
        solana_sdk::transaction::TransactionError::InstructionError(_, inner) => {
            assert!(
                matches!(
                    inner,
                    solana_sdk::instruction::InstructionError::Custom(code)
                    if code == RecurringPaymentError::Unauthorized as u32 + anchor_lang::error::ERROR_CODE_OFFSET
                ),
                "Expected Unauthorized error, got: {:?}",
                inner
            );
        }
        _ => panic!("Expected InstructionError, got: {:?}", error),
    }
}
