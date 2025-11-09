# v1.0 Production Release Guide (Recurring Payments Architecture)

**Date:** 2025-11-08
**Branch:** refactor/recurring-payments-core
**Version:** 1.0.0 (First Production Release)

---

## Overview

Version 1.0.0 is the **first production release** of the Tally recurring payments protocol. This represents a fundamental architectural shift from the development/pre-release **subscription-specific** model to **universal recurring payments**. This enables the protocol to support diverse use cases beyond subscriptions: payroll, investments, grants, and hierarchical payment structures.

**Note:** If you were using pre-release/development versions (0.x.x), this guide documents all changes from the subscription-based architecture to the production recurring payments architecture.

### Why This Architecture?

The pre-release architecture was tightly coupled to subscription terminology and features (trials, grace periods, plan names). This limited applicability to non-subscription use cases and created unnecessary complexity. The production 1.0 architecture:

- ✅ Adopts universal domain language (Payee, PaymentTerms, PaymentAgreement)
- ✅ Removes subscription-specific features from core (trials, grace periods)
- ✅ Reduces account sizes (Plan: 129→80 bytes)
- ✅ Enables hierarchical payment structures
- ✅ Volume-based fee tiers (0.15-0.25% vs 1-2%)

---

## Breaking Changes Summary

### Type Renames

| Pre-release (Subscription) | v1.0 Production (Recurring Payment) |
|---------------------|--------------------------|
| `Merchant` | `Payee` |
| `Plan` | `PaymentTerms` |
| `Subscription` | `PaymentAgreement` |
| `MerchantTier` | `VolumeTier` |
| `SubscriptionError` | `RecurringPaymentError` |

### Instruction Renames

| Pre-release | v1.0 |
|------|------|
| `init_merchant` | `init_payee` |
| `create_plan` | `create_payment_terms` |
| `start_subscription` | `start_agreement` |
| `renew_subscription` | `execute_payment` |
| `cancel_subscription` | `pause_agreement` |
| `close_subscription` | `close_agreement` |
| `update_plan_terms` | ❌ **REMOVED** (subscription-specific) |

### Event Renames

| Pre-release | v1.0 |
|------|------|
| `Subscribed` | `PaymentAgreementStarted` |
| `Renewed` | `PaymentExecuted` |
| `Canceled` | `PaymentAgreementPaused` |
| `SubscriptionClosed` | `PaymentAgreementClosed` |
| `PlanCreated` | `PaymentTermsCreated` |
| `MerchantInitialized` | `PayeeInitialized` |
| `TrialStarted` | ❌ **REMOVED** |
| `TrialConverted` | ❌ **REMOVED** |

### Error Variant Renames

| Pre-release | v1.0 |
|------|------|
| `MerchantNotFound` | `PayeeNotFound` |
| `PlanNotFound` | `PaymentTermsNotFound` |
| `SubscriptionNotFound` | `PaymentAgreementNotFound` |
| `PlanAlreadyExists` | `PaymentTermsAlreadyExist` |
| `AlreadyCanceled` | `AlreadyPaused` |
| `InvalidPlan` | `InvalidPaymentTerms` |
| `InvalidMerchantTreasuryAccount` | `InvalidPayeeTreasuryAccount` |
| `InvalidSubscriberTokenAccount` | `InvalidPayerTokenAccount` |
| `PastGrace` | ❌ **REMOVED** |
| `InvalidTrialDuration` | ❌ **REMOVED** |
| `TrialAlreadyUsed` | ❌ **REMOVED** |

---

## Field-Level Changes

### PaymentTerms (formerly Plan)

| Pre-release Field | v1.0 Field | Notes |
|------------|------------|-------|
| `merchant` | `payee` | Reference to payee PDA |
| `plan_id` | `terms_id` | Payment terms identifier |
| `price_usdc` | `amount_usdc` | Payment amount |
| `period_secs` | `period_secs` | ✅ Unchanged |
| `grace_secs` | ❌ **REMOVED** | Moved to subscription extension |
| `name` | ❌ **REMOVED** | Moved to off-chain indexer |
| `active` | ❌ **REMOVED** | Moved to subscription extension |

### PaymentAgreement (formerly Subscription)

| Pre-release Field | v1.0 Field | Notes |
|------------|------------|-------|
| `plan` | `payment_terms` | Reference to payment terms |
| `subscriber` | `payer` | Who makes payments |
| `next_renewal_ts` | `next_payment_ts` | Next payment timestamp |
| `renewals` | `payment_count` | Total payments executed |
| `last_renewed_ts` | `last_payment_ts` | Last payment timestamp |
| `active` | `active` | ✅ Unchanged |
| `created_ts` | `created_ts` | ✅ Unchanged |
| `last_amount` | `last_amount` | ✅ Unchanged |
| `bump` | `bump` | ✅ Unchanged |

