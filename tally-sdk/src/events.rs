//! Event parsing utilities for Tally program events and structured receipts

use crate::{error::Result, TallyError};
use anchor_lang::prelude::*;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::TransactionError};
use std::collections::HashMap;

/// Event emitted when a subscription is successfully started
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct Canceled {
    /// The merchant who owns the subscription plan
    pub merchant: Pubkey,
    /// The subscription plan being canceled
    pub plan: Pubkey,
    /// The subscriber's public key
    pub subscriber: Pubkey,
}

/// Event emitted when a subscription payment fails
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
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

/// Compute the 8-byte discriminator for an Anchor event
/// Formula: first 8 bytes of SHA256("event:<EventName>")
fn compute_event_discriminator(event_name: &str) -> [u8; 8] {
    use anchor_lang::solana_program::hash;
    let preimage = format!("event:{event_name}");
    let hash_result = hash::hash(preimage.as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash_result.to_bytes()[..8]);
    discriminator
}

/// Get all event discriminators for fast lookup
fn get_event_discriminators() -> HashMap<[u8; 8], &'static str> {
    let mut discriminators = HashMap::new();
    discriminators.insert(compute_event_discriminator("Subscribed"), "Subscribed");
    discriminators.insert(compute_event_discriminator("Renewed"), "Renewed");
    discriminators.insert(compute_event_discriminator("Canceled"), "Canceled");
    discriminators.insert(
        compute_event_discriminator("PaymentFailed"),
        "PaymentFailed",
    );
    discriminators
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
            let event_data = &log[data_start.saturating_add(program_data_prefix.len())..];
            if let Ok(event) = parse_single_event(event_data) {
                events.push(event);
            }
        }
    }

    Ok(events)
}

/// Parse a single event from base64-encoded data
///
/// Anchor events are encoded as: discriminator (8 bytes) + borsh-serialized event data
/// The discriminator is computed as the first 8 bytes of SHA256("event:<EventName>")
pub fn parse_single_event(data: &str) -> Result<TallyEvent> {
    // Decode base64 data
    let decoded_data = base64::prelude::BASE64_STANDARD
        .decode(data)
        .map_err(|e| TallyError::ParseError(format!("Failed to decode base64: {e}")))?;

    // Check minimum length for discriminator (8 bytes)
    if decoded_data.len() < 8 {
        return Err(TallyError::ParseError(
            "Event data too short, must be at least 8 bytes for discriminator".to_string(),
        ));
    }

    // Extract discriminator (first 8 bytes)
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&decoded_data[..8]);

    // Get event data (remaining bytes after discriminator)
    let event_data = &decoded_data[8..];

    // Determine event type based on discriminator
    let discriminators = get_event_discriminators();
    let event_type = discriminators.get(&discriminator).ok_or_else(|| {
        TallyError::ParseError(format!("Unknown event discriminator: {discriminator:?}"))
    })?;

    // Deserialize the event data using Borsh
    match *event_type {
        "Subscribed" => {
            let event = Subscribed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Subscribed event: {e}"))
            })?;
            Ok(TallyEvent::Subscribed(event))
        }
        "Renewed" => {
            let event = Renewed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Renewed event: {e}"))
            })?;
            Ok(TallyEvent::Renewed(event))
        }
        "Canceled" => {
            let event = Canceled::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize Canceled event: {e}"))
            })?;
            Ok(TallyEvent::Canceled(event))
        }
        "PaymentFailed" => {
            let event = PaymentFailed::try_from_slice(event_data).map_err(|e| {
                TallyError::ParseError(format!("Failed to deserialize PaymentFailed event: {e}"))
            })?;
            Ok(TallyEvent::PaymentFailed(event))
        }
        _ => Err(TallyError::ParseError(format!(
            "Unhandled event type: {event_type}"
        ))),
    }
}

/// Parameters for creating a structured receipt
pub struct ReceiptParams {
    /// Transaction signature
    pub signature: Signature,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction slot
    pub slot: u64,
    /// Whether transaction was successful
    pub success: bool,
    /// Transaction error if any
    pub error: Option<TransactionError>,
    /// Transaction logs
    pub logs: Vec<String>,
    /// Compute units consumed
    pub compute_units_consumed: Option<u64>,
    /// Transaction fee
    pub fee: u64,
    /// Program ID to parse events for
    pub program_id: Pubkey,
}

/// Create a structured receipt from transaction components
///
/// # Arguments
/// * `params` - Receipt creation parameters
///
/// # Returns
/// * `Ok(TallyReceipt)` - Structured receipt
/// * `Err(TallyError)` - If parsing fails
pub fn create_receipt(params: ReceiptParams) -> Result<TallyReceipt> {
    let events = parse_events_from_logs(&params.logs, &params.program_id)?;

    Ok(TallyReceipt {
        signature: params.signature,
        block_time: params.block_time,
        slot: params.slot,
        success: params.success,
        error: params.error.map(|e| format!("{e:?}")),
        events,
        logs: params.logs,
        compute_units_consumed: params.compute_units_consumed,
        fee: params.fee,
    })
}

