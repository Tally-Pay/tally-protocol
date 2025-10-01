//! Tests for event simulation functionality

use super::*;
use anchor_client::solana_sdk::signature::{Keypair, Signer};
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_event_generator_creation() {
    let merchant = Keypair::new().pubkey();
    let plan = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: Some(plan),
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 60,
        duration: 10,
        batch_size: 5,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let generator = EventGenerator::new(&command);
    assert_eq!(generator.merchant, merchant);
    assert_eq!(generator.plan, Some(plan));
    assert_eq!(generator.seed, 12345);
    assert_eq!(generator.subscriber_pool.len(), 1000);
    assert_eq!(generator.plan_pool.len(), 1);
}

#[tokio::test]
async fn test_event_generator_without_plan() {
    let merchant = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 60,
        duration: 10,
        batch_size: 5,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let generator = EventGenerator::new(&command);
    assert_eq!(generator.merchant, merchant);
    assert_eq!(generator.plan, None);
    assert_eq!(generator.plan_pool.len(), 10); // Should generate multiple plans
}

#[tokio::test]
async fn test_event_generation_deterministic() {
    let merchant = Keypair::new().pubkey();
    let plan = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: Some(plan),
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 60,
        duration: 10,
        batch_size: 5,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let mut generator1 = EventGenerator::new(&command);
    let mut generator2 = EventGenerator::new(&command);

    // Generate several events from both generators
    let events1: Vec<TallyEvent> = (0..10).map(|_| generator1.generate_event()).collect();
    let events2: Vec<TallyEvent> = (0..10).map(|_| generator2.generate_event()).collect();

    // Events should be identical due to same seed
    assert_eq!(events1.len(), events2.len());
    for (e1, e2) in events1.iter().zip(events2.iter()) {
        match (e1, e2) {
            (TallyEvent::Subscribed(s1), TallyEvent::Subscribed(s2)) => {
                assert_eq!(s1.merchant, s2.merchant);
                assert_eq!(s1.plan, s2.plan);
                assert_eq!(s1.subscriber, s2.subscriber);
                assert_eq!(s1.amount, s2.amount);
            }
            (TallyEvent::Renewed(r1), TallyEvent::Renewed(r2)) => {
                assert_eq!(r1.merchant, r2.merchant);
                assert_eq!(r1.plan, r2.plan);
                assert_eq!(r1.subscriber, r2.subscriber);
                assert_eq!(r1.amount, r2.amount);
            }
            (TallyEvent::Canceled(c1), TallyEvent::Canceled(c2)) => {
                assert_eq!(c1.merchant, c2.merchant);
                assert_eq!(c1.plan, c2.plan);
                assert_eq!(c1.subscriber, c2.subscriber);
            }
            (TallyEvent::PaymentFailed(p1), TallyEvent::PaymentFailed(p2)) => {
                assert_eq!(p1.merchant, p2.merchant);
                assert_eq!(p1.plan, p2.plan);
                assert_eq!(p1.subscriber, p2.subscriber);
                assert_eq!(p1.reason, p2.reason);
            }
            _ => panic!("Event types don't match"),
        }
    }
}

#[tokio::test]
async fn test_event_distribution_normal_scenario() {
    let merchant = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 6000,  // High rate for statistical accuracy
        duration: 1, // Short duration
        batch_size: 100,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let mut generator = EventGenerator::new(&command);
    let mut counts = HashMap::new();
    let total_events = 1000;

    // Generate many events to test distribution
    for _ in 0..total_events {
        let event = generator.generate_event();
        let event_type = match event {
            TallyEvent::Subscribed(_) => "subscribed",
            TallyEvent::Renewed(_) => "renewed",
            TallyEvent::Canceled(_) => "canceled",
            TallyEvent::PaymentFailed(_) => "payment_failed",
        };
        *counts.entry(event_type).or_insert(0) += 1;
    }

    // Check approximate distribution (within 5% tolerance)
    let distribution = EventDistribution::from(SimulationScenario::Normal);

    let subscribed_ratio =
        f64::from(*counts.get("subscribed").unwrap_or(&0)) / f64::from(total_events);
    let renewed_ratio = f64::from(*counts.get("renewed").unwrap_or(&0)) / f64::from(total_events);
    let canceled_ratio = f64::from(*counts.get("canceled").unwrap_or(&0)) / f64::from(total_events);
    let payment_failed_ratio =
        f64::from(*counts.get("payment_failed").unwrap_or(&0)) / f64::from(total_events);

    assert!((subscribed_ratio - f64::from(distribution.subscribed)).abs() < 0.05);
    assert!((renewed_ratio - f64::from(distribution.renewed)).abs() < 0.05);
    assert!((canceled_ratio - f64::from(distribution.canceled)).abs() < 0.05);
    assert!((payment_failed_ratio - f64::from(distribution.payment_failed)).abs() < 0.05);
}