### Payee (formerly Merchant)

| Pre-release Field | v1.0 Field | Notes |
|------------|------------|-------|
| `authority` | `authority` | ✅ Unchanged |
| `usdc_mint` | `usdc_mint` | ✅ Unchanged |
| `treasury_ata` | `treasury_ata` | ✅ Unchanged |
| `tier` | `volume_tier` | Now auto-calculated from volume |
| `platform_fee_bps` | ❌ **REMOVED** | Fee derived from volume_tier |
| ❌ (new) | `monthly_volume_usdc` | Rolling 30-day volume |
| ❌ (new) | `last_volume_update_ts` | Volume tracking timestamp |
| `bump` | `bump` | ✅ Unchanged |

---

## Migration Steps

### Step 1: Update Dependencies

```toml
# Cargo.toml
[dependencies]
tally-protocol = { git = "https://github.com/your-org/tally-protocol", branch = "main" }
# or specific version
tally-protocol = "1.0.0"
```

### Step 2: Update Type Imports

**Before (pre-release 0.x):**
```rust
use tally_protocol::{
    Merchant, Plan, Subscription,
    SubscriptionError, MerchantTier,
    instructions::InitMerchantArgs,
};
```

**After (v1.0):**
```rust
use tally_protocol::{
    Payee, PaymentTerms, PaymentAgreement,
    RecurringPaymentError, VolumeTier,
    instructions::InitPayeeArgs,
};
```

### Step 3: Update Instruction Calls

#### Initialize Payee (formerly Merchant)

**Before:**
```rust
let ix = tally_protocol::instruction::init_merchant(
    InitMerchantArgs {
        treasury_ata: merchant_usdc_ata,
        tier: MerchantTier::Pro,
    }
)?;
```

**After:**
```rust
let ix = tally_protocol::instruction::init_payee(
    InitPayeeArgs {
        treasury_ata: payee_usdc_ata,
        // tier removed - automatically Standard, upgrades based on volume
    }
)?;
```

#### Create Payment Terms (formerly Plan)

**Before:**
```rust
let ix = tally_protocol::instruction::create_plan(
    CreatePlanArgs {
        plan_id: "premium".to_string(),
        name: "Premium Plan".to_string(), // REMOVED in v2.0
        price_usdc: 10_000_000,
        period_secs: 2_592_000,
        grace_secs: 432_000, // REMOVED in v2.0
        active: true, // REMOVED in v2.0
    }
)?;
```

**After:**
```rust
let ix = tally_protocol::instruction::create_payment_terms(
    CreatePaymentTermsArgs {
        terms_id: "premium".to_string(),
        amount_usdc: 10_000_000,
        period_secs: 2_592_000,
        // name, grace_secs, active removed
    }
)?;
```

#### Start Agreement (formerly Subscription)

**Before:**
```rust
let ix = tally_protocol::instruction::start_subscription(
    StartSubscriptionArgs {
        allowance_periods: 3,
        trial_duration_secs: Some(604_800), // REMOVED in v2.0
    }
)?;
```

**After:**
```rust
let ix = tally_protocol::instruction::start_agreement(
    StartAgreementArgs {
        allowance_periods: 3,
        // trial_duration_secs removed
    }
)?;
```

#### Execute Payment (formerly Renew Subscription)

**Before:**
```rust
let ix = tally_protocol::instruction::renew_subscription(
    RenewSubscriptionArgs {
        keeper: keeper_pubkey,
    }
)?;
```

**After:**
```rust
let ix = tally_protocol::instruction::execute_payment(
    ExecutePaymentArgs {
        keeper: keeper_pubkey,
    }
)?;
```

#### Pause Agreement (formerly Cancel Subscription)

**Before:**
```rust
let ix = tally_protocol::instruction::cancel_subscription(
    CancelSubscriptionArgs {}
)?;
```

**After:**
```rust
let ix = tally_protocol::instruction::pause_agreement(
    PauseAgreementArgs {}
)?;
```

### Step 4: Update PDA Derivations

**Before:**
```rust
let (merchant_pda, _) = Pubkey::find_program_address(
    &[b"merchant", authority.as_ref()],
    &program_id,
);

let (plan_pda, _) = Pubkey::find_program_address(
    &[b"plan", merchant_pda.as_ref(), plan_id.as_bytes()],
    &program_id,
);

let (subscription_pda, _) = Pubkey::find_program_address(
    &[b"subscription", plan_pda.as_ref(), subscriber.as_ref()],
    &program_id,
);
```

