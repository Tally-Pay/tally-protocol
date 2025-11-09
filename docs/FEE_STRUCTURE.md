# Fee Structure - Tally Recurring Payments Protocol

**Version:** 2.0.0
**Last Updated:** 2025-11-08

---

## Overview

Tally's fee structure is designed to enable **economically viable hierarchical payment architectures** while maintaining protocol sustainability through volume-based revenue. The platform supports multi-level payment structures (company → department → employee → vendor) at competitive rates.

**Key Design Principles:**
- Low base fees enable hierarchical structures (0.25% base)
- Volume-based discounts reward growth (0.15-0.25%)
- Extensions add value-specific fees (0.3-0.6%)
- Total overhead remains competitive (<2% for 3-level hierarchy)

---

## Fee Components

### 1. Platform Base Fee

**Rate:** 0.15-0.25% (15-25 basis points)
**Determined by:** 30-day rolling volume tier

| Volume Tier | Monthly Volume | Platform Fee | Use Case |
|-------------|----------------|--------------|----------|
| **Standard** | Up to $10K | 0.25% (25 bps) | Small merchants, individuals |
| **Growth** | $10K - $100K | 0.20% (20 bps) | Growing businesses, teams |
| **Scale** | Over $100K | 0.15% (15 bps) | Enterprises, DAOs, funds |

**Automatic Upgrades:** Tiers upgrade automatically when 30-day rolling volume crosses thresholds.

**Example:**
```
Payment: $100 USDC
Standard tier (0.25%): $0.25 platform fee
Growth tier (0.20%): $0.20 platform fee
Scale tier (0.15%): $0.15 platform fee
```

### 2. Keeper Fee

**Rate:** 0.15% (15 basis points)
**Purpose:** Compensates off-chain workers who execute scheduled payments

The keeper fee incentivizes a decentralized network of payment executors while remaining low enough for hierarchical structures.

**Example:**
```
Payment: $100 USDC
Keeper fee (0.15%): $0.15
```

### 3. Extension Fees (Optional)

Extensions add use-case-specific features and charge independently. Extensions keep 100% of their fees.

**Typical Extension Fee Ranges:**

| Extension | Fee Range | Features Provided |
|-----------|-----------|-------------------|
| **Subscription** | 0.4-0.6% | Free trials, grace periods, plan management, reactivation |
| **Payroll** | 0.3-0.5% | Tax calculations, split payments, time tracking, benefits |
| **Investment** | 0.2-0.4% | Vesting schedules, cliff periods, milestones, governance |
| **Grant** | 0.3-0.5% | Milestone validation, reporting, compliance, KYC |

**Example (Subscription Extension at 0.5%):**
```
Payment: $100 USDC
Platform fee (0.25%): $0.25
Keeper fee (0.15%): $0.15
Extension fee (0.50%): $0.50
Total: $0.90 (0.90% of payment)
```

---

## Total Fee Calculations

### Single-Level Payment

**Scenario:** User pays merchant directly

```
Payment: $100 USDC

Core Protocol:
  Platform fee (0.25%): $0.25
  Keeper fee (0.15%): $0.15
  Subtotal: $0.40 (0.40%)

With Subscription Extension (0.50%):
  Extension fee: $0.50
  Total: $0.90 (0.90%)

Merchant receives: $99.10
```

### 3-Level Hierarchical Structure

**Scenario:** Company → Department → Employee payments

```
Level 1: Company pays Department ($1,000)
  Platform (0.25%): $2.50
  Keeper (0.15%): $1.50
  Total: $4.00 (0.40%)

Level 2: Department pays Employee ($500)
  Platform (0.25%): $1.25
  Keeper (0.15%): $0.75
  Total: $2.00 (0.40%)

Level 3: Employee pays Vendor ($100)
  Platform (0.25%): $0.25
  Keeper (0.15%): $0.15
  Total: $0.40 (0.40%)

Total overhead: $6.40 (1.20% cumulative)
```