/// Create a structured receipt from transaction components (legacy compatibility)
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
#[allow(clippy::too_many_arguments)] // Legacy function, will be deprecated
pub fn create_receipt_legacy(
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
    create_receipt(ReceiptParams {
        signature,
        block_time,
        slot,
        success,
        error,
        logs,
        compute_units_consumed,
        fee,
        program_id: *program_id,
    })
}

/// Extract memo from transaction logs
///
/// # Arguments
/// * `logs` - Transaction logs to search
///
/// # Returns
/// * `Option<String>` - Found memo, if any
#[must_use]
pub fn extract_memo_from_logs(logs: &[String]) -> Option<String> {
    for log in logs {
        if log.starts_with("Program log: Memo (len ") {
            // Format: "Program log: Memo (len N): \"message\""
            if let Some(start) = log.find("): \"") {
                let memo_start = start.saturating_add(4); // Skip "): \""
                if let Some(end) = log.rfind('"') {
                    if end > memo_start {
                        return Some(log[memo_start..end].to_string());
                    }
                }
            }
        } else if log.starts_with("Program log: ") && log.contains("memo:") {
            // Alternative memo format
            if let Some(memo_start) = log.find("memo:") {
                let memo_content = &log[memo_start.saturating_add(5)..].trim();
                return Some((*memo_content).to_string());
            }
        }
    }
    None
}

