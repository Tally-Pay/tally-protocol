//! Event simulation command implementation
//!
//! Generates realistic Tally blockchain events for testing the event monitoring system.
//! Supports various simulation scenarios including normal operations, high churn, payment failures,
//! and custom load testing configurations.

use crate::config::TallyCliConfig;
use anyhow::{anyhow, Result};
use clap::ValueEnum;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt::Write,
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tally_sdk::{
    events::{Canceled, PaymentFailed, Renewed, Subscribed, TallyEvent},
    SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signature;
use tokio::{sync::mpsc, time::interval};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Send events via WebSocket to monitoring system
    WebSocket,
    /// Write events to a file
    File,
    /// Print events to stdout
    Stdout,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum SimulationScenario {
    /// Normal operations: 80% renewed, 15% subscribed, 4% canceled, 1% `payment_failed`
    Normal,
    /// High churn scenario: 30% canceled, 20% `payment_failed`, 40% renewed, 10% subscribed
    HighChurn,
    /// New product launch: 70% subscribed, 25% renewed, 4% canceled, 1% `payment_failed`
    NewLaunch,
    /// Payment processing issues: 50% `payment_failed`, 30% renewed, 15% subscribed, 5% canceled
    PaymentIssues,
    /// Custom distribution (specify via --event-distribution)
    Custom,
}

#[derive(Clone, Debug)]
pub struct EventDistribution {
    pub subscribed: f32,
    pub renewed: f32,
    pub canceled: f32,
    pub payment_failed: f32,
}

impl EventDistribution {
    fn validate(&self) -> Result<()> {
        let total = self.subscribed + self.renewed + self.canceled + self.payment_failed;
        if (total - 1.0).abs() > 0.001 {
            return Err(anyhow!(
                "Event distribution percentages must sum to 1.0, got: {total}"
            ));
        }
        Ok(())
    }
}

impl Default for EventDistribution {
    fn default() -> Self {
        Self {
            subscribed: 0.15,
            renewed: 0.80,
            canceled: 0.04,
            payment_failed: 0.01,
        }
    }
}

impl From<SimulationScenario> for EventDistribution {
    fn from(scenario: SimulationScenario) -> Self {
        match scenario {
            SimulationScenario::HighChurn => Self {
                subscribed: 0.10,
                renewed: 0.40,
                canceled: 0.30,
                payment_failed: 0.20,
            },
            SimulationScenario::NewLaunch => Self {
                subscribed: 0.70,
                renewed: 0.25,
                canceled: 0.04,
                payment_failed: 0.01,
            },
            SimulationScenario::PaymentIssues => Self {
                subscribed: 0.15,
                renewed: 0.30,
                canceled: 0.05,
                payment_failed: 0.50,
            },
            SimulationScenario::Custom | SimulationScenario::Normal => Self::default(), // Normal defaults, Custom will be overridden
        }
    }
}

#[derive(Clone, Debug)]
pub struct SimulateEventsCommand {
    pub merchant: Pubkey,
    pub plan: Option<Pubkey>,
    pub scenario: SimulationScenario,
    pub custom_distribution: Option<EventDistribution>,
    pub rate: u64,         // events per minute
    pub duration: u64,     // seconds
    pub batch_size: usize, // events per batch
    pub output_format: OutputFormat,
    pub websocket_url: Option<String>,
    pub output_file: Option<String>,
    pub seed: Option<u64>, // for reproducible randomness
}

impl SimulateEventsCommand {
    /// Validate command parameters
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is outside its valid range:
    /// - Rate must be between 1 and 10,000 events per minute
    /// - Duration must be between 1 and 3,600 seconds
    /// - Batch size must be between 1 and 100
    /// - Event distribution percentages must sum to 100 (when custom scenario used)
    pub fn validate(&self) -> Result<()> {
        if self.rate == 0 {
            return Err(anyhow!("Rate must be greater than 0"));
        }
        if self.rate > 10000 {
            return Err(anyhow!("Rate must not exceed 10,000 events per minute"));
        }
        if self.duration == 0 {
            return Err(anyhow!("Duration must be greater than 0"));
        }
        if self.duration > 3600 {
            return Err(anyhow!("Duration must not exceed 3600 seconds (1 hour)"));
        }
        if self.batch_size == 0 || self.batch_size > 100 {
            return Err(anyhow!("Batch size must be between 1 and 100"));
        }

        match (&self.output_format, &self.websocket_url, &self.output_file) {
            (OutputFormat::WebSocket, None, _) => {
                return Err(anyhow!(
                    "WebSocket URL required for WebSocket output format"
                ));
            }
            (OutputFormat::File, _, None) => {
                return Err(anyhow!("Output file path required for File output format"));
            }
            _ => {}
        }

        if let Some(dist) = &self.custom_distribution {
            dist.validate()?;
        }

        Ok(())
    }

    /// Get the effective event distribution for this simulation
    #[must_use]
    pub fn get_distribution(&self) -> EventDistribution {
        self.custom_distribution
            .as_ref()
            .map_or_else(|| self.scenario.clone().into(), std::clone::Clone::clone)
    }
}

/// Event generator that creates realistic Tally events
pub struct EventGenerator {
    merchant: Pubkey,
    plan: Option<Pubkey>,
    distribution: EventDistribution,
    seed: u64,
    generated_count: u64,
    subscriber_pool: Vec<Pubkey>,
    plan_pool: Vec<Pubkey>,
}

impl EventGenerator {
    /// Create a new event generator
    ///
    /// # Panics
    /// Panics if system time is before `UNIX_EPOCH` (should never happen on modern systems)
    #[must_use]
    pub fn new(command: &SimulateEventsCommand) -> Self {
        let seed = command.seed.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        // Generate a pool of realistic subscriber pubkeys
        let subscriber_pool = Self::generate_pubkey_pool(seed, 1000);

        // Generate plan pool (use provided plan or generate multiple)
        let plan_pool = command.plan.map_or_else(
            || Self::generate_plan_pool(command.merchant, seed, 10),
            |plan| vec![plan],
        );

        Self {
            merchant: command.merchant,
            plan: command.plan,
            distribution: command.get_distribution(),
            seed,
            generated_count: 0,
            subscriber_pool,
            plan_pool,
        }
    }

    /// Generate a pool of realistic subscriber pubkeys
    fn generate_pubkey_pool(seed: u64, count: usize) -> Vec<Pubkey> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut pubkeys = Vec::with_capacity(count);
        for i in 0..count {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let hash = hasher.finish();

            // Create a deterministic but realistic-looking pubkey
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&hash.to_le_bytes());
            bytes[8..16].copy_from_slice(&(hash.wrapping_mul(31)).to_le_bytes());
            bytes[16..24].copy_from_slice(&(hash.wrapping_mul(37)).to_le_bytes());
            bytes[24..32].copy_from_slice(&(hash.wrapping_mul(41)).to_le_bytes());

            pubkeys.push(Pubkey::new_from_array(bytes));
        }
        pubkeys
    }

    /// Generate a pool of plan pubkeys for the merchant
    fn generate_plan_pool(merchant: Pubkey, seed: u64, count: usize) -> Vec<Pubkey> {
        let mut plans = Vec::with_capacity(count);
        for i in 0..count {
            // Use deterministic plan ID generation
            let plan_id = format!("plan_{i}");
            if let Ok((plan_pda, _)) = tally_sdk::pda::plan_from_string(&merchant, &plan_id) {
                plans.push(plan_pda);
            } else {
                // Fallback to generated pubkey if PDA computation fails
                let mut hasher = DefaultHasher::new();
                merchant.hash(&mut hasher);
                seed.hash(&mut hasher);
                i.hash(&mut hasher);
                let hash = hasher.finish();

                let mut bytes = [0u8; 32];
                bytes[..8].copy_from_slice(&hash.to_le_bytes());
                bytes[8..16].copy_from_slice(&(hash.wrapping_mul(43)).to_le_bytes());
                bytes[16..24].copy_from_slice(&(hash.wrapping_mul(47)).to_le_bytes());
                bytes[24..32].copy_from_slice(&(hash.wrapping_mul(53)).to_le_bytes());

                plans.push(Pubkey::new_from_array(bytes));
            }
        }
        plans
    }

    /// Generate the next event based on distribution
    pub fn generate_event(&mut self) -> TallyEvent {
        // Use simple linear congruential generator for deterministic randomness
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        // Generate a value between 0.0 and 1.0 using integer arithmetic to avoid precision loss
        // Use the upper 32 bits for better distribution
        let high_bits = (self.seed >> 32) as u32;
        #[allow(clippy::cast_possible_truncation)]
        let random_value = (f64::from(high_bits) / f64::from(u32::MAX)) as f32;

        // Select event type based on distribution
        let event_type = if random_value < self.distribution.subscribed {
            "subscribed"
        } else if random_value < self.distribution.subscribed + self.distribution.renewed {
            "renewed"
        } else if random_value
            < self.distribution.subscribed + self.distribution.renewed + self.distribution.canceled
        {
            "canceled"
        } else {
            "payment_failed"
        };

        // Select random subscriber and plan
        let subscriber_idx =
            usize::try_from(self.seed % u64::try_from(self.subscriber_pool.len()).unwrap_or(1))
                .unwrap_or(0);
        let subscriber =
            self.subscriber_pool[subscriber_idx.min(self.subscriber_pool.len().saturating_sub(1))];

        let plan = if let Some(plan) = self.plan {
            plan
        } else {
            let plan_idx = usize::try_from(
                (self.seed >> 16) % u64::try_from(self.plan_pool.len()).unwrap_or(1),
            )
            .unwrap_or(0);
            let plan_idx = plan_idx.min(self.plan_pool.len().saturating_sub(1));
            self.plan_pool[plan_idx]
        };

        self.generated_count += 1;

        // Generate realistic amounts (1-100 USDC in micro-units)
        let amount_seed = self
            .seed
            .wrapping_mul(17)
            .wrapping_add(self.generated_count);
        let amount = 1_000_000 + (amount_seed % 99_000_000); // 1-100 USDC

        match event_type {
            "subscribed" => TallyEvent::Subscribed(Subscribed {
                merchant: self.merchant,
                plan,
                subscriber,
                amount,
            }),
            "renewed" => TallyEvent::Renewed(Renewed {
                merchant: self.merchant,
                plan,
                subscriber,
                amount,
            }),
            "canceled" => TallyEvent::Canceled(Canceled {
                merchant: self.merchant,
                plan,
                subscriber,
            }),
            "payment_failed" => {
                let reasons = [
                    "Insufficient funds",
                    "Token account not found",
                    "Allowance exceeded",
                    "Account frozen",
                    "Invalid token mint",
                    "Network congestion",
                    "RPC timeout",
                ];
                let reason_idx =
                    usize::try_from(amount_seed % u64::try_from(reasons.len()).unwrap_or(1))
                        .unwrap_or(0);
                let reason_idx = reason_idx.min(reasons.len().saturating_sub(1));
                TallyEvent::PaymentFailed(PaymentFailed {
                    merchant: self.merchant,
                    plan,
                    subscriber,
                    reason: reasons[reason_idx].to_string(),
                })
            }
            _ => unreachable!(),
        }
    }
}