### 4-Level Hierarchical Structure with Volume Discounts

**Scenario:** Investor → Company → Department → Employee
**Assumption:** Top levels qualify for volume discounts

```
Level 1: Investor → Company ($100,000)
  Platform (0.15% - Scale tier): $150
  Keeper (0.15%): $150
  Total: $300 (0.30%)

Level 2: Company → Department ($10,000)
  Platform (0.20% - Growth tier): $20
  Keeper (0.15%): $15
  Total: $35 (0.35%)

Level 3: Department → Employee ($1,000)
  Platform (0.25% - Standard tier): $2.50
  Keeper (0.15%): $1.50
  Total: $4.00 (0.40%)

Level 4: Employee → Vendor ($100)
  Platform (0.25%): $0.25
  Keeper (0.15%): $0.15
  Total: $0.40 (0.40%)

Total overhead: $339.40 (1.45% cumulative on final $100)
```

---

## Comparison with Traditional Payment Processors

### Single Payment

| Processor | Fee Structure | $100 Payment Cost |
|-----------|---------------|-------------------|
| **Stripe** | 2.9% + $0.30 | $3.20 |
| **PayPal** | 2.9% + $0.30 | $3.20 |
| **Square** | 2.6% + $0.10 | $2.70 |
| **Tally (basic)** | 0.40% | $0.40 |
| **Tally (with extension)** | 0.90% | $0.90 |

**Savings:** 72-88% cheaper than traditional processors

### 3-Level Hierarchy

| Structure | Total Overhead | Viable? |
|-----------|----------------|---------|
| **Traditional (3× 2.9%)** | 8.7% + fees | ❌ Not economically viable |
| **Tally (3× 0.40%)** | 1.20% | ✅ Viable |
| **Tally with extensions (3× 0.90%)** | 2.70% | ✅ Still competitive |

---

## Volume Tier Mechanics

### How Volume Tracking Works

1. **30-Day Rolling Window:** Volume is calculated over the most recent 30 days
2. **Automatic Upgrades:** Tiers upgrade automatically when thresholds are crossed
3. **Volume Reset:** After 30 days without payments, volume resets to zero
4. **Tier Downgrades:** Tier automatically downgrades if volume drops below threshold

### Upgrade Example

```
Day 1-15: Process $5,000 → Standard tier (0.25%)
Day 16: Process $6,000 → Total $11K → Upgraded to Growth tier (0.20%)
Day 30: All future payments use 0.20% until volume drops below $10K
```

### Volume Event

When a tier upgrade occurs, the `VolumeTierUpgraded` event is emitted:

```rust
pub struct VolumeTierUpgraded {
    pub merchant: Pubkey,
    pub old_tier: VolumeTier,        // Standard
    pub new_tier: VolumeTier,        // Growth
    pub monthly_volume_usdc: u64,    // 11,000,000,000 (11K USDC)
    pub new_platform_fee_bps: u16,   // 20 (0.20%)
}
```

---

## Revenue Projections

### Protocol Revenue by Volume

| Monthly Volume | Standard (0.25%) | Growth (0.20%) | Scale (0.15%) |
|----------------|------------------|----------------|---------------|
| $10K | $25 | - | - |
| $50K | - | $100 | - |
| $100K | - | $200 | $150 |
| $1M | - | - | $1,500 |
| $10M | - | - | $15,000 |
| $100M | - | - | $150,000 |
| $1B | - | - | $1,500,000 |

### Extension Developer Revenue (0.5% fee example)

| Monthly Volume | Extension Revenue |
|----------------|-------------------|
| $10K | $50 |
| $50K | $250 |
| $100K | $500 |
| $1M | $5,000 |
| $10M | $50,000 |
| $100M | $500,000 |

**Extension developers keep 100% of their fees** - no revenue share with protocol.

---

## Use Case Economics