/// Find the first Tally event of a specific type in a receipt
impl TallyReceipt {
    /// Get the first Subscribed event, if any
    #[must_use]
    pub fn get_subscribed_event(&self) -> Option<&Subscribed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Subscribed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Renewed event, if any
    #[must_use]
    pub fn get_renewed_event(&self) -> Option<&Renewed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Renewed(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first Canceled event, if any
    #[must_use]
    pub fn get_canceled_event(&self) -> Option<&Canceled> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::Canceled(e) => Some(e),
            _ => None,
        })
    }

    /// Get the first `PaymentFailed` event, if any
    #[must_use]
    pub fn get_payment_failed_event(&self) -> Option<&PaymentFailed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::PaymentFailed(e) => Some(e),
            _ => None,
        })
    }

    /// Extract memo from transaction logs
    #[must_use]
    pub fn extract_memo(&self) -> Option<String> {
        extract_memo_from_logs(&self.logs)
    }

    /// Check if this receipt represents a successful subscription operation
    #[must_use]
    pub fn is_subscription_success(&self) -> bool {
        self.success
            && (self.get_subscribed_event().is_some()
                || self.get_renewed_event().is_some()
                || self.get_canceled_event().is_some())
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
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units"
                .to_string(),
        ];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Test message".to_string()));
    }

    #[test]
    fn test_extract_memo_alternative_format() {
        let logs = vec!["Program log: Processing memo: Hello world".to_string()];

        let memo = extract_memo_from_logs(&logs);
        assert_eq!(memo, Some("Hello world".to_string()));
    }

    #[test]
    fn test_extract_memo_none() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 consumed 1000 of 200000 compute units"
                .to_string(),
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
            block_time: Some(1_640_995_200), // 2022-01-01
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
            block_time: Some(1_640_995_200),
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

        let receipt = create_receipt(ReceiptParams {
            signature,
            block_time: Some(1_640_995_200),
            slot: 100,
            success: true,
            error: None,
            logs: vec!["Program invoked".to_string()],
            compute_units_consumed: Some(5000),
            fee: 5000,
            program_id,
        })
        .unwrap();

        assert_eq!(receipt.signature, signature);
        assert_eq!(receipt.slot, 100);
        assert!(receipt.success);
        assert_eq!(receipt.error, None);
        assert_eq!(receipt.fee, 5000);
    }

    // Helper function to create base64-encoded event data for testing
    fn create_test_event_data(event_name: &str, event_struct: &impl AnchorSerialize) -> String {
        let discriminator = compute_event_discriminator(event_name);
        let mut event_data = Vec::new();
        event_data.extend_from_slice(&discriminator);
        event_struct.serialize(&mut event_data).unwrap();
        base64::prelude::BASE64_STANDARD.encode(event_data)
    }

    #[test]
    fn test_compute_event_discriminator() {
        let subscribed_disc = compute_event_discriminator("Subscribed");
        let renewed_disc = compute_event_discriminator("Renewed");
        let canceled_disc = compute_event_discriminator("Canceled");
        let payment_failed_disc = compute_event_discriminator("PaymentFailed");

        // All discriminators should be unique
        assert_ne!(subscribed_disc, renewed_disc);
        assert_ne!(subscribed_disc, canceled_disc);
        assert_ne!(subscribed_disc, payment_failed_disc);
        assert_ne!(renewed_disc, canceled_disc);
        assert_ne!(renewed_disc, payment_failed_disc);
        assert_ne!(canceled_disc, payment_failed_disc);

        // Discriminators should be deterministic
        assert_eq!(subscribed_disc, compute_event_discriminator("Subscribed"));
        assert_eq!(renewed_disc, compute_event_discriminator("Renewed"));
    }

    #[test]
    fn test_get_event_discriminators() {
        let discriminators = get_event_discriminators();

        assert_eq!(discriminators.len(), 4);
        assert!(discriminators.contains_key(&compute_event_discriminator("Subscribed")));
        assert!(discriminators.contains_key(&compute_event_discriminator("Renewed")));
        assert!(discriminators.contains_key(&compute_event_discriminator("Canceled")));
        assert!(discriminators.contains_key(&compute_event_discriminator("PaymentFailed")));
    }

    #[test]
    fn test_parse_subscribed_event() {
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 5_000_000, // 5 USDC
        };

        let encoded_data = create_test_event_data("Subscribed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 5_000_000);
            }
            _ => panic!("Expected Subscribed event"),
        }
    }

    #[test]
    fn test_parse_renewed_event() {
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let event = Renewed {
            merchant,
            plan,
            subscriber,
            amount: 10_000_000, // 10 USDC
        };

        let encoded_data = create_test_event_data("Renewed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Renewed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 10_000_000);
            }
            _ => panic!("Expected Renewed event"),
        }
    }

    #[test]
    fn test_parse_canceled_event() {
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let event = Canceled {
            merchant,
            plan,
            subscriber,
        };

        let encoded_data = create_test_event_data("Canceled", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::Canceled(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
            }
            _ => panic!("Expected Canceled event"),
        }
    }

    #[test]
    fn test_parse_payment_failed_event() {
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let event = PaymentFailed {
            merchant,
            plan,
            subscriber,
            reason: "Insufficient funds".to_string(),
        };

        let encoded_data = create_test_event_data("PaymentFailed", &event);
        let parsed_event = parse_single_event(&encoded_data).unwrap();

        match parsed_event {
            TallyEvent::PaymentFailed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.reason, "Insufficient funds");
            }
            _ => panic!("Expected PaymentFailed event"),
        }
    }

    #[test]
    fn test_parse_single_event_invalid_base64() {
        let result = parse_single_event("invalid_base64_!@#$%");
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to decode base64"));
        }
    }

    #[test]
    fn test_parse_single_event_too_short() {
        // Create data with only 6 bytes (less than 8-byte discriminator requirement)
        let short_data = base64::prelude::BASE64_STANDARD.encode(vec![1, 2, 3, 4, 5, 6]);
        let result = parse_single_event(&short_data);

        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Event data too short"));
        }
    }

    #[test]
    fn test_parse_single_event_unknown_discriminator() {
        // Create data with unknown discriminator
        let mut data = vec![0xFF; 8]; // Unknown discriminator
        data.extend_from_slice(&[1, 2, 3, 4]); // Some event data
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Unknown event discriminator"));
        }
    }

    #[test]
    fn test_parse_single_event_malformed_event_data() {
        // Create data with correct discriminator but malformed event data
        let discriminator = compute_event_discriminator("Subscribed");
        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // Malformed data that can't be deserialized as Subscribed
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(result.is_err());
        if let Err(TallyError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to deserialize Subscribed event"));
        }
    }

    #[test]
    fn test_parse_events_from_logs() {
        let program_id = crate::program_id();
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let subscribed_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let canceled_event = Canceled {
            merchant,
            plan,
            subscriber,
        };

        let subscribed_data = create_test_event_data("Subscribed", &subscribed_event);
        let canceled_data = create_test_event_data("Canceled", &canceled_event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", program_id, subscribed_data),
            "Program log: Some other log".to_string(),
            format!("Program data: {} {}", program_id, canceled_data),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        assert_eq!(events.len(), 2);

        // Check first event (Subscribed)
        match &events[0] {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected first event to be Subscribed"),
        }

        // Check second event (Canceled)
        match &events[1] {
            TallyEvent::Canceled(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.plan, plan);
                assert_eq!(parsed.subscriber, subscriber);
            }
            _ => panic!("Expected second event to be Canceled"),
        }
    }

    #[test]
    fn test_parse_events_from_logs_with_malformed_data() {
        let program_id = crate::program_id();
        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let valid_event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let valid_data = create_test_event_data("Subscribed", &valid_event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", program_id, valid_data),
            format!("Program data: {} invalid_base64_!@#$%", program_id), // Malformed data - should be skipped
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        // Only the valid event should be parsed, malformed one should be skipped
        assert_eq!(events.len(), 1);

        match &events[0] {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, merchant);
                assert_eq!(parsed.amount, 1_000_000);
            }
            _ => panic!("Expected event to be Subscribed"),
        }
    }

    #[test]
    fn test_parse_events_from_logs_empty() {
        let program_id = crate::program_id();
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program log: No events here".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_parse_events_from_logs_different_program() {
        let program_id = crate::program_id();
        let other_program_id = Keypair::new().pubkey();

        let merchant = Keypair::new().pubkey();
        let plan = Keypair::new().pubkey();
        let subscriber = Keypair::new().pubkey();

        let event = Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        };

        let event_data = create_test_event_data("Subscribed", &event);

        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            format!("Program data: {} {}", other_program_id, event_data), // Different program
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let events = parse_events_from_logs(&logs, &program_id).unwrap();
        // Should not parse events from different program
        assert_eq!(events.len(), 0);
    }
}