/// Event simulator orchestrates event generation and output
pub struct EventSimulator {
    command: SimulateEventsCommand,
    generator: EventGenerator,
    stats: SimulationStats,
}

#[derive(Clone, Debug, Default)]
pub struct SimulationStats {
    pub total_events: u64,
    pub events_by_type: HashMap<String, u64>,
    pub batches_sent: u64,
    pub errors: u64,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
}

impl SimulationStats {
    fn record_event(&mut self, event: &TallyEvent) {
        self.total_events += 1;
        let event_type = match event {
            TallyEvent::Subscribed(_) => "subscribed",
            TallyEvent::Renewed(_) => "renewed",
            TallyEvent::Canceled(_) => "canceled",
            TallyEvent::PaymentFailed(_) => "payment_failed",
        };
        *self
            .events_by_type
            .entry(event_type.to_string())
            .or_insert(0) += 1;
    }

    const fn record_batch(&mut self) {
        self.batches_sent += 1;
    }

    const fn record_error(&mut self) {
        self.errors += 1;
    }

    fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    fn end(&mut self) {
        self.end_time = Some(Instant::now());
    }

    fn duration(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }

    fn events_per_second(&self) -> f64 {
        if let Some(duration) = self.duration() {
            if duration.as_secs_f64() > 0.0 {
                #[allow(clippy::cast_precision_loss)]
                return self.total_events as f64 / duration.as_secs_f64();
            }
        }
        0.0
    }
}