**After:**
```rust
let (payee_pda, _) = Pubkey::find_program_address(
    &[b"payee", authority.as_ref()],
    &program_id,
);

let (payment_terms_pda, _) = Pubkey::find_program_address(
    &[b"payment_terms", payee_pda.as_ref(), terms_id.as_bytes()],
    &program_id,
);

let (payment_agreement_pda, _) = Pubkey::find_program_address(
    &[b"payment_agreement", payment_terms_pda.as_ref(), payer.as_ref()],
    &program_id,
);
```

### Step 5: Update Account Access

**Before:**
```rust
let merchant_account = Merchant::try_from(&merchant_account_info)?;
println!("Merchant tier: {:?}", merchant_account.tier);
println!("Platform fee: {}bps", merchant_account.platform_fee_bps);

let plan_account = Plan::try_from(&plan_account_info)?;
println!("Plan name: {}", String::from_utf8_lossy(&plan_account.name));
println!("Price: {}", plan_account.price_usdc);
println!("Grace period: {}s", plan_account.grace_secs);

let subscription_account = Subscription::try_from(&subscription_account_info)?;
println!("Subscriber: {}", subscription_account.subscriber);
println!("Next renewal: {}", subscription_account.next_renewal_ts);
println!("Total renewals: {}", subscription_account.renewals);
```

**After:**
```rust
let payee_account = Payee::try_from(&payee_account_info)?;
println!("Volume tier: {:?}", payee_account.volume_tier);
println!("Monthly volume: {}", payee_account.monthly_volume_usdc);
// platform_fee_bps is now derived: payee_account.volume_tier.platform_fee_bps()

let payment_terms_account = PaymentTerms::try_from(&payment_terms_account_info)?;
println!("Payment amount: {}", payment_terms_account.amount_usdc);
println!("Period: {}s", payment_terms_account.period_secs);
// name, grace_secs, active fields removed

let payment_agreement_account = PaymentAgreement::try_from(&payment_agreement_account_info)?;
println!("Payer: {}", payment_agreement_account.payer);
println!("Next payment: {}", payment_agreement_account.next_payment_ts);
println!("Total payments: {}", payment_agreement_account.payment_count);
```

### Step 6: Update Error Handling

**Before:**
```rust
match err {
    SubscriptionError::MerchantNotFound => {
        println!("Merchant not initialized");
    }
    SubscriptionError::PlanNotFound => {
        println!("Plan doesn't exist");
    }
    SubscriptionError::SubscriptionNotFound => {
        println!("No active subscription");
    }
    SubscriptionError::AlreadyCanceled => {
        println!("Subscription already canceled");
    }
    SubscriptionError::PastGrace => {
        println!("Grace period expired");
    }
    _ => {}
}
```

**After:**
```rust
match err {
    RecurringPaymentError::PayeeNotFound => {
        println!("Payee not initialized");
    }
    RecurringPaymentError::PaymentTermsNotFound => {
        println!("Payment terms don't exist");
    }
    RecurringPaymentError::PaymentAgreementNotFound => {
        println!("No active payment agreement");
    }
    RecurringPaymentError::AlreadyPaused => {
        println!("Agreement already paused");
    }
    // PastGrace removed - grace periods moved to extension layer
    _ => {}
}
```

### Step 7: Update Event Listeners

**Before:**
```rust
program
    .listen()
    .on("Subscribed", |event: Subscribed| {
        println!("New subscription: {}", event.subscriber);
        println!("Plan: {}", event.plan);
        println!("Next renewal: {}", event.next_renewal_ts);
    })
    .on("Renewed", |event: Renewed| {
        println!("Renewal #{}", event.total_renewals);
        println!("Amount: {}", event.amount);
    })
    .on("Canceled", |event: Canceled| {
        println!("Subscription canceled: {}", event.subscription);
    })
    .on("TrialStarted", |event: TrialStarted| {
        println!("Trial started, ends: {}", event.trial_ends_at);
    });
```

**After:**
```rust
program
    .listen()
    .on("PaymentAgreementStarted", |event: PaymentAgreementStarted| {
        println!("New agreement: {}", event.payer);
        println!("Terms: {}", event.payment_terms);
        println!("Next payment: {}", event.next_payment_ts);
    })
    .on("PaymentExecuted", |event: PaymentExecuted| {
        println!("Payment #{}", event.total_payments);
        println!("Amount: {}", event.amount);
    })
    .on("PaymentAgreementPaused", |event: PaymentAgreementPaused| {
        println!("Agreement paused: {}", event.payment_agreement);
    });
    // TrialStarted removed - trials moved to extension layer
```

