//! Comprehensive integration tests for Tally event parsing
//!
//! This test suite validates the complete event parsing pipeline including:
//! - Event discriminator computation and validation
//! - Borsh deserialization for all 4 event types
//! - Error handling for malformed data
//! - High volume event processing
//! - Base64 encoding/decoding edge cases
//! - Program log parsing and filtering

use anchor_lang::prelude::*;
use base64::prelude::*;
use solana_sdk::signature::{Keypair, Signer};
use anchor_client::solana_sdk::signature::Signature;
use anchor_client::solana_sdk::transaction::TransactionError;
use std::collections::HashMap;
use tally_sdk::{
    events::{
        create_receipt, extract_memo_from_logs, parse_events_from_logs, parse_single_event,
        Canceled, PaymentFailed, ReceiptParams, Renewed, Subscribed, TallyEvent, TallyReceipt,
    },
    TallyError,
};

/// Test fixture for creating realistic event data
struct EventTestFixture {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub program_id: Pubkey,
}

impl EventTestFixture {
    fn new() -> Self {
        Self {
            merchant: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            plan: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            subscriber: Pubkey::from(Keypair::new().pubkey().to_bytes()),
            program_id: tally_sdk::program_id(),
        }
    }

    /// Create base64-encoded event data for testing
    fn create_encoded_event<T: AnchorSerialize>(event_name: &str, event: &T) -> String {
        let discriminator = Self::compute_discriminator(event_name);
        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        event.serialize(&mut data).unwrap();
        BASE64_STANDARD.encode(data)
    }

    /// Compute discriminator for event (matches implementation in events.rs)
    fn compute_discriminator(event_name: &str) -> [u8; 8] {
        use anchor_lang::solana_program::hash;
        let preimage = format!("event:{event_name}");
        let hash_result = hash::hash(preimage.as_bytes());
        let mut discriminator = [0u8; 8];
        discriminator.copy_from_slice(&hash_result.to_bytes()[..8]);
        discriminator
    }

    /// Create a Subscribed event
    const fn create_subscribed_event(&self, amount: u64) -> Subscribed {
        Subscribed {
            merchant: self.merchant,
            plan: self.plan,
            subscriber: self.subscriber,
            amount,
        }
    }

    /// Create a Renewed event
    const fn create_renewed_event(&self, amount: u64) -> Renewed {
        Renewed {
            merchant: self.merchant,
            plan: self.plan,
            subscriber: self.subscriber,
            amount,
        }
    }

    /// Create a Canceled event
    const fn create_canceled_event(&self) -> Canceled {
        Canceled {
            merchant: self.merchant,
            plan: self.plan,
            subscriber: self.subscriber,
        }
    }

    /// Create a `PaymentFailed` event
    const fn create_payment_failed_event(&self, reason: String) -> PaymentFailed {
        PaymentFailed {
            merchant: self.merchant,
            plan: self.plan,
            subscriber: self.subscriber,
            reason,
        }
    }

    /// Create realistic program logs with multiple events
    fn create_program_logs(&self, events: Vec<(&str, String)>) -> Vec<String> {
        let mut logs = vec![
            format!("Program {} invoke [1]", self.program_id),
            "Program log: Instruction: ProcessSubscription".to_string(),
        ];

        for (event_name, event_data) in events {
            logs.push(format!("Program data: {} {}", self.program_id, event_data));
            logs.push(format!("Program log: Event emitted: {event_name}"));
        }

        logs.push(format!(
            "Program {} consumed 15000 of 200000 compute units",
            self.program_id
        ));
        logs.push(format!("Program {} success", self.program_id));
        logs
    }
}