impl EventSimulator {
    /// Create a new event simulator
    ///
    /// # Errors
    ///
    /// Returns an error if the command validation fails or if WebSocket URL parsing fails.
    pub fn new(command: SimulateEventsCommand) -> Result<Self> {
        command.validate()?;
        let generator = EventGenerator::new(&command);

        Ok(Self {
            command,
            generator,
            stats: SimulationStats::default(),
        })
    }

    /// Calculate timing parameters for the simulation
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn calculate_timing_parameters(&self) -> (f64, u64, u64) {
        let events_per_second = self.command.rate as f64 / 60.0;
        let total_events = (events_per_second * self.command.duration as f64)
            .min(u64::MAX as f64)
            .max(0.0) as u64;
        let interval_ms = if events_per_second >= 1.0 {
            (1000.0 / events_per_second).max(1.0) as u64
        } else {
            1000
        };
        (events_per_second, total_events, interval_ms)
    }

    /// Setup the output channel and spawn the output handler
    fn setup_output_channel(
        &self,
    ) -> (
        mpsc::Sender<Vec<TallyEvent>>,
        tokio::task::JoinHandle<Result<()>>,
    ) {
        let (tx, mut rx) = mpsc::channel::<Vec<TallyEvent>>(100);
        let output_handle = {
            let command = self.command.clone();
            tokio::spawn(async move { Self::handle_output(command, &mut rx).await })
        };
        (tx, output_handle)
    }

