# Spam Detection and Monitoring Guide

## Overview

This guide provides production-ready code examples and implementation strategies for detecting and preventing spam attacks on Tally Protocol. It complements the [RATE_LIMITING_STRATEGY.md](./RATE_LIMITING_STRATEGY.md) document with concrete implementations.

**Key Components:**
1. Indexer-based pattern detection
2. Real-time alerting systems
3. Dashboard anomaly detection
4. Automated response mechanisms

---

## Table of Contents

1. [Indexer Implementation](#indexer-implementation)
2. [Alert Thresholds and Rules](#alert-thresholds-and-rules)
3. [Real-Time Monitoring](#real-time-monitoring)
4. [Automated Response Systems](#automated-response-systems)
5. [Integration Examples](#integration-examples)

---

## Indexer Implementation

### Architecture Overview

```
Solana RPC Node
    ↓ (WebSocket subscription)
Transaction Indexer
    ↓ (Event parsing)
Event Store (PostgreSQL/TimescaleDB)
    ↓ (Pattern detection)
Alert Engine
    ↓ (Notifications)
Operations Dashboard / Incident Response
```

### Rust Indexer Example

**Dependencies (Cargo.toml):**

```toml
[dependencies]
anchor-client = "0.31.1"
solana-client = "2.0"
solana-sdk = "2.0"
tokio = { version = "1.40", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "chrono"] }
chrono = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Complete Indexer Implementation:**

```rust
use anchor_client::solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter},
};
use anchor_client::solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::str::FromStr;
use tracing::{error, info, warn};

/// Configuration for the spam detection indexer
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub rpc_url: String,
    pub program_id: String,
    pub database_url: String,
    pub alert_webhook_url: Option<String>,
}

/// Detected operation type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    PlanCreated,
    SubscriptionStarted,
    SubscriptionCanceled,
    SubscriptionRenewed,
}

/// Indexed transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedOperation {
    pub signature: String,
    pub timestamp: DateTime<Utc>,
    pub operation_type: OperationType,
    pub merchant: Option<String>,
    pub subscriber: Option<String>,
    pub plan: Option<String>,
    pub amount: Option<u64>,
}

/// Spam pattern detection result
#[derive(Debug, Clone, Serialize)]
pub struct SpamAlert {
    pub alert_type: String,
    pub severity: AlertSeverity,
    pub account: String,
    pub metric: String,
    pub threshold: f64,
    pub actual: f64,
    pub window_secs: u64,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

pub struct SpamDetectionIndexer {
    rpc: RpcClient,
    db_pool: PgPool,
    config: IndexerConfig,
}

impl SpamDetectionIndexer {
    /// Initialize the indexer with database connection
    pub async fn new(config: IndexerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Connect to PostgreSQL
        let db_pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&config.database_url)
            .await?;

        // Initialize schema
        Self::init_schema(&db_pool).await?;

        let rpc = RpcClient::new_with_commitment(
            config.rpc_url.clone(),
            CommitmentConfig::confirmed(),
        );

        Ok(Self {
            rpc,
            db_pool,
            config,
        })
    }