---

## Common Migration Patterns

### Pattern 1: Fetching All Payment Terms for a Payee

**Before:**
```rust
let plans = program
    .accounts::<Plan>()
    .filter(|p| p.merchant == merchant_pda)
    .fetch_all()
    .await?;

for plan in plans {
    println!("Plan: {}", String::from_utf8_lossy(&plan.name));
    println!("  Price: {} USDC", plan.price_usdc / 1_000_000);
    println!("  Period: {}s", plan.period_secs);
}
```

**After:**
```rust
let payment_terms = program
    .accounts::<PaymentTerms>()
    .filter(|pt| pt.payee == payee_pda)
    .fetch_all()
    .await?;

for terms in payment_terms {
    println!("Terms ID: {}", String::from_utf8_lossy(&terms.terms_id));
    println!("  Amount: {} USDC", terms.amount_usdc / 1_000_000);
    println!("  Period: {}s", terms.period_secs);
}
```

### Pattern 2: Checking Agreement Status

**Before:**
```rust
let subscription = program.account::<Subscription>(subscription_pda).await?;

if !subscription.active {
    println!("Subscription is canceled");
    return;
}

let time_until_renewal = subscription.next_renewal_ts - Clock::get()?.unix_timestamp;
if time_until_renewal < 0 {
    println!("Renewal overdue by {}s", time_until_renewal.abs());
} else {
    println!("Next renewal in {}s", time_until_renewal);
}
```

**After:**
```rust
let agreement = program.account::<PaymentAgreement>(payment_agreement_pda).await?;

if !agreement.active {
    println!("Agreement is paused");
    return;
}

let time_until_payment = agreement.next_payment_ts - Clock::get()?.unix_timestamp;
if time_until_payment < 0 {
    println!("Payment overdue by {}s", time_until_payment.abs());
} else {
    println!("Next payment in {}s", time_until_payment);
}
```

### Pattern 3: Volume Tier Logic

**Before:**
```rust
let merchant = program.account::<Merchant>(merchant_pda).await?;
let platform_fee = merchant.platform_fee_bps;

println!("Merchant tier: {:?}", merchant.tier);
println!("Platform fee: {}bps", platform_fee);
```

**After:**
```rust
let payee = program.account::<Payee>(payee_pda).await?;
let platform_fee = payee.volume_tier.platform_fee_bps();

println!("Volume tier: {:?}", payee.volume_tier);
println!("Platform fee: {}bps ({})",
    platform_fee,
    match payee.volume_tier {
        VolumeTier::Standard => "0.25%",
        VolumeTier::Growth => "0.20%",
        VolumeTier::Scale => "0.15%",
    }
);
println!("Monthly volume: ${}", payee.monthly_volume_usdc / 1_000_000);
```

---

## Removed Features (Moved to Extensions)

### Free Trials

**v1.x:**
```rust
// Create plan with trial
let ix = create_plan(CreatePlanArgs {
    // ... other fields
    trial_duration_secs: Some(604_800), // 7 days
})?;

// Start subscription with trial
let ix = start_subscription(StartSubscriptionArgs {
    trial_duration_secs: Some(604_800),
})?;
```

**v2.0:**
Trials are **removed from core protocol**. To implement trials:
1. Use the **subscription extension layer** (separate program)
2. Or implement at application level:
   - Store trial state off-chain
   - Start agreement with `amount_usdc = 0` for trial period
   - Update to paid terms after trial converts

### Grace Periods

**v1.x:**
```rust
// Create plan with grace period
let ix = create_plan(CreatePlanArgs {
    // ... other fields
    grace_secs: 432_000, // 5 days
})?;
```

**v2.0:**
Grace periods are **removed from core protocol**. To implement grace periods:
1. Use the **subscription extension layer**
2. Or implement at keeper level:
   - Allow renewals within grace window
   - Mark as "at risk" during grace period
   - Emit custom events for grace period status

### Plan Active/Inactive Status

**v1.x:**
```rust
let plan = program.account::<Plan>(plan_pda).await?;
if !plan.active {
    return Err("Plan is inactive");
}
```

**v2.0:**
Active status **removed from PaymentTerms**. To implement:
1. Use **subscription extension layer**
2. Or track off-chain:
   - Store plan status in database
   - Check status before creating agreements
   - Prevent new agreements for inactive plans at application layer