    /// Setup shutdown signal handlers
    ///
    /// # Errors
    ///
    /// Returns an error if signal handlers cannot be set up
    fn setup_shutdown_signal() -> Result<Arc<AtomicBool>> {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

            tokio::spawn(async move {
                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, shutting down gracefully");
                        shutdown_clone.store(true, Ordering::SeqCst);
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, shutting down gracefully");
                        shutdown_clone.store(true, Ordering::SeqCst);
                    }
                }
            });
        }

        Ok(shutdown)
    }

    /// Run the main event generation loop
    ///
    /// # Errors
    ///
    /// Returns an error if there are issues with event generation or output handling
    async fn run_event_generation_loop(
        &mut self,
        tx: mpsc::Sender<Vec<TallyEvent>>,
        output_handle: tokio::task::JoinHandle<Result<()>>,
        shutdown: Arc<AtomicBool>,
        events_per_second: f64,
        total_events: u64,
        interval_ms: u64,
    ) -> Result<()> {
        let mut interval = interval(Duration::from_millis(interval_ms.max(1)));
        let mut events_sent = 0u64;
        let mut batch = Vec::with_capacity(self.command.batch_size);

        let end_time = Instant::now() + Duration::from_secs(self.command.duration);

        while events_sent < total_events
            && Instant::now() < end_time
            && !shutdown.load(Ordering::SeqCst)
        {
            interval.tick().await;

            let events_to_generate = self.calculate_events_to_generate(
                events_per_second,
                interval_ms,
                total_events,
                events_sent,
            );

            for _ in 0..events_to_generate {
                let event = self.generator.generate_event();
                self.stats.record_event(&event);
                batch.push(event);
                events_sent += 1;

                if batch.len() >= self.command.batch_size {
                    self.send_batch(&tx, &mut batch).await;
                }
            }
        }

        self.finalize_simulation(tx, output_handle, batch).await
    }

    /// Calculate how many events to generate in this iteration
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn calculate_events_to_generate(
        &self,
        events_per_second: f64,
        interval_ms: u64,
        total_events: u64,
        events_sent: u64,
    ) -> usize {
        if events_per_second < 1.0 {
            // For very low rates, generate probabilistically
            let probability = events_per_second * (interval_ms as f64 / 1000.0);
            usize::from(rand::random::<f64>() < probability)
        } else {
            // For normal rates, generate up to batch size
            let remaining = total_events - events_sent;
            self.command.batch_size.min(remaining as usize)
        }
    }

    /// Send a batch of events
    async fn send_batch(
        &mut self,
        tx: &mpsc::Sender<Vec<TallyEvent>>,
        batch: &mut Vec<TallyEvent>,
    ) {
        if let Err(e) = tx.send(batch.clone()).await {
            error!("Failed to send event batch: {}", e);
            self.stats.record_error();
        } else {
            self.stats.record_batch();
        }
        batch.clear();
    }

    /// Finalize the simulation by sending remaining events and cleaning up
    async fn finalize_simulation(
        &mut self,
        tx: mpsc::Sender<Vec<TallyEvent>>,
        output_handle: tokio::task::JoinHandle<Result<()>>,
        batch: Vec<TallyEvent>,
    ) -> Result<()> {
        // Send remaining events
        if !batch.is_empty() {
            if let Err(e) = tx.send(batch).await {
                error!("Failed to send final event batch: {}", e);
                self.stats.record_error();
            } else {
                self.stats.record_batch();
            }
        }

        // Close the channel and wait for output handler
        drop(tx);
        if let Err(e) = output_handle.await {
            error!("Output handler failed: {}", e);
            self.stats.record_error();
        }

        Ok(())
    }

    /// Run the simulation
    ///
    /// # Errors
    ///
    /// Returns an error if WebSocket connection fails, if sending events fails,
    /// or if there are issues with the simulation setup.
    pub async fn run(&mut self) -> Result<SimulationStats> {
        info!(
            "Starting event simulation: {} events/min for {} seconds",
            self.command.rate, self.command.duration
        );

        self.stats.start();

        let (events_per_second, total_events, interval_ms) = self.calculate_timing_parameters();
        info!(
            "Simulation parameters: {:.2} events/sec, {} total events, {}ms interval",
            events_per_second, total_events, interval_ms
        );

        let (tx, output_handle) = self.setup_output_channel();

        let shutdown = Self::setup_shutdown_signal()?;

        self.run_event_generation_loop(
            tx,
            output_handle,
            shutdown,
            events_per_second,
            total_events,
            interval_ms,
        )
        .await?;

        self.stats.end();

        info!(
            "Simulation completed: {} events in {:.2}s ({:.2} events/sec)",
            self.stats.total_events,
            self.stats.duration().unwrap_or_default().as_secs_f64(),
            self.stats.events_per_second()
        );

        Ok(self.stats.clone())
    }

    /// Handle output of generated events
    async fn handle_output(
        command: SimulateEventsCommand,
        rx: &mut mpsc::Receiver<Vec<TallyEvent>>,
    ) -> Result<()> {
        match command.output_format {
            OutputFormat::WebSocket => {
                Self::handle_websocket_output(command.websocket_url.unwrap(), rx).await
            }
            OutputFormat::File => Self::handle_file_output(command.output_file.unwrap(), rx).await,
            OutputFormat::Stdout => Self::handle_stdout_output(rx).await,
        }
    }

    /// Handle WebSocket output
    #[allow(clippy::cognitive_complexity)]
    async fn handle_websocket_output(
        websocket_url: String,
        rx: &mut mpsc::Receiver<Vec<TallyEvent>>,
    ) -> Result<()> {
        info!("Connecting to WebSocket: {}", websocket_url);

        let (ws_stream, _) = connect_async(&websocket_url)
            .await
            .map_err(|e| anyhow!("Failed to connect to WebSocket: {e}"))?;

        let (mut write, _read) = ws_stream.split();

        info!("Connected to WebSocket, sending events...");

        while let Some(events) = rx.recv().await {
            for event in events {
                let message = Self::format_websocket_message(&event)?;

                if let Err(e) = write.send(Message::Text(message)).await {
                    error!("Failed to send WebSocket message: {}", e);
                    return Err(anyhow!("WebSocket send failed: {e}"));
                }

                debug!("Sent event via WebSocket: {:?}", event);
            }
        }

        info!("WebSocket output completed");
        Ok(())
    }

    /// Handle file output
    async fn handle_file_output(
        file_path: String,
        rx: &mut mpsc::Receiver<Vec<TallyEvent>>,
    ) -> Result<()> {
        use tokio::fs::OpenOptions;
        use tokio::io::AsyncWriteExt;

        info!("Writing events to file: {}", file_path);

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .await
            .map_err(|e| anyhow!("Failed to open output file: {e}"))?;

        while let Some(events) = rx.recv().await {
            for event in events {
                let line = format!("{}\n", serde_json::to_string(&event)?);
                file.write_all(line.as_bytes())
                    .await
                    .map_err(|e| anyhow!("Failed to write to file: {e}"))?;
            }
        }

        file.flush()
            .await
            .map_err(|e| anyhow!("Failed to flush file: {e}"))?;

        info!("File output completed");
        Ok(())
    }

    /// Handle stdout output
    async fn handle_stdout_output(rx: &mut mpsc::Receiver<Vec<TallyEvent>>) -> Result<()> {
        while let Some(events) = rx.recv().await {
            for event in events {
                println!("{}", serde_json::to_string(&event)?);
            }
        }
        Ok(())
    }

    /// Format an event as a WebSocket message matching the monitoring system format
    fn format_websocket_message(event: &TallyEvent) -> Result<String> {
        // Create a realistic transaction signature
        let tx_sig = Signature::new_unique();

        // Create a mock Solana RPC notification format that matches what the monitoring system expects
        let mock_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "logsNotification",
            "params": {
                "result": {
                    "context": {
                        "slot": Self::current_slot()
                    },
                    "value": {
                        "signature": tx_sig.to_string(),
                        "err": null,
                        "logs": Self::create_mock_logs(event, &tx_sig)?
                    }
                },
                "subscription": 1
            }
        });

        serde_json::to_string(&mock_notification)
            .map_err(|e| anyhow!("Failed to serialize WebSocket message: {e}"))
    }

    /// Create mock transaction logs that contain the event data
    fn create_mock_logs(event: &TallyEvent, _tx_sig: &Signature) -> Result<Vec<String>> {
        let program_id = tally_sdk::program_id();

        // Serialize the event with proper discriminator
        let event_data = Self::serialize_event_with_discriminator(event)?;
        let encoded_data = base64::prelude::BASE64_STANDARD.encode(event_data);

        let logs = vec![
            format!("Program {} invoke [1]", program_id),
            "Program log: Instruction: StartSubscription".to_string(),
            format!("Program data: {} {}", program_id, encoded_data),
            format!(
                "Program {} consumed 5000 of 200000 compute units",
                program_id
            ),
            format!("Program {} success", program_id),
        ];

        Ok(logs)
    }

    /// Serialize an event with the proper Anchor discriminator
    fn serialize_event_with_discriminator(event: &TallyEvent) -> Result<Vec<u8>> {
        use anchor_lang::prelude::*;
        use anchor_lang::solana_program::hash;

        let (event_name, event_data) = match event {
            TallyEvent::Subscribed(e) => ("Subscribed", e.try_to_vec()?),
            TallyEvent::Renewed(e) => ("Renewed", e.try_to_vec()?),
            TallyEvent::Canceled(e) => ("Canceled", e.try_to_vec()?),
            TallyEvent::PaymentFailed(e) => ("PaymentFailed", e.try_to_vec()?),
        };

        // Compute discriminator: first 8 bytes of SHA256("event:<EventName>")
        let preimage = format!("event:{event_name}");
        let hash_result = hash::hash(preimage.as_bytes());
        let discriminator = &hash_result.to_bytes()[..8];

        // Combine discriminator + event data
        let mut result = Vec::with_capacity(8 + event_data.len());
        result.extend_from_slice(discriminator);
        result.extend_from_slice(&event_data);

        Ok(result)
    }

    /// Get current slot number (mock implementation)
    fn current_slot() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 2 // Approximate 2-second slot times
    }
}

