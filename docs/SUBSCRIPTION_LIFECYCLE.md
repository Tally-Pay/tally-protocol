# Subscription Lifecycle Management

This document provides a comprehensive guide to subscription lifecycle management in the Tally Protocol, with special attention to renewal count semantics, reactivation behavior, and off-chain integration patterns.

## Table of Contents

1. [Overview](#overview)
2. [Subscription States](#subscription-states)
3. [Renewal Count Semantics](#renewal-count-semantics)
4. [Reactivation Behavior](#reactivation-behavior)
5. [Off-Chain Integration Guide](#off-chain-integration-guide)
6. [Event Monitoring](#event-monitoring)
7. [Best Practices](#best-practices)

## Overview

The Tally Protocol implements a subscription management system where subscriptions can progress through multiple lifecycle stages: creation, active renewal cycles, cancellation, and reactivation. A key design decision is that the `renewals` counter tracks **lifetime renewals across all sessions**, not just the current active session.

This intentional design provides a complete historical record of the subscriber-merchant relationship, which is valuable for:

- Loyalty programs and rewards based on total subscription lifetime
- Analytics on long-term customer engagement
- Business intelligence on churn and reactivation patterns
- Tiered benefits based on cumulative subscription duration

## Subscription States

A subscription account can exist in the following states:

### New Subscription
- `active = true`
- `renewals = 0`
- `created_ts` = current timestamp
- `next_renewal_ts` = current timestamp + period

### Active Subscription
- `active = true`
- `renewals` increments with each successful renewal
- `next_renewal_ts` advances by `period_secs` after each renewal

### Canceled Subscription
- `active = false`
- `renewals` preserved (not reset)
- `created_ts` preserved
- All other fields remain unchanged

### Reactivated Subscription
- `active = true` (reset)
- `renewals` **preserved from previous session**
- `created_ts` **preserved from original creation**
- `next_renewal_ts` = reactivation timestamp + period (reset)
- `last_renewed_ts` = reactivation timestamp (reset)
- `last_amount` = current plan price (may change between sessions)

## Renewal Count Semantics

The `renewals` field in the `Subscription` account has specific semantics that are critical to understand for both on-chain and off-chain systems.

### Definition

**Lifetime Renewals**: The `renewals` counter represents the **cumulative number of successful renewal payments** across the **entire lifetime** of the subscription relationship, including all sessions (initial subscription + any reactivations).

### Behavior Across Lifecycle

| Event | renewals Behavior | Rationale |
|-------|------------------|-----------|
| New Subscription | Initialized to `0` | Starting point for a new relationship |
| Each Renewal | Incremented by `1` | Tracks each successful billing cycle |
| Cancellation | **Preserved** (not reset) | Maintains historical record |
| Reactivation | **Preserved** (continues counting) | Tracks lifetime relationship |

### Example Lifecycle

```rust
// Initial subscription start
Subscription {
    plan: plan_pubkey,
    subscriber: user_pubkey,
    active: true,
    renewals: 0,           // Starting count
    created_ts: 1704067200, // 2024-01-01
    next_renewal_ts: 1706745600, // 2024-02-01
    last_renewed_ts: 1704067200,
}

// After 10 successful renewals (10 months)
Subscription {
    active: true,
    renewals: 10,          // 10 billing cycles completed
    created_ts: 1704067200, // Original timestamp preserved
    next_renewal_ts: 1730419200, // 2024-11-01
}

// User cancels subscription
Subscription {
    active: false,         // Marked inactive
    renewals: 10,          // Historical count preserved
    created_ts: 1704067200,
    next_renewal_ts: 1730419200, // Last scheduled renewal (no longer processed)
}

// User reactivates 6 months later (2025-05-01)
Subscription {
    active: true,          // Reactivated
    renewals: 10,          // PRESERVED - not reset to 0
    created_ts: 1704067200, // Original creation timestamp preserved
    next_renewal_ts: 1746057600, // New renewal date: 2025-06-01
    last_renewed_ts: 1743465600, // Reactivation timestamp: 2025-05-01
    last_amount: plan.price_usdc, // Updated to current plan price
}

// After 5 more renewals in the reactivated session
Subscription {
    active: true,
    renewals: 15,          // Cumulative: 10 (first session) + 5 (second session)
    created_ts: 1704067200, // Still the original timestamp
    next_renewal_ts: 1759276800, // 2025-11-01
}
```

## Reactivation Behavior

When a previously canceled subscription is reactivated via `start_subscription`, the system follows a specific pattern for field preservation vs. reset.

### Fields Preserved Across Reactivation

These fields maintain their historical values to provide a complete record:

1. **`created_ts`**: Original subscription creation timestamp
   - **Why**: Tracks the start of the subscriber-merchant relationship
   - **Use Case**: Calculate total relationship duration, anniversary rewards

2. **`renewals`**: Cumulative renewal count across all sessions
   - **Why**: Maintains complete billing history for analytics and loyalty programs
   - **Use Case**: Tier-based rewards, lifetime value calculations

3. **`bump`**: PDA derivation seed
   - **Why**: Immutable property of the account's address derivation
   - **Use Case**: Account lookup and validation

### Fields Reset on Reactivation

These fields are updated to reflect the new billing cycle:

1. **`active`**: Set to `true`
   - **Why**: Enables renewal processing for the reactivated subscription
   - **Use Case**: Renewal eligibility checks

2. **`next_renewal_ts`**: Current timestamp + period
   - **Why**: Schedules the next billing cycle from reactivation time
   - **Use Case**: Renewal scheduling and processing

3. **`last_amount`**: Current plan price
   - **Why**: Plan pricing may have changed since cancellation
   - **Use Case**: Billing amount for upcoming renewals

4. **`last_renewed_ts`**: Reactivation timestamp
   - **Why**: Prevents immediate double-billing after reactivation
   - **Use Case**: Renewal eligibility validation

### Reactivation Event

When a subscription is reactivated, the program emits a `SubscriptionReactivated` event with additional context:

```rust
#[event]
pub struct SubscriptionReactivated {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub total_renewals: u32,        // Current renewals count (preserved)
    pub original_created_ts: i64,   // Original creation timestamp
}
```

This event provides off-chain systems with all the context needed to:
- Distinguish reactivations from new subscriptions
- Track historical renewal counts
- Calculate total subscription relationship duration

## Off-Chain Integration Guide

Off-chain systems (indexers, analytics, frontends) must account for the renewal count preservation behavior when tracking subscription metrics.

### Tracking Current Session Renewals

To track renewals within the current active session (excluding previous sessions), off-chain systems should maintain session state:

#### Database Schema Example (PostgreSQL)

```sql
-- Subscription sessions table
CREATE TABLE subscription_sessions (
    id SERIAL PRIMARY KEY,
    subscription_account BYTEA NOT NULL,  -- On-chain subscription PDA
    plan_account BYTEA NOT NULL,
    subscriber_pubkey BYTEA NOT NULL,
    session_start_ts BIGINT NOT NULL,     -- Timestamp when session started
    session_end_ts BIGINT,                -- NULL for active sessions
    renewals_at_start INT NOT NULL,       -- renewals count when session started
    renewals_at_end INT,                  -- renewals count when session ended
    is_active BOOLEAN DEFAULT TRUE,

    CONSTRAINT unique_active_session
        UNIQUE (subscription_account)
        WHERE is_active = TRUE
);

-- Index for fast lookups
CREATE INDEX idx_subscription_sessions_account
    ON subscription_sessions(subscription_account);

CREATE INDEX idx_subscription_sessions_active
    ON subscription_sessions(is_active)
    WHERE is_active = TRUE;
```

#### Indexer Logic (TypeScript/JavaScript)

```typescript
import { Connection, PublicKey } from '@solana/web3.js';
import { Program, AnchorProvider } from '@coral-xyz/anchor';

interface SubscriptionSession {
    subscriptionAccount: PublicKey;
    planAccount: PublicKey;
    subscriberPubkey: PublicKey;
    sessionStartTs: number;
    sessionEndTs: number | null;
    renewalsAtStart: number;
    renewalsAtEnd: number | null;
    isActive: boolean;
}

class SubscriptionIndexer {
    constructor(
        private program: Program,
        private db: Database // Your database client
    ) {}

    /**
     * Handle new subscription event
     * Creates a new session starting from renewals = 0
     */
    async handleSubscribed(event: any) {
        const subscription = await this.program.account.subscription.fetch(
            event.subscription
        );

        await this.db.subscriptionSessions.create({
            subscriptionAccount: event.subscription,
            planAccount: event.plan,
            subscriberPubkey: event.subscriber,
            sessionStartTs: subscription.createdTs,
            sessionEndTs: null,
            renewalsAtStart: 0,  // New subscriptions start at 0
            renewalsAtEnd: null,
            isActive: true,
        });
    }

    /**
     * Handle subscription reactivation event
     * Creates a new session starting from current renewals count
     */
    async handleReactivated(event: any) {
        const subscription = await this.program.account.subscription.fetch(
            event.subscription
        );

        // Close previous session (if exists)
        await this.db.subscriptionSessions.updateWhere(
            { subscriptionAccount: event.subscription, isActive: true },
            {
                isActive: false,
                sessionEndTs: event.timestamp,
                renewalsAtEnd: event.totalRenewals, // Preserved count
            }
        );

        // Create new session starting from preserved renewals count
        await this.db.subscriptionSessions.create({
            subscriptionAccount: event.subscription,
            planAccount: event.plan,
            subscriberPubkey: event.subscriber,
            sessionStartTs: event.timestamp,
            sessionEndTs: null,
            renewalsAtStart: event.totalRenewals,  // Preserved from previous session
            renewalsAtEnd: null,
            isActive: true,
        });
    }

    /**
     * Handle subscription cancellation event
     * Closes the current active session
     */
    async handleCanceled(event: any) {
        const subscription = await this.program.account.subscription.fetch(
            event.subscription
        );

        await this.db.subscriptionSessions.updateWhere(
            { subscriptionAccount: event.subscription, isActive: true },
            {
                isActive: false,
                sessionEndTs: event.timestamp,
                renewalsAtEnd: subscription.renewals,
            }
        );
    }

    /**
     * Handle renewal event
     * No session state change needed - renewals increment automatically
     */
    async handleRenewed(event: any) {
        // Session tracking: No action needed
        // The renewals count on-chain increments automatically

        // Optional: Update analytics/metrics
        await this.updateRenewalMetrics(event);
    }

    /**
     * Get current session renewals (excludes previous sessions)
     */
    async getCurrentSessionRenewals(
        subscriptionAccount: PublicKey
    ): Promise<number> {
        const subscription = await this.program.account.subscription.fetch(
            subscriptionAccount
        );

        const session = await this.db.subscriptionSessions.findOne({
            subscriptionAccount: subscriptionAccount,
            isActive: true,
        });

        if (!session) {
            return 0; // No active session
        }

        // Current session renewals = total renewals - renewals at session start
        return subscription.renewals - session.renewalsAtStart;
    }

    /**
     * Get lifetime statistics for a subscription
     */
    async getLifetimeStats(
        subscriptionAccount: PublicKey
    ): Promise<{
        totalRenewals: number;
        totalSessions: number;
        currentSessionRenewals: number;
        relationshipDurationDays: number;
    }> {
        const subscription = await this.program.account.subscription.fetch(
            subscriptionAccount
        );

        const sessions = await this.db.subscriptionSessions.findAll({
            subscriptionAccount: subscriptionAccount,
        });

        const currentSession = sessions.find(s => s.isActive);
        const currentSessionRenewals = currentSession
            ? subscription.renewals - currentSession.renewalsAtStart
            : 0;

        const now = Math.floor(Date.now() / 1000);
        const relationshipDurationDays = Math.floor(
            (now - subscription.createdTs) / 86400
        );

        return {
            totalRenewals: subscription.renewals,
            totalSessions: sessions.length,
            currentSessionRenewals,
            relationshipDurationDays,
        };
    }
}
```

#### GraphQL Schema Example

```graphql
type Subscription {
  account: PublicKey!
  plan: PublicKey!
  subscriber: PublicKey!
  active: Boolean!

  # On-chain fields
  totalRenewals: Int!           # Lifetime renewals (from on-chain)
  createdTs: Timestamp!         # Original creation timestamp
  nextRenewalTs: Timestamp!
  lastAmount: BigInt!

  # Off-chain computed fields
  currentSession: SubscriptionSession
  allSessions: [SubscriptionSession!]!
  lifetimeStats: LifetimeStats!
}

type SubscriptionSession {
  id: ID!
  subscriptionAccount: PublicKey!
  sessionStartTs: Timestamp!
  sessionEndTs: Timestamp
  renewalsAtStart: Int!
  renewalsAtEnd: Int
  isActive: Boolean!

  # Computed field
  sessionRenewals: Int!         # renewalsAtEnd - renewalsAtStart (or current - start)
}

type LifetimeStats {
  totalRenewals: Int!           # From on-chain renewals field
  totalSessions: Int!           # Count of all sessions
  currentSessionRenewals: Int!  # Renewals in active session
  relationshipDurationDays: Int!
}

type Query {
  subscription(account: PublicKey!): Subscription

  # Get current session renewals only
  currentSessionRenewals(account: PublicKey!): Int!

  # Get lifetime statistics
  lifetimeStats(account: PublicKey!): LifetimeStats!
}
```

#### GraphQL Query Examples

```graphql
# Get full subscription details with session breakdown
query GetSubscriptionDetails($account: PublicKey!) {
  subscription(account: $account) {
    account
    subscriber
    active
    totalRenewals
    createdTs

    currentSession {
      sessionStartTs
      renewalsAtStart
      sessionRenewals
    }

    lifetimeStats {
      totalRenewals
      totalSessions
      currentSessionRenewals
      relationshipDurationDays
    }
  }
}

# Get only current session renewals
query GetCurrentSessionRenewals($account: PublicKey!) {
  currentSessionRenewals(account: $account)
}

# Get all historical sessions for analytics
query GetSubscriptionHistory($account: PublicKey!) {
  subscription(account: $account) {
    allSessions {
      sessionStartTs
      sessionEndTs
      renewalsAtStart
      renewalsAtEnd
      sessionRenewals
      isActive
    }
  }
}
```

## Event Monitoring

To properly track subscription lifecycle, monitor these events:

### Subscribed Event
Emitted when a **new** subscription is created (first time):

```rust
#[event]
pub struct Subscribed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
}
```

**Indexer Action**: Create new subscription session with `renewals_at_start = 0`

### SubscriptionReactivated Event
Emitted when a previously canceled subscription is reactivated:

```rust
#[event]
pub struct SubscriptionReactivated {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub total_renewals: u32,        // Preserved count from previous sessions
    pub original_created_ts: i64,   // Original creation timestamp
}
```

**Indexer Action**:
1. Close previous session (if tracked)
2. Create new session with `renewals_at_start = total_renewals`

### Renewed Event
Emitted on each successful renewal payment:

```rust
#[event]
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
}
```

**Indexer Action**: Update renewal metrics (the on-chain `renewals` counter increments automatically)

### Canceled Event
Emitted when a subscription is canceled:

```rust
#[event]
pub struct Canceled {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
}
```

**Indexer Action**: Close current session with final `renewals_at_end`

### SubscriptionClosed Event
Emitted when a subscription account is permanently closed and rent reclaimed:

```rust
#[event]
pub struct SubscriptionClosed {
    pub plan: Pubkey,
    pub subscriber: Pubkey,
}
```

**Indexer Action**: Mark all sessions as terminated, subscription account no longer exists

## Best Practices

### For Protocol Integrators

1. **Distinguish Session vs. Lifetime Renewals**
   - Use the on-chain `renewals` field for lifetime metrics
   - Track session start/end off-chain to calculate per-session renewals
   - Never assume `renewals = 0` means a new subscriber

2. **Event-Driven Indexing**
   - Monitor `SubscriptionReactivated` events to detect session boundaries
   - Use `total_renewals` from events to maintain accurate session tracking
   - Handle out-of-order events gracefully (use timestamps)

3. **Analytics Queries**
   - For loyalty programs: Use lifetime `renewals` count
   - For churn analysis: Track session duration and count
   - For revenue forecasting: Use current session renewals

4. **User Experience**
   - Show both "Current Streak" (session renewals) and "Lifetime Payments" (total renewals)
   - Display original subscription date from `created_ts`
   - Highlight reactivation milestones in user dashboards

### For Application Developers

1. **Frontend Display**
   ```typescript
   // Good: Show both metrics clearly
   const displayMetrics = {
       lifetimeRenewals: subscription.renewals,
       currentStreak: getCurrentSessionRenewals(subscription),
       memberSince: new Date(subscription.createdTs * 1000),
   };
   ```

2. **Business Logic**
   - Use lifetime renewals for tier-based benefits
   - Use session renewals for streak-based rewards
   - Account for reactivation in churn prediction models

3. **Testing**
   - Test reactivation flows explicitly
   - Verify renewal counts persist across cancel/reactivate cycles
   - Validate event emissions and indexer state updates

### For Smart Contract Developers

1. **Field Documentation**
   - Clearly document which fields are preserved vs. reset
   - Explain the rationale for preservation (historical record)
   - Provide lifecycle examples in comments

2. **Event Design**
   - Include historical context in reactivation events
   - Emit events that enable session boundary detection
   - Provide sufficient data for off-chain reconstruction

3. **Testing**
   - Test multi-session lifecycle scenarios
   - Verify field preservation across reactivation
   - Validate event data includes all necessary context

## Summary

The Tally Protocol's subscription lifecycle design intentionally preserves renewal counts across reactivation cycles to maintain a complete historical record of subscriber-merchant relationships. This design enables:

- **Accurate lifetime value tracking** for business analytics
- **Loyalty programs** based on cumulative subscription duration
- **Churn and reactivation insights** through session analysis
- **Flexible off-chain session tracking** for current streak metrics

Off-chain systems must account for this behavior by:
1. Monitoring `SubscriptionReactivated` events to detect session boundaries
2. Tracking session start/end with associated renewal counts
3. Computing current session renewals as: `current_renewals - renewals_at_session_start`
4. Maintaining separate metrics for lifetime vs. session-based analytics

This approach provides the best of both worlds: complete on-chain historical records and flexible off-chain session tracking for diverse business needs.