---

## Fee Structure Changes

### Pre-release Fee Structure (Subscription-Specific)

```
MerchantTier::Free       → 2.0% platform fee
MerchantTier::Pro        → 1.5% platform fee
MerchantTier::Enterprise → 1.0% platform fee
Keeper fee: 0.5%
```

### v1.0 Production Fee Structure (Volume-Based)

```
VolumeTier::Standard  → 0.25% platform fee (up to $10K monthly volume)
VolumeTier::Growth    → 0.20% platform fee ($10K-$100K monthly volume)
VolumeTier::Scale     → 0.15% platform fee (>$100K monthly volume)
Keeper fee: 0.15%
```

**Migration Impact:**
- ✅ **Significantly lower fees** (0.40% total vs 2.5% total)
- ✅ **Automatic tier upgrades** based on volume
- ✅ **No tier selection** - all payees start at Standard
- ✅ **Hierarchical-friendly** - low enough for multi-level payments

---

## Testing Your Migration

### Unit Tests

Update your test helpers:

```rust
// Before
fn create_test_merchant() -> Merchant {
    Merchant {
        authority: test_authority(),
        usdc_mint: USDC_MINT,
        treasury_ata: test_ata(),
        tier: MerchantTier::Pro,
        platform_fee_bps: 150,
        bump: 255,
    }
}

// After
fn create_test_payee() -> Payee {
    Payee {
        authority: test_authority(),
        usdc_mint: USDC_MINT,
        treasury_ata: test_ata(),
        volume_tier: VolumeTier::Standard,
        monthly_volume_usdc: 0,
        last_volume_update_ts: 0,
        bump: 255,
    }
}
```

### Integration Tests

Update account fetching:

```rust
// Before
let merchant = client
    .get_account_with_commitment(&merchant_pda, CommitmentConfig::confirmed())
    .await?
    .value
    .ok_or("Merchant not found")?;
let merchant: Merchant = Account::unpack(&merchant.data)?;

// After
let payee = client
    .get_account_with_commitment(&payee_pda, CommitmentConfig::confirmed())
    .await?
    .value
    .ok_or("Payee not found")?;
let payee: Payee = Account::unpack(&payee.data)?;
```

---

## Rollback Strategy

If you encounter issues and need to rollback to pre-release versions temporarily:

```toml
# Cargo.toml - Pin to pre-release
[dependencies]
tally-protocol = { git = "https://github.com/your-org/tally-protocol", tag = "v0.2.1" }
```

**Note:** v1.0 uses **different PDAs** (payee vs merchant seeds), so pre-release (0.x) and production (1.0) accounts are **not compatible**. You cannot mix versions.

---

## Production Release Checklist

Use this checklist to ensure complete migration to v1.0:

- [ ] Updated `Cargo.toml` to v1.0 dependency
- [ ] Replaced all `Merchant` → `Payee` type references
- [ ] Replaced all `Plan` → `PaymentTerms` type references
- [ ] Replaced all `Subscription` → `PaymentAgreement` type references
- [ ] Updated `SubscriptionError` → `RecurringPaymentError`
- [ ] Updated all instruction calls (`init_merchant` → `init_payee`, etc.)
- [ ] Updated all PDA derivations (seeds changed)
- [ ] Updated all field accesses (`price_usdc` → `amount_usdc`, etc.)
- [ ] Removed references to deleted fields (`grace_secs`, `name`, `active`)
- [ ] Updated event listeners (event names changed)
- [ ] Updated error handling (error variant names changed)
- [ ] Removed trial logic (if applicable)
- [ ] Removed grace period logic (if applicable)
- [ ] Updated all tests
- [ ] Updated all documentation
- [ ] Tested locally against v2.0 program

---

## Support & Resources

- **Architecture Docs:** `.claude/RECURRING_PAYMENTS_ARCHITECTURE.md`
- **Fee Structure Guide:** See fee refactor commits (1b92f72, etc.)
- **Example CLI:** `tally-cli` repository (updated for v2.0)
- **Test Suite:** `program/tests/*.rs` (455 passing tests)
- **SDK Tests:** `sdk/tests/*.rs` (105 passing tests)

---

## Questions?

For migration assistance:
1. Review the test suite in `program/tests/` for usage examples
2. Check `tally-cli` source code for SDK integration patterns
3. Review commit history for detailed change rationale

**This is the first production release (v1.0.0).** If you were using pre-release versions (0.x), take time to update and test thoroughly before deploying.