#[tokio::test]
async fn test_event_distribution_high_churn_scenario() {
    let merchant = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::HighChurn,
        custom_distribution: None,
        rate: 6000,
        duration: 1,
        batch_size: 100,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let mut generator = EventGenerator::new(&command);
    let mut counts = HashMap::new();
    let total_events = 1000;

    for _ in 0..total_events {
        let event = generator.generate_event();
        let event_type = match event {
            TallyEvent::Subscribed(_) => "subscribed",
            TallyEvent::Renewed(_) => "renewed",
            TallyEvent::Canceled(_) => "canceled",
            TallyEvent::PaymentFailed(_) => "payment_failed",
        };
        *counts.entry(event_type).or_insert(0) += 1;
    }

    // High churn should have more cancellations and payment failures
    let canceled_ratio = f64::from(*counts.get("canceled").unwrap_or(&0)) / f64::from(total_events);
    let payment_failed_ratio =
        f64::from(*counts.get("payment_failed").unwrap_or(&0)) / f64::from(total_events);

    // Should be higher than normal scenario
    assert!(canceled_ratio > 0.25); // Expected ~30%
    assert!(payment_failed_ratio > 0.15); // Expected ~20%
}

#[tokio::test]
async fn test_custom_event_distribution() {
    let merchant = Keypair::new().pubkey();

    let custom_dist = EventDistribution {
        subscribed: 0.5,
        renewed: 0.3,
        canceled: 0.15,
        payment_failed: 0.05,
    };

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Custom,
        custom_distribution: Some(custom_dist),
        rate: 6000,
        duration: 1,
        batch_size: 100,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let mut generator = EventGenerator::new(&command);
    let mut counts = HashMap::new();
    let total_events = 1000;

    for _ in 0..total_events {
        let event = generator.generate_event();
        let event_type = match event {
            TallyEvent::Subscribed(_) => "subscribed",
            TallyEvent::Renewed(_) => "renewed",
            TallyEvent::Canceled(_) => "canceled",
            TallyEvent::PaymentFailed(_) => "payment_failed",
        };
        *counts.entry(event_type).or_insert(0) += 1;
    }

    let subscribed_ratio =
        f64::from(*counts.get("subscribed").unwrap_or(&0)) / f64::from(total_events);
    let renewed_ratio = f64::from(*counts.get("renewed").unwrap_or(&0)) / f64::from(total_events);

    // Check custom distribution is approximately followed
    assert!((subscribed_ratio - 0.5).abs() < 0.05);
    assert!((renewed_ratio - 0.3).abs() < 0.05);
}

#[tokio::test]
async fn test_command_validation() {
    let merchant = Keypair::new().pubkey();

    // Valid command
    let valid_command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 60,
        duration: 10,
        batch_size: 5,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: None,
    };
    assert!(valid_command.validate().is_ok());

    // Invalid rate (too high)
    let invalid_rate = SimulateEventsCommand {
        rate: 20000, // Too high
        ..valid_command.clone()
    };
    assert!(invalid_rate.validate().is_err());

    // Invalid rate (zero)
    let zero_rate = SimulateEventsCommand {
        rate: 0,
        ..valid_command.clone()
    };
    assert!(zero_rate.validate().is_err());

    // Invalid duration (too long)
    let invalid_duration = SimulateEventsCommand {
        duration: 7200, // Too long
        ..valid_command.clone()
    };
    assert!(invalid_duration.validate().is_err());

    // Invalid batch size
    let invalid_batch = SimulateEventsCommand {
        batch_size: 0,
        ..valid_command.clone()
    };
    assert!(invalid_batch.validate().is_err());

    // WebSocket output without URL
    let websocket_no_url = SimulateEventsCommand {
        output_format: OutputFormat::WebSocket,
        websocket_url: None,
        ..valid_command.clone()
    };
    assert!(websocket_no_url.validate().is_err());

    // File output without path
    let file_no_path = SimulateEventsCommand {
        output_format: OutputFormat::File,
        output_file: None,
        ..valid_command
    };
    assert!(file_no_path.validate().is_err());
}