    /// Create database schema for indexing
    async fn init_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS indexed_operations (
                id BIGSERIAL PRIMARY KEY,
                signature TEXT NOT NULL UNIQUE,
                timestamp TIMESTAMPTZ NOT NULL,
                operation_type TEXT NOT NULL,
                merchant TEXT,
                subscriber TEXT,
                plan TEXT,
                amount BIGINT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_timestamp ON indexed_operations(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_merchant ON indexed_operations(merchant);
            CREATE INDEX IF NOT EXISTS idx_subscriber ON indexed_operations(subscriber);
            CREATE INDEX IF NOT EXISTS idx_operation_type ON indexed_operations(operation_type);

            -- Hypertable for time-series optimization (if using TimescaleDB)
            -- SELECT create_hypertable('indexed_operations', 'timestamp', if_not_exists => TRUE);
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Index a single transaction
    pub async fn index_transaction(
        &self,
        signature: &Signature,
    ) -> Result<Option<IndexedOperation>, Box<dyn std::error::Error>> {
        // Fetch transaction details
        let tx = self.rpc.get_transaction(signature, Default::default())?;

        // Parse operation from transaction logs
        let operation = self.parse_operation(&tx)?;

        if let Some(op) = &operation {
            // Store in database
            self.store_operation(op).await?;

            // Run spam detection
            self.detect_spam_patterns(op).await?;
        }

        Ok(operation)
    }

    /// Parse operation from transaction (simplified - extend for full event parsing)
    fn parse_operation(
        &self,
        tx: &solana_client::rpc_response::EncodedConfirmedTransactionWithStatusMeta,
    ) -> Result<Option<IndexedOperation>, Box<dyn std::error::Error>> {
        // Implementation would parse anchor events from transaction logs
        // This is a placeholder - use tally-sdk event parsing in production

        // Example structure:
        // - Extract program logs
        // - Parse anchor event discriminators
        // - Deserialize event data
        // - Map to IndexedOperation

        // For demonstration, return None
        // In production, use tally_sdk::events::parse_events()
        Ok(None)
    }

    /// Store operation in database
    async fn store_operation(
        &self,
        op: &IndexedOperation,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO indexed_operations
            (signature, timestamp, operation_type, merchant, subscriber, plan, amount)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (signature) DO NOTHING
            "#,
        )
        .bind(&op.signature)
        .bind(&op.timestamp)
        .bind(serde_json::to_string(&op.operation_type)?)
        .bind(&op.merchant)
        .bind(&op.subscriber)
        .bind(&op.plan)
        .bind(op.amount.map(|a| a as i64))
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Detect spam patterns based on recent operations
    pub async fn detect_spam_patterns(
        &self,
        op: &IndexedOperation,
    ) -> Result<Vec<SpamAlert>, Box<dyn std::error::Error>> {
        let mut alerts = Vec::new();

        // Check merchant plan creation rate
        if let Some(merchant) = &op.merchant {
            if matches!(op.operation_type, OperationType::PlanCreated) {
                alerts.extend(self.check_merchant_plan_rate(merchant).await?);
            }
        }

        // Check subscriber churn rate
        if let Some(subscriber) = &op.subscriber {
            alerts.extend(self.check_subscriber_churn(subscriber).await?);
        }

        // Check cancellation spam
        if let Some(subscriber) = &op.subscriber {
            if matches!(op.operation_type, OperationType::SubscriptionCanceled) {
                alerts.extend(self.check_cancellation_spam(subscriber).await?);
            }
        }

        // Send alerts if any detected
        for alert in &alerts {
            self.send_alert(alert).await?;
        }

        Ok(alerts)
    }

    /// Check merchant plan creation rate (>10 per hour)
    async fn check_merchant_plan_rate(
        &self,
        merchant: &str,
    ) -> Result<Vec<SpamAlert>, Box<dyn std::error::Error>> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM indexed_operations
            WHERE merchant = $1
              AND operation_type = '"plan_created"'
              AND timestamp > NOW() - INTERVAL '1 hour'
            "#,
        )
        .bind(merchant)
        .fetch_one(&self.db_pool)
        .await?;

        let threshold = 10.0;
        let actual = count.0 as f64;

        if actual > threshold {
            Ok(vec![SpamAlert {
                alert_type: "merchant_plan_spam".to_string(),
                severity: AlertSeverity::Critical,
                account: merchant.to_string(),
                metric: "plans_created_per_hour".to_string(),
                threshold,
                actual,
                window_secs: 3600,
                evidence: vec![
                    format!("Created {} plans in last hour", count.0),
                    format!("Threshold: {} plans/hour", threshold),
                ],
            }])
        } else {
            Ok(vec![])
        }
    }

    /// Check subscriber churn (>80% cancel within 1 hour of start)
    async fn check_subscriber_churn(
        &self,
        subscriber: &str,
    ) -> Result<Vec<SpamAlert>, Box<dyn std::error::Error>> {
        let stats: (i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE operation_type = '"subscription_started"'),
                COUNT(*) FILTER (WHERE operation_type = '"subscription_canceled"')
            FROM indexed_operations
            WHERE subscriber = $1
              AND timestamp > NOW() - INTERVAL '1 hour'
            "#,
        )
        .bind(subscriber)
        .fetch_one(&self.db_pool)
        .await?;

        let started = stats.0 as f64;
        let canceled = stats.1 as f64;