#[tokio::test]
async fn test_subscribed_event_parsing_comprehensive() {
    let fixture = EventTestFixture::new();

    // Test various amounts including edge cases
    let test_amounts = vec![
        0,           // Zero amount
        1,           // Minimum amount
        1_000_000,   // 1 USDC
        5_000_000,   // 5 USDC (typical subscription)
        100_000_000, // 100 USDC (high-value subscription)
        u64::MAX,    // Maximum possible amount
    ];

    for amount in test_amounts {
        let event = fixture.create_subscribed_event(amount);
        let encoded_data = EventTestFixture::create_encoded_event("Subscribed", &event);

        // Parse the event
        let result = parse_single_event(&encoded_data);
        assert!(
            result.is_ok(),
            "Failed to parse Subscribed event with amount {amount}"
        );

        match result.unwrap() {
            TallyEvent::Subscribed(parsed) => {
                assert_eq!(parsed.merchant, fixture.merchant);
                assert_eq!(parsed.plan, fixture.plan);
                assert_eq!(parsed.subscriber, fixture.subscriber);
                assert_eq!(parsed.amount, amount);
            }
            _ => panic!("Expected Subscribed event"),
        }
    }
}

#[tokio::test]
async fn test_renewed_event_parsing_comprehensive() {
    let fixture = EventTestFixture::new();

    // Test multiple renewal scenarios
    let renewal_scenarios = vec![
        (1_000_000, "Monthly renewal"),
        (5_000_000, "Premium monthly renewal"),
        (12_000_000, "Annual renewal"),
        (100_000_000, "Enterprise annual renewal"),
    ];

    for (amount, description) in renewal_scenarios {
        let event = fixture.create_renewed_event(amount);
        let encoded_data = EventTestFixture::create_encoded_event("Renewed", &event);

        let result = parse_single_event(&encoded_data);
        assert!(
            result.is_ok(),
            "Failed to parse Renewed event: {description}"
        );

        match result.unwrap() {
            TallyEvent::Renewed(parsed) => {
                assert_eq!(parsed.merchant, fixture.merchant);
                assert_eq!(parsed.plan, fixture.plan);
                assert_eq!(parsed.subscriber, fixture.subscriber);
                assert_eq!(parsed.amount, amount);
            }
            _ => panic!("Expected Renewed event for {description}"),
        }
    }
}

#[tokio::test]
async fn test_canceled_event_parsing() {
    let fixture = EventTestFixture::new();
    let event = fixture.create_canceled_event();
    let encoded_data = EventTestFixture::create_encoded_event("Canceled", &event);

    let result = parse_single_event(&encoded_data);
    assert!(result.is_ok());

    match result.unwrap() {
        TallyEvent::Canceled(parsed) => {
            assert_eq!(parsed.merchant, fixture.merchant);
            assert_eq!(parsed.plan, fixture.plan);
            assert_eq!(parsed.subscriber, fixture.subscriber);
        }
        _ => panic!("Expected Canceled event"),
    }
}

#[tokio::test]
async fn test_payment_failed_event_parsing_comprehensive() {
    let fixture = EventTestFixture::new();

    // Test various failure reasons including edge cases
    let failure_reasons = vec![
        "Insufficient funds".to_string(),
        "Account frozen".to_string(),
        "Invalid signature".to_string(),
        "Token account not found".to_string(),
        "Delegate approval revoked".to_string(),
        "Program account closed".to_string(),
        // Test long error message
        "A".repeat(1000),
        // Test unicode characters
        "Payment failed: 支付失败 🚫💳".to_string(),
        // Test empty reason
        String::new(),
    ];

    for reason in failure_reasons {
        let event = fixture.create_payment_failed_event(reason.clone());
        let encoded_data = EventTestFixture::create_encoded_event("PaymentFailed", &event);

        let result = parse_single_event(&encoded_data);
        assert!(
            result.is_ok(),
            "Failed to parse PaymentFailed event with reason: {reason}"
        );

        match result.unwrap() {
            TallyEvent::PaymentFailed(parsed) => {
                assert_eq!(parsed.merchant, fixture.merchant);
                assert_eq!(parsed.plan, fixture.plan);
                assert_eq!(parsed.subscriber, fixture.subscriber);
                assert_eq!(parsed.reason, reason);
            }
            _ => panic!("Expected PaymentFailed event"),
        }
    }
}