### 1. Subscription Platform

**Fee Breakdown:**
- Platform: 0.25%
- Keeper: 0.15%
- Subscription extension: 0.50%
- **Total: 0.90%**

**$10/month subscription:**
- Merchant receives: $9.91
- Total fees: $0.09

**Comparison:**
- Stripe ($10): Merchant receives $9.41 (loses $0.59)
- Tally ($10): Merchant receives $9.91 (loses $0.09)
- **Merchant saves:** $0.50 per subscriber per month

### 2. Multi-Merchant User

**Scenario:** User subscribes to 5 merchants at $10/month each

**Traditional (separate accounts):**
- Total cost: $50
- Fees: 5 × $0.59 = $2.95
- User pays: $50
- Hassle: Manage 5 separate USDC accounts

**Tally (single account):**
- Total cost: $50
- Fees: 5 × $0.09 = $0.45
- User pays: $50
- Convenience: Single USDC account
- **Savings: $2.50/month**

### 3. Hierarchical Payroll

**Scenario:** Company with 10 employees, pays biweekly

**Structure:**
```
Company → Payroll Processor → Employees (10)
```

**Traditional (11 separate payments):**
- 11 × $0.30 fixed fee = $3.30 minimum
- Plus percentage fees: ~2.9%
- **Not economically viable for small payrolls**

**Tally:**
- Level 1 (Company → Processor): 0.40% on total payroll
- Level 2 (Processor → each employee): 0.40% on individual payment
- **Total overhead: 0.80%**
- No minimum fees

**$10,000 biweekly payroll:**
- Traditional: ~$293+ in fees
- Tally: ~$80 in fees
- **Savings: $213 per pay period ($5,538/year)**

### 4. DAO Treasury Management

**Scenario:** DAO pays 20 working groups monthly

**Tally Benefits:**
- Single treasury approves delegate once
- Automatic monthly payments to all groups
- Can pause individual groups without affecting others
- On-chain transparency and audit trail

**Fee Impact ($100K monthly budget):**
- Platform (likely Scale tier): 0.15% = $150
- Keeper: 0.15% = $150
- **Total: $300/month overhead**

**vs. Manual Payments:**
- 20 transactions × $5 labor cost = $100 labor
- Gas fees: Variable
- Error risk: High
- **Tally saves time and reduces errors**

---

## Fee Constants Reference

```rust
// Core protocol fee constants (program/src/constants.rs)

/// Platform base fee for Standard tier (in basis points)
pub const PLATFORM_BASE_FEE_BPS: u16 = 25; // 0.25%

/// Keeper fee for executing scheduled payments (in basis points)
pub const KEEPER_FEE_BPS: u16 = 15; // 0.15%

/// Maximum platform fee across all tiers (in basis points)
pub const MAX_PLATFORM_FEE_BPS: u16 = 50; // 0.5%

/// Minimum platform fee across all tiers (in basis points)
pub const MIN_PLATFORM_FEE_BPS: u16 = 10; // 0.1%

/// Volume threshold for Growth tier (in USDC microlamports)
pub const GROWTH_TIER_THRESHOLD_USDC: u64 = 10_000_000_000; // $10K

/// Volume threshold for Scale tier (in USDC microlamports)
pub const SCALE_TIER_THRESHOLD_USDC: u64 = 100_000_000_000; // $100K

/// Rolling window period for volume calculations (in seconds)
pub const VOLUME_WINDOW_SECONDS: i64 = 2_592_000; // 30 days
```

---

## Extension Development Guidelines

### Setting Extension Fees

**Recommended approach:**

1. **Value-based pricing:** Charge based on features provided, not protocol costs
2. **Competitive analysis:** Compare to similar services in traditional markets
3. **User economics:** Ensure total cost (protocol + extension) remains attractive
4. **Sustainability:** Fee should support extension maintenance and development

### Example Extension Fee Calculation

**Subscription Extension (0.5% fee):**