        if started > 0.0 {
            let churn_rate = canceled / started;
            let threshold = 0.8;

            if churn_rate > threshold && started >= 5.0 {
                Ok(vec![SpamAlert {
                    alert_type: "subscriber_churn_spam".to_string(),
                    severity: AlertSeverity::Warning,
                    account: subscriber.to_string(),
                    metric: "churn_rate".to_string(),
                    threshold,
                    actual: churn_rate,
                    window_secs: 3600,
                    evidence: vec![
                        format!("Started {} subscriptions", started as u64),
                        format!("Canceled {} subscriptions", canceled as u64),
                        format!("Churn rate: {:.1}%", churn_rate * 100.0),
                    ],
                }])
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    }

    /// Check cancellation spam (>10 cancellations per hour)
    async fn check_cancellation_spam(
        &self,
        subscriber: &str,
    ) -> Result<Vec<SpamAlert>, Box<dyn std::error::Error>> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM indexed_operations
            WHERE subscriber = $1
              AND operation_type = '"subscription_canceled"'
              AND timestamp > NOW() - INTERVAL '1 hour'
            "#,
        )
        .bind(subscriber)
        .fetch_one(&self.db_pool)
        .await?;

        let threshold = 10.0;
        let actual = count.0 as f64;

        if actual > threshold {
            Ok(vec![SpamAlert {
                alert_type: "cancellation_spam".to_string(),
                severity: AlertSeverity::Info,
                account: subscriber.to_string(),
                metric: "cancellations_per_hour".to_string(),
                threshold,
                actual,
                window_secs: 3600,
                evidence: vec![
                    format!("Canceled {} subscriptions in last hour", count.0),
                    "Note: Cancellation spam is low impact (self-inflicted)".to_string(),
                ],
            }])
        } else {
            Ok(vec![])
        }
    }

    /// Send alert to webhook or logging system
    async fn send_alert(&self, alert: &SpamAlert) -> Result<(), Box<dyn std::error::Error>> {
        match alert.severity {
            AlertSeverity::Critical => error!("🚨 CRITICAL SPAM ALERT: {:?}", alert),
            AlertSeverity::Warning => warn!("⚠️  WARNING SPAM ALERT: {:?}", alert),
            AlertSeverity::Info => info!("ℹ️  INFO SPAM ALERT: {:?}", alert),
        }

        // Send to webhook if configured
        if let Some(webhook_url) = &self.config.alert_webhook_url {
            let client = reqwest::Client::new();
            client
                .post(webhook_url)
                .json(alert)
                .send()
                .await?;
        }

        Ok(())
    }

    /// Get merchant operation summary for dashboard
    pub async fn get_merchant_summary(
        &self,
        merchant: &str,
        window_hours: u32,
    ) -> Result<MerchantSummary, Box<dyn std::error::Error>> {
        let summary: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE operation_type = '"plan_created"'),
                COUNT(*) FILTER (WHERE operation_type = '"subscription_started"'),
                COUNT(*) FILTER (WHERE operation_type = '"subscription_canceled"')
            FROM indexed_operations
            WHERE merchant = $1
              AND timestamp > NOW() - INTERVAL '1 hour' * $2
            "#,
        )
        .bind(merchant)
        .bind(window_hours as i32)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(MerchantSummary {
            merchant: merchant.to_string(),
            window_hours,
            plans_created: summary.0 as u64,
            subscriptions_started: summary.1 as u64,
            subscriptions_canceled: summary.2 as u64,
            churn_rate: if summary.1 > 0 {
                summary.2 as f64 / summary.1 as f64
            } else {
                0.0
            },
        })
    }
}

#[derive(Debug, Serialize)]
pub struct MerchantSummary {
    pub merchant: String,
    pub window_hours: u32,
    pub plans_created: u64,
    pub subscriptions_started: u64,
    pub subscriptions_canceled: u64,
    pub churn_rate: f64,
}

