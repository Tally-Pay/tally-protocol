//! Event parsing utilities for Tally program events and structured receipts

use crate::{error::Result, TallyError};
use anchor_lang::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::TransactionError,
};

/// Event emitted when a subscription is successfully started
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscribed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being subscribed to
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for the subscription (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a subscription is successfully renewed
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Renewed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being renewed
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The amount paid for the renewal (in USDC micro-units)
    pub amount: u64,
}

/// Event emitted when a subscription is canceled
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Canceled {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being canceled
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
}

/// Event emitted when a subscription payment fails
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentFailed {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan where payment failed
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
    /// The reason for payment failure (encoded as string for off-chain analysis)
    pub reason: String,
}

/// All possible Tally program events
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TallyEvent {
    /// Subscription started
    Subscribed(Subscribed),
    /// Subscription renewed
    Renewed(Renewed),
    /// Subscription canceled
    Canceled(Canceled),
    /// Payment failed
    PaymentFailed(PaymentFailed),
}

/// Structured receipt for a Tally transaction
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TallyReceipt {
    /// Transaction signature
    pub signature: Signature,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction slot
    pub slot: u64,
    /// Whether the transaction was successful
    pub success: bool,
    /// Transaction error if any
    pub error: Option<String>,
    /// Parsed Tally events from this transaction
    pub events: Vec<TallyEvent>,
    /// Program logs from the transaction
    pub logs: Vec<String>,
    /// Compute units consumed
    pub compute_units_consumed: Option<u64>,
    /// Transaction fee in lamports
    pub fee: u64,
}

/// Parse Tally events from transaction logs
///
/// # Arguments
/// * `logs` - The transaction logs to parse
/// * `program_id` - The Tally program ID to filter events
///
/// # Returns
/// * `Ok(Vec<TallyEvent>)` - Parsed events
/// * `Err(TallyError)` - If parsing fails
pub fn parse_events_from_logs(logs: &[String], program_id: &Pubkey) -> Result<Vec<TallyEvent>> {
    let mut events = Vec::new();
    let program_data_prefix = format!("Program data: {program_id} ");

    for log in logs {
        if let Some(data_start) = log.find(&program_data_prefix) {
            let event_data = &log[data_start + program_data_prefix.len()..];
            if let Ok(event) = parse_single_event(event_data) {
                events.push(event);
            }
        }
    }

    Ok(events)
}

/// Parse a single event from base64-encoded data (simplified version)
fn parse_single_event(data: &str) -> Result<TallyEvent> {
    // For now, we'll create mock events based on data patterns
    // In a real implementation, this would properly decode and deserialize the events
    if data.contains("subscribed") || data.contains("start") {
        Ok(TallyEvent::Subscribed(Subscribed {
            merchant: Pubkey::default(),
            plan: Pubkey::default(),
            subscriber: Pubkey::default(),
            amount: 0,
        }))
    } else if data.contains("renewed") || data.contains("renew") {
        Ok(TallyEvent::Renewed(Renewed {
            merchant: Pubkey::default(),
            plan: Pubkey::default(),
            subscriber: Pubkey::default(),
            amount: 0,
        }))
    } else if data.contains("canceled") || data.contains("cancel") {
        Ok(TallyEvent::Canceled(Canceled {
            merchant: Pubkey::default(),
            plan: Pubkey::default(),
            subscriber: Pubkey::default(),
        }))
    } else if data.contains("failed") || data.contains("error") {
        Ok(TallyEvent::PaymentFailed(PaymentFailed {
            merchant: Pubkey::default(),
            plan: Pubkey::default(),
            subscriber: Pubkey::default(),
            reason: "Unknown error".to_string(),
        }))
    } else {
        Err("Unknown event type".into())
    }
}

/// Create a structured receipt from transaction components
///
/// # Arguments
/// * `signature` - Transaction signature
/// * `block_time` - Block time (Unix timestamp)
/// * `slot` - Transaction slot
/// * `success` - Whether transaction was successful
/// * `error` - Transaction error if any
/// * `logs` - Transaction logs
/// * `compute_units_consumed` - Compute units consumed
/// * `fee` - Transaction fee
/// * `program_id` - Program ID to parse events for
///
/// # Returns
/// * `Ok(TallyReceipt)` - Structured receipt
/// * `Err(TallyError)` - If parsing fails
pub fn create_receipt(
    signature: Signature,
    block_time: Option<i64>,
    slot: u64,
    success: bool,
    error: Option<TransactionError>,
    logs: Vec<String>,
    compute_units_consumed: Option<u64>,
    fee: u64,
    program_id: &Pubkey,
) -> Result<TallyReceipt> {
    let events = parse_events_from_logs(&logs, program_id)?;

    Ok(TallyReceipt {
        signature,
        block_time,
        slot,
        success,
        error: error.map(|e| format!("{e:?}")),
        events,
        logs,
        compute_units_consumed,
        fee,
    })
}