#[tokio::test]
async fn test_event_distribution_validation() {
    // Valid distribution
    let valid_dist = EventDistribution {
        subscribed: 0.25,
        renewed: 0.50,
        canceled: 0.20,
        payment_failed: 0.05,
    };
    assert!(valid_dist.validate().is_ok());

    // Invalid distribution (doesn't sum to 1.0)
    let invalid_dist = EventDistribution {
        subscribed: 0.25,
        renewed: 0.50,
        canceled: 0.20,
        payment_failed: 0.10, // Total = 1.05
    };
    assert!(invalid_dist.validate().is_err());

    // Another invalid distribution (under 1.0)
    let under_dist = EventDistribution {
        subscribed: 0.20,
        renewed: 0.40,
        canceled: 0.15,
        payment_failed: 0.05, // Total = 0.80
    };
    assert!(under_dist.validate().is_err());
}

#[tokio::test]
async fn test_simulate_events_stdout() {
    let merchant = Keypair::new().pubkey();

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 600,   // 10 events per second
        duration: 1, // 1 second
        batch_size: 5,
        output_format: OutputFormat::Stdout,
        websocket_url: None,
        output_file: None,
        seed: Some(12345),
    };

    let mut simulator = EventSimulator::new(command).unwrap();

    // Run simulation with timeout to prevent hanging
    let result = timeout(Duration::from_secs(5), simulator.run()).await;
    assert!(result.is_ok(), "Simulation should complete within timeout");

    let stats = result.unwrap().unwrap();
    assert!(stats.total_events > 0);
    assert!(stats.events_per_second() > 0.0);
    assert_eq!(stats.errors, 0);
}

#[tokio::test]
async fn test_simulate_events_file_output() {
    let merchant = Keypair::new().pubkey();
    let temp_file = "/tmp/test_events.jsonl";

    let command = SimulateEventsCommand {
        merchant,
        plan: None,
        scenario: SimulationScenario::Normal,
        custom_distribution: None,
        rate: 300, // 5 events per second
        duration: 1,
        batch_size: 3,
        output_format: OutputFormat::File,
        websocket_url: None,
        output_file: Some(temp_file.to_string()),
        seed: Some(12345),
    };

    let mut simulator = EventSimulator::new(command).unwrap();
    let result = timeout(Duration::from_secs(5), simulator.run()).await;
    assert!(result.is_ok());

    let stats = result.unwrap().unwrap();
    assert!(stats.total_events > 0);

    // Check that file was created and contains events
    let file_contents = tokio::fs::read_to_string(temp_file).await.unwrap();
    assert!(!file_contents.is_empty());

    // Clean up
    let _ = tokio::fs::remove_file(temp_file).await;
}

#[tokio::test]
async fn test_websocket_message_format() {
    let merchant = Keypair::new().pubkey();
    let plan = Keypair::new().pubkey();
    let subscriber = Keypair::new().pubkey();

    let event = TallyEvent::Subscribed(Subscribed {
        merchant,
        plan,
        subscriber,
        amount: 5_000_000,
    });

    let message = EventSimulator::format_websocket_message(&event).unwrap();

    // Parse the message to ensure it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&message).unwrap();

    // Check structure matches Solana RPC notification format
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["method"], "logsNotification");
    assert!(parsed["params"]["result"]["context"]["slot"].is_number());
    assert!(parsed["params"]["result"]["value"]["signature"].is_string());
    assert!(parsed["params"]["result"]["value"]["logs"].is_array());

    let logs = parsed["params"]["result"]["value"]["logs"]
        .as_array()
        .unwrap();
    assert!(!logs.is_empty());

    // Check that one of the logs contains program data
    let has_program_data = logs
        .iter()
        .any(|log| log.as_str().unwrap_or("").contains("Program data:"));
    assert!(has_program_data);
}