/// Example main function to run the indexer
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = IndexerConfig {
        rpc_url: "https://api.devnet.solana.com".to_string(),
        program_id: "6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5".to_string(),
        database_url: "postgresql://user:pass@localhost/tally_indexer".to_string(),
        alert_webhook_url: Some("https://hooks.slack.com/services/YOUR/WEBHOOK/URL".to_string()),
    };

    let indexer = SpamDetectionIndexer::new(config).await?;

    info!("Spam detection indexer started");

    // In production, subscribe to transaction logs via WebSocket
    // and call indexer.index_transaction() for each relevant transaction

    Ok(())
}
```

---

## Alert Thresholds and Rules

### Alert Configuration Schema

**YAML Configuration Example:**

```yaml
# alert_rules.yaml
rules:
  merchant_plan_spam:
    enabled: true
    severity: critical
    metric: plans_created_per_hour
    threshold: 10
    window_secs: 3600
    actions:
      - log_alert
      - send_webhook
      - notify_operations

  subscriber_churn:
    enabled: true
    severity: warning
    metric: churn_rate
    threshold: 0.8
    minimum_subscriptions: 5
    window_secs: 3600
    actions:
      - log_alert
      - send_webhook

  cancellation_spam:
    enabled: true
    severity: info
    metric: cancellations_per_hour
    threshold: 10
    window_secs: 3600
    actions:
      - log_alert

  high_failure_rate:
    enabled: true
    severity: warning
    metric: failed_transaction_ratio
    threshold: 0.3
    window_secs: 900  # 15 minutes
    actions:
      - log_alert
      - send_webhook

  coordinated_activity:
    enabled: true
    severity: critical
    metric: simultaneous_operations
    threshold: 20
    window_secs: 60  # 1 minute
    actions:
      - log_alert
      - send_webhook
      - auto_throttle

webhooks:
  slack:
    url: "https://hooks.slack.com/services/YOUR/WEBHOOK"
    enabled: true

  pagerduty:
    integration_key: "YOUR_PAGERDUTY_KEY"
    enabled: false

  email:
    smtp_server: "smtp.example.com"
    from: "alerts@tallypay.click"
    to:
      - "ops@tallypay.click"
      - "security@tallypay.click"
    enabled: true
```

### TypeScript Alert Manager

```typescript
// alert_manager.ts
import { PrismaClient } from '@prisma/client';
import axios from 'axios';

interface AlertRule {
  name: string;
  enabled: boolean;
  severity: 'critical' | 'warning' | 'info';
  metric: string;
  threshold: number;
  windowSecs: number;
  actions: string[];
}

interface AlertEvent {
  ruleName: string;
  severity: string;
  account: string;
  metric: string;
  threshold: number;
  actual: number;
  timestamp: Date;
  evidence: string[];
}

export class AlertManager {
  private prisma: PrismaClient;
  private rules: Map<string, AlertRule>;
  private webhookUrl?: string;

  constructor(webhookUrl?: string) {
    this.prisma = new PrismaClient();
    this.rules = new Map();
    this.webhookUrl = webhookUrl;
  }

  loadRules(rules: AlertRule[]) {
    for (const rule of rules) {
      this.rules.set(rule.name, rule);
    }
  }

  async checkMerchantPlanRate(merchant: string): Promise<AlertEvent[]> {
    const rule = this.rules.get('merchant_plan_spam');
    if (!rule || !rule.enabled) return [];

    const oneHourAgo = new Date(Date.now() - rule.windowSecs * 1000);

    const count = await this.prisma.indexedOperation.count({
      where: {
        merchant,
        operationType: 'plan_created',
        timestamp: { gte: oneHourAgo },
      },
    });

    if (count > rule.threshold) {
      const alert: AlertEvent = {
        ruleName: rule.name,
        severity: rule.severity,
        account: merchant,
        metric: rule.metric,
        threshold: rule.threshold,
        actual: count,
        timestamp: new Date(),
        evidence: [
          `Created ${count} plans in last hour`,
          `Threshold: ${rule.threshold} plans/hour`,
        ],
      };

      await this.handleAlert(alert, rule);
      return [alert];
    }

    return [];
  }

  async checkSubscriberChurn(subscriber: string): Promise<AlertEvent[]> {
    const rule = this.rules.get('subscriber_churn');
    if (!rule || !rule.enabled) return [];

    const windowStart = new Date(Date.now() - rule.windowSecs * 1000);

    const [started, canceled] = await Promise.all([
      this.prisma.indexedOperation.count({
        where: {
          subscriber,
          operationType: 'subscription_started',
          timestamp: { gte: windowStart },
        },
      }),
      this.prisma.indexedOperation.count({
        where: {
          subscriber,
          operationType: 'subscription_canceled',
          timestamp: { gte: windowStart },
        },
      }),
    ]);

    if (started > 0) {
      const churnRate = canceled / started;

      if (churnRate > rule.threshold && started >= 5) {
        const alert: AlertEvent = {
          ruleName: rule.name,
          severity: rule.severity,
          account: subscriber,
          metric: rule.metric,
          threshold: rule.threshold,
          actual: churnRate,
          timestamp: new Date(),
          evidence: [
            `Started ${started} subscriptions`,
            `Canceled ${canceled} subscriptions`,
            `Churn rate: ${(churnRate * 100).toFixed(1)}%`,
          ],
        };

        await this.handleAlert(alert, rule);
        return [alert];
      }
    }

    return [];
  }