/// Execute the simulate events command
///
/// # Errors
///
/// Returns an error if the event simulation fails, if WebSocket connections fail,
/// or if the command validation fails.
#[allow(clippy::cast_precision_loss)]
pub async fn execute(
    _tally_client: &SimpleTallyClient,
    command: SimulateEventsCommand,
    _config: &TallyCliConfig,
) -> Result<String> {
    let mut simulator = EventSimulator::new(command)?;
    let stats = simulator.run().await?;

    // Format results
    let duration = stats.duration().unwrap_or_default();
    let mut output = format!("Event Simulation Results\n{}\n", "=".repeat(50));
    writeln!(output, "Total Events:      {}", stats.total_events)?;
    writeln!(output, "Duration:          {:.2}s", duration.as_secs_f64())?;
    writeln!(
        output,
        "Events/Second:     {:.2}",
        stats.events_per_second()
    )?;
    writeln!(output, "Batches Sent:      {}", stats.batches_sent)?;
    writeln!(output, "Errors:            {}", stats.errors)?;

    output.push_str("\nEvents by Type:\n");
    for (event_type, count) in &stats.events_by_type {
        let percentage = if stats.total_events > 0 {
            (*count as f64 / stats.total_events as f64) * 100.0
        } else {
            0.0
        };
        writeln!(output, "  {event_type:<15} {count:>8} ({percentage:>5.1}%)")?;
    }

    Ok(output)
}

// Add rand dependency for probabilistic event generation
mod rand {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEED: AtomicU64 = AtomicU64::new(1);

    pub fn random<T: Random>() -> T {
        T::random()
    }

    pub trait Random {
        fn random() -> Self;
    }

    impl Random for f64 {
        #[allow(clippy::cast_precision_loss)]
        fn random() -> Self {
            let seed = SEED.fetch_add(1, Ordering::SeqCst);
            let mut x = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            x = (x >> 16) & 0x7FFF;
            x as Self / 32767.0
        }
    }
}

// Tests module
#[cfg(test)]
mod tests;

// Required dependencies for WebSocket functionality
use base64::prelude::*;
use futures_util::{SinkExt, StreamExt};