```
Features provided:
- Free trial management
- Grace period handling
- Plan status management
- Reactivation logic
- Enhanced analytics

Development cost: $50K
Monthly maintenance: $5K
Target monthly revenue: $10K (for sustainability)

Required volume: $10K / 0.5% = $2M monthly
Break-even: ~6 months at $2M/month volume
```

### Extension Fee Best Practices

1. **Keep total under 1%:** Core + extension should stay under 1% for single payments
2. **Document clearly:** Users should understand what they're paying for
3. **No hidden fees:** All fees transparent in extension code
4. **Value alignment:** Higher fees should provide proportionally more value

---

## Frequently Asked Questions

### Q: Why did fees drop from 1-2% to 0.25%?

**A:** The protocol evolved from subscription-specific to universal recurring payments. Lower fees enable hierarchical structures where fees compound across multiple levels. A 3-level hierarchy with 1% fees would total 3% overhead - not viable. At 0.25% per level, total overhead is 0.75% - competitive and sustainable.

### Q: How does Tally sustain itself at 0.25%?

**A:** Volume-based revenue model. $1B monthly volume at 0.25% = $2.5M monthly revenue. The protocol scales through adoption, not high margins.

### Q: Can fees change after deployment?

**A:** No. Core protocol fees are hardcoded constants that cannot change post-deployment. Volume tier thresholds are also immutable. This guarantees predictable economics for all users.

### Q: Do extensions share revenue with the protocol?

**A:** No. Extensions keep 100% of their fees. This maximizes developer incentive to build high-quality extensions and grow the ecosystem.

### Q: What prevents extension developers from charging excessive fees?

**A:** Market competition. Extensions are permissionless - anyone can build competing extensions. Users choose extensions with the best value proposition.

### Q: How do volume tiers upgrade/downgrade?

**A:** Automatically. Volume is tracked on each payment. When 30-day rolling volume crosses a threshold, the tier upgrades immediately. Downgrades occur when volume drops below threshold after 30-day window resets.

### Q: Can I manually select a higher tier for lower fees?

**A:** No. Tiers are determined solely by actual payment volume. This prevents gaming and ensures fairness.

---

## Migration from v1.x.x Fee Structure

### Old Fee Structure (v1.x.x)

| Tier | Platform Fee | Use Case |
|------|--------------|----------|
| Free | 2.0% | Small merchants |
| Pro | 1.5% | Growing businesses |
| Enterprise | 1.0% | Large enterprises |

Plus 0.5% keeper fee = **1.5-2.5% total**

### New Fee Structure (v2.0.0)

| Tier | Platform Fee | Use Case |
|------|--------------|----------|
| Standard | 0.25% | All users under $10K/month |
| Growth | 0.20% | $10K-$100K/month volume |
| Scale | 0.15% | Over $100K/month volume |

Plus 0.15% keeper fee = **0.40-0.65% total**

### Impact

- **Existing merchants:** 60-84% fee reduction
- **Hierarchical structures:** Now economically viable
- **Automatic:** No action required for tier upgrades

---

## Summary

Tally's volume-based fee structure enables economically viable hierarchical payment architectures while maintaining protocol sustainability through volume-based revenue:

- **Core Protocol:** 0.40% total (0.25% platform + 0.15% keeper)
- **With Extensions:** 0.70-1.00% total (add 0.3-0.6% extension fee)
- **Multi-level:** Fees compound but remain competitive
- **Volume Discounts:** Automatic tier upgrades reward growth
- **Developer-Friendly:** Extensions keep 100% of their fees

This structure supports diverse use cases from simple subscriptions to complex hierarchical payment networks, all on a single unified protocol.

---

**For more information:**
- Technical specification: `.claude/RECURRING_PAYMENTS_ARCHITECTURE.md`
- Implementation details: `program/src/constants.rs`
- Volume tier logic: `program/src/state.rs`