  private async handleAlert(alert: AlertEvent, rule: AlertRule) {
    // Log alert
    if (rule.actions.includes('log_alert')) {
      console.log(`[${alert.severity.toUpperCase()}] ${alert.ruleName}:`, alert);
    }

    // Send webhook
    if (rule.actions.includes('send_webhook') && this.webhookUrl) {
      await this.sendWebhook(alert);
    }

    // Store in database for audit trail
    await this.storeAlert(alert);
  }

  private async sendWebhook(alert: AlertEvent) {
    if (!this.webhookUrl) return;

    try {
      await axios.post(this.webhookUrl, {
        text: `🚨 *${alert.severity.toUpperCase()}*: ${alert.ruleName}`,
        blocks: [
          {
            type: 'section',
            text: {
              type: 'mrkdwn',
              text: `*Account:* ${alert.account}\n*Metric:* ${alert.metric}\n*Threshold:* ${alert.threshold}\n*Actual:* ${alert.actual}`,
            },
          },
          {
            type: 'section',
            text: {
              type: 'mrkdwn',
              text: `*Evidence:*\n${alert.evidence.map(e => `• ${e}`).join('\n')}`,
            },
          },
        ],
      });
    } catch (error) {
      console.error('Failed to send webhook:', error);
    }
  }

  private async storeAlert(alert: AlertEvent) {
    await this.prisma.spamAlert.create({
      data: {
        ruleName: alert.ruleName,
        severity: alert.severity,
        account: alert.account,
        metric: alert.metric,
        threshold: alert.threshold,
        actual: alert.actual,
        timestamp: alert.timestamp,
        evidence: JSON.stringify(alert.evidence),
      },
    });
  }

  async getRecentAlerts(hours: number = 24): Promise<AlertEvent[]> {
    const since = new Date(Date.now() - hours * 3600 * 1000);

    const alerts = await this.prisma.spamAlert.findMany({
      where: { timestamp: { gte: since } },
      orderBy: { timestamp: 'desc' },
    });

    return alerts.map(a => ({
      ruleName: a.ruleName,
      severity: a.severity as 'critical' | 'warning' | 'info',
      account: a.account,
      metric: a.metric,
      threshold: a.threshold,
      actual: a.actual,
      timestamp: a.timestamp,
      evidence: JSON.parse(a.evidence),
    }));
  }
}
```

---

## Real-Time Monitoring

### Grafana Dashboard Configuration

**Dashboard JSON (grafana_dashboard.json):**

```json
{
  "dashboard": {
    "title": "Tally Protocol Spam Detection",
    "panels": [
      {
        "title": "Plans Created per Hour",
        "targets": [
          {
            "expr": "rate(tally_plans_created_total[1h])"
          }
        ],
        "type": "graph"
      },
      {
        "title": "Subscription Churn Rate",
        "targets": [
          {
            "expr": "tally_subscriptions_canceled_total / tally_subscriptions_started_total"
          }
        ],
        "type": "stat"
      },
      {
        "title": "Top Merchants by Activity",
        "targets": [
          {
            "expr": "topk(10, sum by (merchant) (rate(tally_operations_total[1h])))"
          }
        ],
        "type": "table"
      }
    ]
  }
}
```

### Prometheus Metrics Exporter

```rust
// metrics_exporter.rs
use prometheus::{Encoder, Histogram, IntCounter, IntGauge, Registry, TextEncoder};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use warp::Filter;

pub struct MetricsExporter {
    registry: Registry,
    plans_created: IntCounter,
    subscriptions_started: IntCounter,
    subscriptions_canceled: IntCounter,
    operation_latency: Histogram,
}

