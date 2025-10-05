# Rate Limiting Strategy for Tally Protocol

## Executive Summary

Tally Protocol intentionally **does not implement on-chain rate limiting** for subscription operations. This document explains the rationale, economic constraints, and recommended off-chain mitigation strategies.

**Key Findings:**
- On-chain rate limiting requires state changes and account migrations
- Solana transaction fees provide economic spam deterrence
- Off-chain rate limiting is the industry-standard solution
- RPC-level and indexer-level controls are more effective and flexible

## Table of Contents

1. [Why No On-Chain Rate Limiting](#why-no-on-chain-rate-limiting)
2. [Economic Spam Deterrence](#economic-spam-deterrence)
3. [Off-Chain Rate Limiting Architecture](#off-chain-rate-limiting-architecture)
4. [Attack Scenarios and Mitigation](#attack-scenarios-and-mitigation)
5. [Recommendations](#recommendations)

---

## Why No On-Chain Rate Limiting

### Technical Constraints

**Account Size Limitations**

Implementing on-chain rate limiting would require adding timestamp and counter fields to existing account structures:

```rust
// Example: Hypothetical rate limiting fields
pub struct Merchant {
    // ... existing fields ...
    pub last_plan_created_ts: i64,     // +8 bytes
    pub plans_created_count: u16,      // +2 bytes
    pub rate_limit_window_start: i64,  // +8 bytes
}

pub struct Subscription {
    // ... existing fields ...
    pub cancellation_count: u16,       // +2 bytes
    pub last_cancellation_ts: i64,     // +8 bytes
}
```

**Critical Issue:** Adding fields to existing account structures requires **account migration** for all deployed instances, which is:
- Complex and error-prone
- Risks data loss or corruption
- Requires downtime or versioning complexity
- Increases ongoing maintenance burden

**Rent and Storage Costs**

Each rate limiting field increases account size and therefore rent costs:
- Additional 18 bytes per Merchant account: ~0.000125 SOL additional rent
- Additional 10 bytes per Subscription account: ~0.000069 SOL additional rent
- Multiplied across thousands of accounts = significant cumulative cost
- Passed on to users through higher transaction costs

### Design Philosophy

**Solana Best Practices**

Following Solana ecosystem patterns:
- Anchor programs keep state minimal and optimized
- Rate limiting is handled at infrastructure layers (RPC, validators)
- On-chain logic focuses on correctness, not abuse prevention
- Economic incentives (fees + rent) naturally limit spam

**Separation of Concerns**

- **On-Chain**: Business logic correctness and security
- **Off-Chain**: Resource management, rate limiting, monitoring
- **Infrastructure**: DDoS protection, network-level controls

---

## Economic Spam Deterrence

### Transaction Fee Economics

**Base Costs per Operation**

Solana transaction fees provide natural spam deterrence:

| Operation | Signature Fee | Rent Deposit | Total Minimum Cost |
|-----------|---------------|--------------|-------------------|
| `create_plan` | 0.000005 SOL (~$0.0007) | 0.00089 SOL (~$0.12) | ~$0.12 |
| `start_subscription` | 0.000005 SOL | 0.00078 SOL (~$0.11) | ~$0.11 |
| `cancel_subscription` | 0.000005 SOL | 0 SOL | ~$0.0007 |

**Note:** SOL prices are illustrative at $140/SOL. Actual costs vary with market conditions.

### Attack Cost Analysis

**Plan Spam Attack**

Creating 10,000 fake plans:
- Transaction fees: 10,000 × 0.000005 SOL = 0.05 SOL (~$7)
- Rent deposits: 10,000 × 0.00089 SOL = 8.9 SOL (~$1,246)
- **Total cost: ~$1,253**
- **Account space consumed:** 10,000 × 129 bytes = 1.29 MB

**Mitigation:** Rent deposits make large-scale plan spam economically prohibitive.

**Subscription Churn Attack**

Repeatedly starting and canceling the same subscription:
- Per cycle: 0.000005 SOL (start) + 0.000005 SOL (cancel) = 0.00001 SOL
- 1,000 churn cycles: 0.01 SOL (~$1.40)
- **Total cost for 1,000 cycles: ~$1.40**

**Mitigation:** Low individual cost, but requires:
- USDC balance for initial payment
- Delegate approval setup
- Off-chain monitoring can detect and block patterns

**Cancellation Spam Attack**

Canceling 1,000 subscriptions:
- Transaction fees: 1,000 × 0.000005 SOL = 0.005 SOL (~$0.70)
- **Total cost: ~$0.70**

**Mitigation:**
- Lowest cost attack vector
- Requires pre-existing subscriptions
- Easily detected by monitoring systems
- RPC rate limiting prevents high-frequency abuse

### Economic Deterrence Summary

**Key Insights:**
1. Rent deposits make account creation spam expensive (>$1,000 for 10K accounts)
2. Transaction fees prevent unlimited free operations
3. Operations requiring USDC transfers add additional friction
4. Costs scale linearly with abuse volume, making large-scale attacks prohibitive

**Limitations:**
- Low-volume spam remains economically feasible
- Cancellation spam is cheapest attack vector
- Economic deterrence alone is insufficient without monitoring

---

## Off-Chain Rate Limiting Architecture

### Multi-Layer Defense Strategy

```
User Request
    ↓
[1] RPC Node Rate Limiting
    ↓ (passed)
[2] Transaction Validation
    ↓ (passed)
[3] On-Chain Execution
    ↓
[4] Indexer Monitoring
    ↓
[5] Dashboard Anomaly Detection
```

### Layer 1: RPC-Level Rate Limiting

**Implementation:** Configure RPC node to limit requests per IP/account.

**Recommended Limits:**

| Metric | Limit | Window | Action |
|--------|-------|--------|--------|
| Requests per IP | 100 | 1 minute | Throttle |
| Transactions per Account | 50 | 1 minute | Throttle |
| Failed Transactions per IP | 20 | 5 minutes | Block (temporary) |
| Plan Creation per Merchant | 10 | 1 hour | Alert + Throttle |
| Subscription Operations per User | 20 | 1 hour | Alert + Throttle |

**Tools:**
- **Helius RPC**: Built-in rate limiting and priority fees
- **QuickNode**: Configurable per-method rate limits
- **Custom RPC**: HAProxy or Nginx rate limiting modules
- **Cloudflare**: Enterprise DDoS protection and rate limiting

**Example Nginx Configuration:**

```nginx
http {
    # Define rate limit zones
    limit_req_zone $binary_remote_addr zone=rpc_ip:10m rate=100r/m;
    limit_req_zone $http_x_account_pubkey zone=rpc_account:10m rate=50r/m;

    server {
        location /rpc {
            # Apply rate limits
            limit_req zone=rpc_ip burst=20 nodelay;
            limit_req zone=rpc_account burst=10 nodelay;

            # Forward to Solana RPC
            proxy_pass http://solana-rpc:8899;
        }
    }
}
```

### Layer 2: Transaction Validation

**Validation Checkpoints:**

1. **Pre-Flight Checks** (before submission)
   - Account state validation
   - Balance verification
   - Recent transaction history check

2. **Simulation Testing**
   - Simulate transaction before sending
   - Catch errors early to reduce spam
   - Validate against current on-chain state

**SDK Integration Example:**

```rust
// Pre-flight validation in tally-sdk
pub async fn validate_create_plan(
    client: &SimpleTallyClient,
    merchant_pubkey: &Pubkey,
    plan_id: &str,
) -> Result<(), SdkError> {
    // Check if plan already exists
    let plan_pda = pda::plan_address_from_string(merchant_pubkey, plan_id)?;
    if client.account_exists(&plan_pda).await? {
        return Err(SdkError::PlanAlreadyExists);
    }

    // Check recent plan creation rate (off-chain query)
    let recent_plans = client.get_merchant_recent_plans(merchant_pubkey, 3600).await?;
    if recent_plans.len() >= 10 {
        return Err(SdkError::RateLimitExceeded);
    }

    Ok(())
}
```

### Layer 3: Indexer Monitoring

**Purpose:** Detect anomalous patterns in historical data.

**Monitoring Dimensions:**

1. **Merchant Activity**
   - Plans created per hour/day
   - Subscription conversion rates
   - Failed transaction ratio

2. **Subscriber Behavior**
   - Subscription start/cancel frequency
   - Churn patterns
   - Cancellation clustering

3. **System-Wide Metrics**
   - Total operations per hour
   - Error rate trends
   - Geographic distribution

**Alert Thresholds:**

```yaml
alerts:
  critical:
    - metric: plans_created_per_merchant
      threshold: 100
      window: 1h
      action: investigate_immediately

    - metric: subscription_churn_rate
      threshold: 0.8  # 80% cancel within 1 hour
      window: 1h
      action: flag_merchant_account

  warning:
    - metric: failed_transaction_ratio
      threshold: 0.3  # 30% failures
      window: 15m
      action: alert_operations

    - metric: cancellations_per_subscriber
      threshold: 10
      window: 1h
      action: review_subscriber_account
```

**Implementation:** See [SPAM_DETECTION.md](./SPAM_DETECTION.md) for detailed indexer code examples.

### Layer 4: Dashboard Anomaly Detection

**Real-Time Visualization:**

- Merchant operation frequency charts
- Subscriber behavior heatmaps
- System health dashboards
- Anomaly detection alerts

**Machine Learning Integration (Advanced):**

```python
# Example: Anomaly detection using isolation forest
from sklearn.ensemble import IsolationForest
import pandas as pd

def detect_merchant_spam(merchant_metrics: pd.DataFrame) -> list:
    """
    Detect anomalous merchant behavior using ML.

    Features:
    - plans_created_per_hour
    - subscription_start_rate
    - cancellation_rate
    - average_plan_price
    """
    features = merchant_metrics[[
        'plans_created_per_hour',
        'subscription_start_rate',
        'cancellation_rate',
        'average_plan_price'
    ]]

    # Train isolation forest
    clf = IsolationForest(contamination=0.05, random_state=42)
    predictions = clf.fit_predict(features)

    # Return anomalous merchant IDs
    anomalies = merchant_metrics[predictions == -1]
    return anomalies['merchant_id'].tolist()
```

---

## Attack Scenarios and Mitigation

### Scenario 1: Plan Creation Spam

**Attack:** Malicious merchant creates 10,000 fake plans to bloat state.

**Economic Cost:**
- Rent: 10,000 × 0.00089 SOL = 8.9 SOL (~$1,246)
- Transaction fees: 10,000 × 0.000005 SOL = 0.05 SOL (~$7)
- **Total: ~$1,253**

**Detection:**
- RPC layer: Throttle after 10 plans/hour
- Indexer: Alert on >100 plans/day from single merchant
- Dashboard: Flag merchant for manual review

**Mitigation:**
1. **Immediate:** RPC blocks further requests from merchant
2. **Short-term:** Manual review of merchant account
3. **Long-term:** Machine learning model to predict spam merchants

**Prevention:**
- Require KYC for high-volume merchants
- Implement merchant reputation scoring
- Progressive limits (new merchants have lower thresholds)

### Scenario 2: Subscription Churn Attack

**Attack:** User repeatedly starts and cancels same subscription to generate noise.

**Economic Cost:**
- Per cycle: 0.00001 SOL (start + cancel) + USDC for initial payment
- 1,000 cycles: ~$1.40 in SOL + gas for USDC operations
- **Total: ~$2-5 (depending on USDC amount)**

**Detection:**
- Indexer: Track subscription lifetime duration
- Alert on subscriptions with lifetime <5 minutes
- Flag subscribers with >10 churn events per day

**Mitigation:**
1. **RPC Layer:** Limit subscription operations to 20/hour per account
2. **Application Layer:** Enforce minimum subscription lifetime (e.g., 1 hour cooldown)
3. **Reputation System:** Reduce limits for users with high churn rates

**Prevention:**
- Charge non-refundable setup fees
- Implement subscriber trust scores
- Require email verification for new subscribers

### Scenario 3: Cancellation Spam

**Attack:** Attacker cancels legitimate subscriptions (their own) repeatedly.

**Economic Cost:**
- Per cancellation: 0.000005 SOL (~$0.0007)
- 1,000 cancellations: ~$0.70
- **Cheapest attack vector**

**Detection:**
- Indexer: Count cancellations per subscriber per day
- Alert on >10 cancellations from single account
- Track cancellation velocity spikes

**Mitigation:**
1. **RPC Layer:** Limit cancellations to 5/hour per account
2. **Idempotency:** Cancellation is already idempotent (safe to call multiple times)
3. **Monitoring:** Alert operations team for manual review

**Impact Assessment:**
- **Low Impact:** Cancellation only affects attacker's own subscriptions
- **No State Bloat:** No new accounts created
- **Economic Deterrence:** Requires pre-existing subscriptions
- **Monitoring Priority:** Low (self-inflicted spam)

### Scenario 4: Distributed Attack (Sybil)

**Attack:** Attacker uses multiple accounts to bypass per-account rate limits.

**Economic Cost:**
- Account creation: Free (Solana accounts)
- SOL distribution: Manual effort to fund each account
- Coordination overhead: Significant

**Detection:**
- **IP Clustering:** Detect operations from same IP range
- **Timing Analysis:** Identify coordinated timestamp patterns
- **Behavior Fingerprinting:** Similar operation sequences across accounts
- **Graph Analysis:** Network of related accounts

**Mitigation:**
1. **RPC Layer:** IP-based rate limiting
2. **Application Layer:** CAPTCHA for high-volume operations
3. **Blockchain Analysis:** Track SOL funding sources
4. **Heuristic Scoring:** Assign trust scores based on account age, activity, etc.

**Advanced Detection Example:**

```python
def detect_sybil_cluster(transactions: list) -> list:
    """
    Detect coordinated Sybil accounts based on:
    - Transaction timing correlation
    - Shared funding sources
    - Similar operation patterns
    """
    clusters = []

    # Group transactions by 5-minute windows
    time_windows = group_by_time_window(transactions, window_secs=300)

    for window in time_windows:
        # Check for >10 similar operations in same window
        if len(window) > 10:
            # Analyze funding source correlation
            funding_sources = [get_funding_source(tx) for tx in window]
            if len(set(funding_sources)) < 3:
                # High correlation = likely Sybil cluster
                clusters.append({
                    'accounts': [tx.signer for tx in window],
                    'confidence': 0.85,
                    'evidence': 'coordinated_timing_shared_funding'
                })

    return clusters
```

---

## Recommendations

### Immediate Actions (Week 1)

1. **Enable RPC Rate Limiting**
   - Configure per-IP limits: 100 requests/minute
   - Configure per-account limits: 50 transactions/minute
   - Set up basic alerting for threshold breaches

2. **Deploy Basic Monitoring**
   - Set up indexer for operation counting
   - Create dashboard for merchant/subscriber metrics
   - Configure email alerts for anomalies

3. **Document Procedures**
   - Create runbooks for spam incident response
   - Define escalation paths
   - Train operations team

### Short-Term Improvements (Month 1)

1. **Enhanced Indexer Analytics**
   - Implement pattern detection algorithms
   - Build historical baseline metrics
   - Create anomaly detection models

2. **Application-Layer Controls**
   - Add SDK-level pre-flight validation
   - Implement client-side rate limit warnings
   - Build merchant self-service analytics

3. **Incident Response Automation**
   - Auto-throttle high-volume accounts
   - Automated alerting with context
   - Integration with incident management system

### Long-Term Strategy (Quarter 1)

1. **Machine Learning Integration**
   - Train spam detection models
   - Behavioral fingerprinting
   - Predictive risk scoring

2. **Reputation Systems**
   - Merchant trust scores
   - Subscriber reputation tracking
   - Progressive rate limit adjustments

3. **Advanced Infrastructure**
   - Multi-region RPC deployment
   - DDoS protection (Cloudflare Enterprise)
   - Real-time graph analysis for Sybil detection

### Operational Best Practices

1. **Monitoring Hygiene**
   - Review alerts daily
   - Tune thresholds monthly based on false positive rates
   - Document all spam incidents for pattern analysis

2. **Escalation Procedures**
   - Define clear ownership for spam response
   - Establish SLAs for incident response
   - Create communication templates for affected users

3. **Continuous Improvement**
   - Quarterly security review
   - Update threat models based on observed attacks
   - Community bug bounty for novel attack vectors

---

## Conclusion

**Tally Protocol's approach to rate limiting prioritizes:**

1. **Economic Deterrence:** Solana's fee and rent model naturally limits spam
2. **Off-Chain Flexibility:** Infrastructure-layer controls are more adaptable than on-chain logic
3. **Operational Excellence:** Monitoring and response capabilities over restrictive on-chain limits
4. **User Experience:** Avoid on-chain complexity that increases transaction costs

**This strategy aligns with Solana ecosystem best practices** where rate limiting and abuse prevention are handled at the infrastructure and application layers, allowing on-chain programs to remain lean, efficient, and focused on correctness.

**For implementation guidance, see:**
- [SPAM_DETECTION.md](./SPAM_DETECTION.md) - Detailed monitoring code examples
- [OPERATIONAL_PROCEDURES.md](./OPERATIONAL_PROCEDURES.md) - Incident response procedures

---

## References

- [Solana Transaction Fees Documentation](https://docs.solana.com/transaction_cost)
- [Anchor Account Size Best Practices](https://www.anchor-lang.com/docs/space)
- [Helius RPC Rate Limiting](https://docs.helius.dev/introduction/rate-limits)
- [QuickNode Enterprise Features](https://www.quicknode.com/docs/enterprise)

---

**Document Version:** 1.0
**Last Updated:** 2025-10-05
**Security Audit Reference:** M-6 - No Rate Limiting on Operations