#[tokio::test]
async fn test_malformed_event_data_handling() {
    // Test various malformed data scenarios
    let short_4_bytes = BASE64_STANDARD.encode(vec![0u8; 4]);
    let short_7_bytes = BASE64_STANDARD.encode(vec![0u8; 7]);
    let unknown_discriminator = BASE64_STANDARD.encode(vec![0xFF; 8]);

    let malformed_cases = vec![
        ("", "Empty string"),
        ("invalid_base64_!@#$", "Invalid base64"),
        (
            "SGVsbG8gV29ybGQ=",
            "Valid base64 but too short for discriminator",
        ),
        (&short_4_bytes, "Less than 8 bytes"),
        (&short_7_bytes, "Exactly 7 bytes"),
        (
            &unknown_discriminator,
            "Valid length but unknown discriminator",
        ),
    ];

    for (malformed_data, description) in malformed_cases {
        let result = parse_single_event(malformed_data);
        assert!(
            result.is_err(),
            "Expected error for malformed data: {description}"
        );

        // Verify error type
        match result.unwrap_err() {
            TallyError::ParseError(msg) => {
                assert!(
                    !msg.is_empty(),
                    "Error message should not be empty for: {description}"
                );
            }
            _ => panic!("Expected ParseError for: {description}"),
        }
    }
}

#[tokio::test]
async fn test_corrupted_event_serialization() {
    let _fixture = EventTestFixture::new();

    // Create valid discriminator but corrupted serialized data
    let discriminator = EventTestFixture::compute_discriminator("Subscribed");
    let corrupted_cases = vec![
        (vec![], "No event data after discriminator"),
        (vec![0xFF], "Single corrupted byte"),
        (vec![0xFF, 0xFF, 0xFF], "Multiple corrupted bytes"),
        (
            vec![1, 2, 3, 4, 5],
            "Invalid data that can't be deserialized as Subscribed",
        ),
    ];

    for (corrupted_data, description) in corrupted_cases {
        let mut data = Vec::new();
        data.extend_from_slice(&discriminator);
        data.extend_from_slice(&corrupted_data);
        let encoded_data = BASE64_STANDARD.encode(data);

        let result = parse_single_event(&encoded_data);
        assert!(
            result.is_err(),
            "Expected error for corrupted data: {description}"
        );

        match result.unwrap_err() {
            TallyError::ParseError(msg) => {
                assert!(
                    msg.contains("Failed to deserialize"),
                    "Error should mention deserialization failure for: {description}"
                );
            }
            _ => panic!("Expected ParseError for corrupted data: {description}"),
        }
    }
}

#[tokio::test]
async fn test_multiple_events_in_logs() {
    let fixture = EventTestFixture::new();

    // Create multiple events of different types
    let subscribed = fixture.create_subscribed_event(5_000_000);
    let renewed = fixture.create_renewed_event(5_000_000);
    let canceled = fixture.create_canceled_event();
    let payment_failed = fixture.create_payment_failed_event("Test failure".to_string());

    let events_data = vec![
        (
            "Subscribed",
            EventTestFixture::create_encoded_event("Subscribed", &subscribed),
        ),
        (
            "Renewed",
            EventTestFixture::create_encoded_event("Renewed", &renewed),
        ),
        (
            "Canceled",
            EventTestFixture::create_encoded_event("Canceled", &canceled),
        ),
        (
            "PaymentFailed",
            EventTestFixture::create_encoded_event("PaymentFailed", &payment_failed),
        ),
    ];

    let logs = fixture.create_program_logs(events_data);
    let parsed_events = parse_events_from_logs(&logs, &fixture.program_id).unwrap();

    assert_eq!(parsed_events.len(), 4, "Should parse all 4 events");

    // Verify each event type was parsed correctly
    let event_types: Vec<&str> = parsed_events
        .iter()
        .map(|e| match e {
            TallyEvent::Subscribed(_) => "Subscribed",
            TallyEvent::Renewed(_) => "Renewed",
            TallyEvent::Canceled(_) => "Canceled",
            TallyEvent::PaymentFailed(_) => "PaymentFailed",
        })
        .collect();

    assert_eq!(
        event_types,
        vec!["Subscribed", "Renewed", "Canceled", "PaymentFailed"]
    );
}