impl MetricsExporter {
    pub fn new() -> Self {
        let registry = Registry::new();

        let plans_created = IntCounter::new(
            "tally_plans_created_total",
            "Total number of plans created",
        )
        .unwrap();
        registry.register(Box::new(plans_created.clone())).unwrap();

        let subscriptions_started = IntCounter::new(
            "tally_subscriptions_started_total",
            "Total number of subscriptions started",
        )
        .unwrap();
        registry.register(Box::new(subscriptions_started.clone())).unwrap();

        let subscriptions_canceled = IntCounter::new(
            "tally_subscriptions_canceled_total",
            "Total number of subscriptions canceled",
        )
        .unwrap();
        registry.register(Box::new(subscriptions_canceled.clone())).unwrap();

        let operation_latency = Histogram::new(
            "tally_operation_latency_seconds",
            "Operation processing latency",
        )
        .unwrap();
        registry.register(Box::new(operation_latency.clone())).unwrap();

        Self {
            registry,
            plans_created,
            subscriptions_started,
            subscriptions_canceled,
            operation_latency,
        }
    }

    pub fn record_plan_created(&self) {
        self.plans_created.inc();
    }

    pub fn record_subscription_started(&self) {
        self.subscriptions_started.inc();
    }

    pub fn record_subscription_canceled(&self) {
        self.subscriptions_canceled.inc();
    }

    pub fn record_latency(&self, duration_secs: f64) {
        self.operation_latency.observe(duration_secs);
    }

    /// Start HTTP server to expose metrics at /metrics
    pub async fn serve(self: Arc<Self>, port: u16) {
        let metrics_route = warp::path("metrics").map(move || {
            let encoder = TextEncoder::new();
            let metric_families = self.registry.gather();
            let mut buffer = Vec::new();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        });

        warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;
    }
}
```

---

## Automated Response Systems

### Auto-Throttling Implementation

```rust
// auto_throttle.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct ThrottleManager {
    throttled_accounts: Arc<Mutex<HashMap<String, ThrottleState>>>,
}

#[derive(Clone)]
struct ThrottleState {
    throttled_until: Instant,
    reason: String,
    severity: ThrottleSeverity,
}

#[derive(Clone, PartialEq)]
enum ThrottleSeverity {
    Soft,   // Reduced rate limits
    Hard,   // Blocked temporarily
    Banned, // Permanent block
}