/// Extract memo from transaction logs
///
/// # Arguments
/// * `logs` - Transaction logs to search
///
/// # Returns
/// * `Option<String>` - Found memo, if any
pub fn extract_memo_from_logs(logs: &[String]) -> Option<String> {
    for log in logs {
        if log.starts_with("Program log: Memo (len ") {
            // Format: "Program log: Memo (len N): \"message\""
            if let Some(start) = log.find("): \"") {
                let memo_start = start + 4; // Skip "): \""
                if let Some(end) = log.rfind('"') {
                    if end > memo_start {
                        return Some(log[memo_start..end].to_string());
                    }
                }
            }
        } else if log.starts_with("Program log: ") && log.contains("memo:") {
            // Alternative memo format
            if let Some(memo_start) = log.find("memo:") {
                let memo_content = &log[memo_start + 5..].trim();
                return Some((*memo_content).to_string());
            }
        }
    }
    None
}

/// Find the first Tally event of a specific type in a receipt
impl TallyReceipt {
    /// Get the first Subscribed event, if any
    pub fn get_subscribed_event(&self) -> Option<&Subscribed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Subscribed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Renewed event, if any
    pub fn get_renewed_event(&self) -> Option<&Renewed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Renewed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Canceled event, if any
    pub fn get_canceled_event(&self) -> Option<&Canceled> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Canceled(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first PaymentFailed event, if any
    pub fn get_payment_failed_event(&self) -> Option<&PaymentFailed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentFailed(e) => Some(e),
            _ => None,
        })
    }

    /// Extract memo from transaction logs
    pub fn extract_memo(&self) -> Option<String> {
        extract_memo_from_logs(&self.logs)
    }

    /// Check if this receipt represents a successful subscription operation
    pub fn is_subscription_success(&self) -> bool {
        self.success && (
            self.get_subscribed_event().is_some() ||
            self.get_renewed_event().is_some() ||
            self.get_canceled_event().is_some()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{Keypair, Signer};

    #[test]
    fn test_extract_memo_from_logs() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: Memo (len 12): \"Test message\"".to_string(),
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units".to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Test message".to_string()));
    }

    #[test]
    fn test_extract_memo_alternative_format() {
        let logs = vec![
            "Program log: Processing memo: Hello world".to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_memo_none() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units".to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, None);
    }

    #[test]
    fn test_tally_receipt_event_getters() {
        let signature = Signature::default();
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let subscribed_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000, // 1 USDC
        };

        let receipt = TallyReceipt {
            signature,
            block_time: Some(1640995200), // 2022-01-01
            slot: 100,
            success: true,
            error: None,
            events: vec![TallyEvent::Subscribed(subscribed_event.clone())],
            logs: vec![],
            compute_units_consumed: Some(5000),
            fee: 5000,
        };

        assert_eq!(receipt.get_subscribed_event(), Some(&subscribed_event));
        assert_eq!(receipt.get_renewed_event(), None);
        assert_eq!(receipt.get_canceled_event(), None);
        assert_eq!(receipt.get_payment_failed_event(), None);
        assert!(receipt.is_subscription_success());
    }

    #[test]
    fn test_tally_receipt_failed_transaction() {
        let signature = Signature::default();

        let receipt = TallyReceipt {
            signature,
            block_time: Some(1640995200),
            slot: 100,
            success: false,
            error: Some("InsufficientFunds".to_string()),
            events: vec![],
            logs: vec![],
            compute_units_consumed: Some(1000),
            fee: 5000,
        };

        assert!(!receipt.is_subscription_success());
    }

    #[test]
    fn test_create_receipt() {
        let signature = Signature::default();
        let program_id = crate::program_id();

        let receipt = create_receipt(
            signature,
            Some(1640995200),
            100,
            true,
            None,
            vec!["Program invoked".to_string()],
            Some(5000),
            5000,
            &program_id,
        ).unwrap();

        assert_eq!(receipt.signature, signature);
        assert_eq!(receipt.slot, 100);
        assert!(receipt.success);
        assert_eq!(receipt.error, None);
        assert_eq!(receipt.fee, 5000);
    }
}