#[tokio::test]
async fn test_mixed_valid_and_invalid_events_in_logs() {
    let fixture = EventTestFixture::new();

    // Mix valid and invalid events
    let valid_event = fixture.create_subscribed_event(1_000_000);
    let valid_data = EventTestFixture::create_encoded_event("Subscribed", &valid_event);

    let events_data = vec![
        ("Subscribed", valid_data),
        ("Invalid", "invalid_base64_!@#$".to_string()), // This should be skipped
        ("Corrupted", BASE64_STANDARD.encode(vec![0xFF; 4])), // This should be skipped
    ];

    let logs = fixture.create_program_logs(events_data);
    let parsed_events = parse_events_from_logs(&logs, &fixture.program_id).unwrap();

    // Should only parse the valid event, skip invalid ones
    assert_eq!(parsed_events.len(), 1, "Should only parse valid events");

    match &parsed_events[0] {
        TallyEvent::Subscribed(event) => {
            assert_eq!(event.amount, 1_000_000);
        }
        _ => panic!("Expected first event to be Subscribed"),
    }
}

const EVENT_COUNT: usize = 1000;

#[tokio::test]
async fn test_high_volume_event_parsing_performance() {
    let fixture = EventTestFixture::new();

    // Create a large number of events for performance testing
    let mut events_data = Vec::new();

    for i in 0..EVENT_COUNT {
        let event = fixture.create_subscribed_event((i as u64) * 1_000_000);
        let encoded = EventTestFixture::create_encoded_event("Subscribed", &event);
        events_data.push(("Subscribed", encoded));
    }

    let logs = fixture.create_program_logs(events_data);

    // Measure parsing performance
    let start = std::time::Instant::now();
    let parsed_events = parse_events_from_logs(&logs, &fixture.program_id).unwrap();
    let duration = start.elapsed();

    assert_eq!(parsed_events.len(), EVENT_COUNT);

    // Performance assertion: should parse 1000 events in less than 100ms
    assert!(
        duration.as_millis() < 100,
        "High volume parsing took too long: {}ms",
        duration.as_millis()
    );

    // Verify event data integrity
    for (i, event) in parsed_events.iter().enumerate() {
        match event {
            TallyEvent::Subscribed(subscribed) => {
                assert_eq!(subscribed.amount, (i as u64) * 1_000_000);
                assert_eq!(subscribed.merchant, fixture.merchant);
            }
            _ => panic!("Expected Subscribed event at index {i}"),
        }
    }
}

#[tokio::test]
async fn test_program_id_filtering() {
    let fixture = EventTestFixture::new();
    let other_program_id: Pubkey = Pubkey::from(Keypair::new().pubkey().to_bytes());

    let event = fixture.create_subscribed_event(1_000_000);
    let event_data = EventTestFixture::create_encoded_event("Subscribed", &event);

    // Create logs with events from different programs
    let logs = vec![
        format!("Program {} invoke [1]", fixture.program_id),
        format!("Program data: {} {}", fixture.program_id, &event_data), // Our program
        format!("Program data: {} {}", other_program_id, &event_data),   // Different program
        format!("Program {} success", fixture.program_id),
    ];

    // Should only parse events from our program
    let parsed_events = parse_events_from_logs(&logs, &fixture.program_id).unwrap();
    assert_eq!(
        parsed_events.len(),
        1,
        "Should only parse events from target program"
    );

    // Test parsing with different program filter
    let other_parsed = parse_events_from_logs(&logs, &other_program_id).unwrap();
    assert_eq!(
        other_parsed.len(),
        1,
        "Should parse events when filtering for other program"
    );
}