impl ThrottleManager {
    pub fn new() -> Self {
        Self {
            throttled_accounts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if account is currently throttled
    pub fn is_throttled(&self, account: &str) -> bool {
        let mut throttled = self.throttled_accounts.lock().unwrap();

        if let Some(state) = throttled.get(account) {
            if Instant::now() < state.throttled_until {
                return true;
            } else {
                // Throttle period expired
                throttled.remove(account);
            }
        }

        false
    }

    /// Apply throttle to an account
    pub fn apply_throttle(
        &self,
        account: &str,
        duration: Duration,
        reason: String,
        severity: ThrottleSeverity,
    ) {
        let mut throttled = self.throttled_accounts.lock().unwrap();

        throttled.insert(
            account.to_string(),
            ThrottleState {
                throttled_until: Instant::now() + duration,
                reason,
                severity,
            },
        );
    }

    /// Auto-throttle based on spam alert
    pub fn auto_throttle_from_alert(&self, alert: &SpamAlert) {
        let (duration, severity) = match alert.severity {
            AlertSeverity::Critical => (Duration::from_secs(3600), ThrottleSeverity::Hard),
            AlertSeverity::Warning => (Duration::from_secs(900), ThrottleSeverity::Soft),
            AlertSeverity::Info => return, // Don't throttle for info alerts
        };

        self.apply_throttle(
            &alert.account,
            duration,
            format!("{}: {}", alert.alert_type, alert.metric),
            severity,
        );

        tracing::warn!(
            "Auto-throttled account {} for {:?} due to {}",
            alert.account,
            duration,
            alert.alert_type
        );
    }

    /// Get throttle status for account
    pub fn get_throttle_status(&self, account: &str) -> Option<(Duration, String)> {
        let throttled = self.throttled_accounts.lock().unwrap();

        throttled.get(account).map(|state| {
            let remaining = state.throttled_until.saturating_duration_since(Instant::now());
            (remaining, state.reason.clone())
        })
    }
}
```

---

## Integration Examples

### Complete Monitoring Service

```rust
// monitoring_service.rs
use tokio::time::{interval, Duration};

pub struct MonitoringService {
    indexer: SpamDetectionIndexer,
    alert_manager: AlertManager,
    throttle_manager: ThrottleManager,
    metrics_exporter: Arc<MetricsExporter>,
}

impl MonitoringService {
    pub async fn new(config: IndexerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let indexer = SpamDetectionIndexer::new(config).await?;
        let alert_manager = AlertManager::new(/* webhook URL */);
        let throttle_manager = ThrottleManager::new();
        let metrics_exporter = Arc::new(MetricsExporter::new());

        Ok(Self {
            indexer,
            alert_manager,
            throttle_manager,
            metrics_exporter,
        })
    }

    /// Start monitoring service
    pub async fn start(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        // Start metrics HTTP server
        let metrics_clone = self.metrics_exporter.clone();
        tokio::spawn(async move {
            metrics_clone.serve(9090).await;
        });

        // Run periodic spam checks
        let mut check_interval = interval(Duration::from_secs(60));

        loop {
            check_interval.tick().await;
            self.run_spam_checks().await?;
        }
    }

    async fn run_spam_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Get list of active merchants and subscribers from database
        // Run checks for each account

        // Example: Check all merchants
        let merchants = self.get_active_merchants().await?;

        for merchant in merchants {
            let alerts = self.alert_manager.check_merchant_plan_rate(&merchant).await?;

            for alert in alerts {
                // Auto-throttle if critical
                if alert.severity == "critical" {
                    self.throttle_manager.auto_throttle_from_alert(&alert);
                }
            }
        }

        Ok(())
    }

    async fn get_active_merchants(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // Query database for merchants with recent activity
        Ok(vec![]) // Placeholder
    }
}
```

---

## Deployment and Operations

### Docker Compose Setup

```yaml
# docker-compose.yml
version: '3.8'

services:
  postgres:
    image: timescale/timescaledb:latest-pg15
    environment:
      POSTGRES_DB: tally_indexer
      POSTGRES_USER: tally
      POSTGRES_PASSWORD: secure_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"

  indexer:
    build: ./indexer
    environment:
      RPC_URL: https://api.devnet.solana.com
      DATABASE_URL: postgresql://tally:secure_password@postgres/tally_indexer
      WEBHOOK_URL: https://hooks.slack.com/services/YOUR/WEBHOOK
    depends_on:
      - postgres

  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    ports:
      - "9091:9090"

  grafana:
    image: grafana/grafana:latest
    environment:
      GF_SECURITY_ADMIN_PASSWORD: admin
    volumes:
      - grafana_data:/var/lib/grafana
    ports:
      - "3000:3000"

volumes:
  postgres_data:
  prometheus_data:
  grafana_data:
```

### Kubernetes Deployment

```yaml
# k8s-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: tally-spam-indexer
spec:
  replicas: 3
  selector:
    matchLabels:
      app: tally-spam-indexer
  template:
    metadata:
      labels:
        app: tally-spam-indexer
    spec:
      containers:
      - name: indexer
        image: tallypay/spam-indexer:latest
        env:
        - name: RPC_URL
          value: "https://api.mainnet-beta.solana.com"
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: tally-secrets
              key: database-url
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
```

---

## Summary

This guide provides production-ready implementations for:

1. **Rust Indexer**: Real-time transaction monitoring with pattern detection
2. **TypeScript Alert Manager**: Flexible alerting system with configurable rules
3. **Metrics Exporter**: Prometheus integration for dashboard visualization
4. **Auto-Throttling**: Automated account restrictions based on spam detection
5. **Deployment**: Docker and Kubernetes configurations

**Next Steps:**
1. Deploy indexer infrastructure
2. Configure alert thresholds for your environment
3. Set up Grafana dashboards
4. Test with simulated spam scenarios
5. Integrate with incident response procedures

**Related Documentation:**
- [RATE_LIMITING_STRATEGY.md](./RATE_LIMITING_STRATEGY.md) - Overall strategy and economics
- [OPERATIONAL_PROCEDURES.md](./OPERATIONAL_PROCEDURES.md) - Incident response procedures

---

**Document Version:** 1.0
**Last Updated:** 2025-10-05
**Security Audit Reference:** M-6 - No Rate Limiting on Operations