#[tokio::test]
async fn test_event_serialization_with_discriminator() {
    let merchant = Keypair::new().pubkey();
    let plan = Keypair::new().pubkey();
    let subscriber = Keypair::new().pubkey();

    // Test each event type
    let events = vec![
        TallyEvent::Subscribed(Subscribed {
            merchant,
            plan,
            subscriber,
            amount: 1_000_000,
        }),
        TallyEvent::Renewed(Renewed {
            merchant,
            plan,
            subscriber,
            amount: 2_000_000,
        }),
        TallyEvent::Canceled(Canceled {
            merchant,
            plan,
            subscriber,
        }),
        TallyEvent::PaymentFailed(PaymentFailed {
            merchant,
            plan,
            subscriber,
            reason: "Test failure".to_string(),
        }),
    ];

    for event in events {
        let serialized = EventSimulator::serialize_event_with_discriminator(&event).unwrap();

        // Should have at least 8 bytes for discriminator
        assert!(serialized.len() >= 8);

        // First 8 bytes should be the discriminator
        let discriminator = &serialized[..8];

        // Discriminator should be non-zero (Anchor discriminators are hashes)
        assert_ne!(discriminator, &[0u8; 8]);

        // Event data should be after discriminator
        let event_data = &serialized[8..];
        assert!(!event_data.is_empty());
    }
}

#[tokio::test]
async fn test_simulation_stats() {
    let mut stats = SimulationStats::default();

    // Test initial state
    assert_eq!(stats.total_events, 0);
    assert_eq!(stats.batches_sent, 0);
    assert_eq!(stats.errors, 0);
    assert!(stats.start_time.is_none());
    assert!(stats.end_time.is_none());

    // Test recording events
    let merchant = Keypair::new().pubkey();
    let plan = Keypair::new().pubkey();
    let subscriber = Keypair::new().pubkey();

    let subscribed_event = TallyEvent::Subscribed(Subscribed {
        merchant,
        plan,
        subscriber,
        amount: 1_000_000,
    });

    let renewed_event = TallyEvent::Renewed(Renewed {
        merchant,
        plan,
        subscriber,
        amount: 2_000_000,
    });

    stats.record_event(&subscribed_event);
    stats.record_event(&renewed_event);
    stats.record_event(&subscribed_event);

    assert_eq!(stats.total_events, 3);
    assert_eq!(stats.events_by_type["subscribed"], 2);
    assert_eq!(stats.events_by_type["renewed"], 1);

    // Test batches and errors
    stats.record_batch();
    stats.record_batch();
    stats.record_error();

    assert_eq!(stats.batches_sent, 2);
    assert_eq!(stats.errors, 1);

    // Test timing
    stats.start();
    tokio::time::sleep(Duration::from_millis(100)).await;
    stats.end();

    assert!(stats.start_time.is_some());
    assert!(stats.end_time.is_some());
    assert!(stats.duration().is_some());
    assert!(stats.duration().unwrap().as_millis() >= 100);
    assert!(stats.events_per_second() > 0.0);
}

#[tokio::test]
async fn test_pubkey_pool_generation() {
    let pool1 = EventGenerator::generate_pubkey_pool(12345, 100);
    let pool2 = EventGenerator::generate_pubkey_pool(12345, 100);
    let pool3 = EventGenerator::generate_pubkey_pool(54321, 100);

    // Same seed should generate same pubkeys
    assert_eq!(pool1.len(), 100);
    assert_eq!(pool2.len(), 100);
    assert_eq!(pool1, pool2);

    // Different seed should generate different pubkeys
    assert_ne!(pool1, pool3);

    // All pubkeys should be unique within a pool
    let mut unique_pool1 = pool1.clone();
    unique_pool1.sort();
    unique_pool1.dedup();
    assert_eq!(unique_pool1.len(), pool1.len());
}

#[tokio::test]
async fn test_plan_pool_generation() {
    let merchant1 = Keypair::new().pubkey();
    let merchant2 = Keypair::new().pubkey();

    let pool1 = EventGenerator::generate_plan_pool(merchant1, 12345, 5);
    let pool2 = EventGenerator::generate_plan_pool(merchant1, 12345, 5);
    let pool3 = EventGenerator::generate_plan_pool(merchant2, 12345, 5);

    // Same merchant should generate same plans (deterministic)
    assert_eq!(pool1.len(), 5);
    assert_eq!(pool2.len(), 5);
    assert_eq!(pool1, pool2);

    // Different merchant should generate different plans
    assert_ne!(pool1, pool3);

    // All plans should be unique within a pool
    let mut unique_pool1 = pool1.clone();
    unique_pool1.sort();
    unique_pool1.dedup();
    assert_eq!(unique_pool1.len(), pool1.len());
}