#[tokio::test]
async fn test_receipt_creation_comprehensive() {
    let fixture = EventTestFixture::new();

    // Create events
    let subscribed = fixture.create_subscribed_event(5_000_000);
    let payment_failed = fixture.create_payment_failed_event("Insufficient balance".to_string());

    let events_data = vec![
        (
            "Subscribed",
            EventTestFixture::create_encoded_event("Subscribed", &subscribed),
        ),
        (
            "PaymentFailed",
            EventTestFixture::create_encoded_event("PaymentFailed", &payment_failed),
        ),
    ];

    let logs = fixture.create_program_logs(events_data);
    let signature = Signature::new_unique();

    // Test successful transaction receipt
    let receipt = create_receipt(ReceiptParams {
        signature,
        block_time: Some(1_640_995_200), // 2022-01-01
        slot: 12345,
        success: true,
        error: None,
        logs: logs.clone(),
        compute_units_consumed: Some(15000),
        fee: 5000,
        program_id: fixture.program_id,
    })
    .unwrap();

    assert_eq!(receipt.signature, signature);
    assert_eq!(receipt.block_time, Some(1_640_995_200));
    assert_eq!(receipt.slot, 12345);
    assert!(receipt.success);
    assert_eq!(receipt.error, None);
    assert_eq!(receipt.events.len(), 2);
    assert_eq!(receipt.compute_units_consumed, Some(15000));
    assert_eq!(receipt.fee, 5000);

    // Test receipt with transaction error
    let error_receipt = create_receipt(ReceiptParams {
        signature: Signature::new_unique(),
        block_time: Some(1_640_995_200),
        slot: 12346,
        success: false,
        error: Some(TransactionError::InsufficientFundsForFee),
        logs,
        compute_units_consumed: Some(5000),
        fee: 5000,
        program_id: fixture.program_id,
    })
    .unwrap();

    assert!(!error_receipt.success);
    assert!(error_receipt.error.is_some());
    assert!(error_receipt
        .error
        .unwrap()
        .contains("InsufficientFundsForFee"));
}

#[tokio::test]
async fn test_receipt_event_getters() {
    let fixture = EventTestFixture::new();

    // Create receipt with multiple event types
    let subscribed = fixture.create_subscribed_event(5_000_000);
    let renewed = fixture.create_renewed_event(5_000_000);
    let canceled = fixture.create_canceled_event();
    let payment_failed = fixture.create_payment_failed_event("Test failure".to_string());

    let events = vec![
        TallyEvent::Subscribed(subscribed.clone()),
        TallyEvent::Renewed(renewed.clone()),
        TallyEvent::Canceled(canceled.clone()),
        TallyEvent::PaymentFailed(payment_failed.clone()),
    ];

    let receipt = TallyReceipt {
        signature: Signature::new_unique(),
        block_time: Some(1_640_995_200),
        slot: 12345,
        success: true,
        error: None,
        events,
        logs: vec![],
        compute_units_consumed: Some(15000),
        fee: 5000,
    };

    // Test event getters
    assert_eq!(receipt.get_subscribed_event(), Some(&subscribed));
    assert_eq!(receipt.get_renewed_event(), Some(&renewed));
    assert_eq!(receipt.get_canceled_event(), Some(&canceled));
    assert_eq!(receipt.get_payment_failed_event(), Some(&payment_failed));

    // Test subscription success detection
    assert!(receipt.is_subscription_success());
}

#[tokio::test]
async fn test_memo_extraction_comprehensive() {
    let memo_cases = vec![
        // Standard memo format
        (
            vec!["Program log: Memo (len 12): \"Hello World!\"".to_string()],
            Some("Hello World!".to_string()),
        ),
        // Alternative memo format
        (
            vec!["Program log: Processing memo: Payment for subscription".to_string()],
            Some("Payment for subscription".to_string()),
        ),
        // Multiple memos (should return first)
        (
            vec![
                "Program log: Memo (len 5): \"First\"".to_string(),
                "Program log: Memo (len 6): \"Second\"".to_string(),
            ],
            Some("First".to_string()),
        ),
        // No memo
        (vec!["Program log: No memo here".to_string()], None),
        // Empty memo (current implementation returns None for empty memos)
        (vec!["Program log: Memo (len 0): \"\"".to_string()], None),
        // Unicode memo
        (
            vec!["Program log: Memo (len 15): \"Hello 世界! 🌍\"".to_string()],
            Some("Hello 世界! 🌍".to_string()),
        ),
    ];

    for (logs, expected) in memo_cases {
        let extracted = extract_memo_from_logs(&logs);
        assert_eq!(
            extracted, expected,
            "Memo extraction failed for logs: {logs:?}"
        );
    }
}

#[tokio::test]
async fn test_event_discriminator_uniqueness_and_determinism() {
    let _fixture = EventTestFixture::new();

    // Test that all discriminators are unique
    let event_names = vec!["Subscribed", "Renewed", "Canceled", "PaymentFailed"];
    let mut discriminators = HashMap::new();

    for event_name in &event_names {
        let disc = EventTestFixture::compute_discriminator(event_name);

        // Check for uniqueness
        for (existing_disc, existing_name) in &discriminators {
            assert_ne!(
                disc, *existing_disc,
                "Discriminator collision between {event_name} and {existing_name}"
            );
        }

        discriminators.insert(disc, event_name);
    }

    // Test discriminator determinism (should be same across multiple computations)
    for event_name in &event_names {
        let disc1 = EventTestFixture::compute_discriminator(event_name);
        let disc2 = EventTestFixture::compute_discriminator(event_name);
        assert_eq!(
            disc1, disc2,
            "Discriminator for {event_name} should be deterministic"
        );
    }

    // Verify we have all expected discriminators
    assert_eq!(
        discriminators.len(),
        4,
        "Should have exactly 4 unique discriminators"
    );
}

#[tokio::test]
async fn test_concurrent_event_parsing() {
    let _fixture = EventTestFixture::new();

    // Test concurrent parsing of events
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let fixture = EventTestFixture::new(); // Each task gets its own fixture
            tokio::spawn(async move {
                let event = fixture.create_subscribed_event(i * 1_000_000);
                let encoded = EventTestFixture::create_encoded_event("Subscribed", &event);

                // Parse the event
                let result = parse_single_event(&encoded);
                assert!(result.is_ok(), "Failed to parse event in task {i}");

                match result.unwrap() {
                    TallyEvent::Subscribed(parsed) => {
                        assert_eq!(parsed.amount, i * 1_000_000);
                        i // Return the task number for verification
                    }
                    _ => panic!("Expected Subscribed event in task {i}"),
                }
            })
        })
        .collect();

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // Verify all tasks completed successfully
    for (i, result) in results.iter().enumerate() {
        assert_eq!(*result, i as u64, "Task {i} returned wrong result");
    }
}

#[tokio::test]
async fn test_edge_case_scenarios() {
    let fixture = EventTestFixture::new();

    // Test with maximum size event data
    let large_reason = "A".repeat(10000); // Very large error message
    let large_event = fixture.create_payment_failed_event(large_reason.clone());
    let encoded = EventTestFixture::create_encoded_event("PaymentFailed", &large_event);

    let result = parse_single_event(&encoded);
    assert!(result.is_ok(), "Should handle large event data");

    match result.unwrap() {
        TallyEvent::PaymentFailed(parsed) => {
            assert_eq!(parsed.reason, large_reason);
        }
        _ => panic!("Expected PaymentFailed event"),
    }

    // Test with minimum viable data
    let min_event = fixture.create_canceled_event();
    let min_encoded = EventTestFixture::create_encoded_event("Canceled", &min_event);

    let min_result = parse_single_event(&min_encoded);
    assert!(min_result.is_ok(), "Should handle minimal event data");
}

const BENCHMARK_COUNT: usize = 10000;

/// Performance benchmark for event parsing
#[tokio::test]
async fn benchmark_event_parsing_throughput() {
    let fixture = EventTestFixture::new();

    // Prepare test data
    let mut encoded_events = Vec::with_capacity(BENCHMARK_COUNT);

    for i in 0..BENCHMARK_COUNT {
        let event = fixture.create_subscribed_event(i as u64);
        encoded_events.push(EventTestFixture::create_encoded_event("Subscribed", &event));
    }

    // Benchmark parsing throughput
    let start = std::time::Instant::now();

    for encoded in &encoded_events {
        let result = parse_single_event(encoded);
        assert!(result.is_ok(), "Parsing should succeed in benchmark");
    }

    let duration = start.elapsed();
    #[allow(clippy::cast_precision_loss)] // Intentional conversion for benchmarking
    let events_per_second = (BENCHMARK_COUNT as f64) / duration.as_secs_f64();

    // Performance assertion: should parse at least 40K events per second
    assert!(
        events_per_second > 40_000.0,
        "Event parsing throughput too low: {events_per_second:.0} events/sec"
    );

    println!("Event parsing benchmark: {events_per_second:.0} events/sec");
}
