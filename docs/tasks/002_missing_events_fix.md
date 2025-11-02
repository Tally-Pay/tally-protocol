# Task 002: Add Missing Event Types to SDK

**Status**: Investigation Complete - Ready for Implementation
**Priority**: High
**Estimated Effort**: 3-4 hours

## Overview

The SDK currently parses only 4 out of 20 event types emitted by the tally-protocol program. This investigation identifies all missing events, their structures, and provides a complete implementation plan.

## Current State Analysis

### Currently Implemented Events (4/20)

The SDK (`/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`) currently supports:

1. **Subscribed** - Lines 15-24
2. **Renewed** - Lines 30-39 (INCOMPLETE - missing keeper and keeper_fee fields)
3. **Canceled** - Lines 45-52
4. **PaymentFailed** - Lines 58-67

### Missing Events from Program (16/20)

Based on `/home/rodzilla/projects/tally/tally-protocol/program/src/events.rs`:

1. **SubscriptionReactivated** (lines 18-31) - NEW event not in SDK
2. **SubscriptionClosed** (lines 63-68)
3. **PlanStatusChanged** (lines 85-94)
4. **ConfigInitialized** (lines 98-117)
5. **MerchantInitialized** (lines 121-134)
6. **PlanCreated** (lines 138-155)
7. **ProgramPaused** (lines 159-164)
8. **ProgramUnpaused** (lines 168-173)
9. **LowAllowanceWarning** (lines 191-204)
10. **FeesWithdrawn** (lines 215-224)
11. **DelegateMismatchWarning** (lines 254-265)
12. **ConfigUpdated** (lines 272-285)
13. **MerchantTierChanged** (lines 297-306)
14. **PlanTermsUpdated** (lines 318-337)
15. **TrialStarted** (lines 351-360)
16. **TrialConverted** (lines 374-381)

## Critical Issue: Renewed Event Missing Fields

The SDK's `Renewed` event is **INCOMPLETE**. It's missing two critical fields:

**Program Definition** (program/src/events.rs:34-48):
```rust
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub keeper: Pubkey,        // MISSING IN SDK
    pub keeper_fee: u64,       // MISSING IN SDK
}
```

**SDK Definition** (sdk/src/events.rs:30-39):
```rust
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    // MISSING: keeper and keeper_fee
}
```

**Impact**: This will cause deserialization failures when parsing Renewed events from transaction logs.

## Complete Event Structure Definitions

### 1. SubscriptionReactivated (NEW)

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct SubscriptionReactivated {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub total_renewals: u32,
    pub original_created_ts: i64,
}
```

**Description**: Emitted when a previously canceled subscription is reactivated.

### 2. SubscriptionClosed

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct SubscriptionClosed {
    pub plan: Pubkey,
    pub subscriber: Pubkey,
}
```

**Description**: Emitted when a subscription account is closed and rent is reclaimed.

### 3. PlanStatusChanged

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PlanStatusChanged {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub active: bool,
    pub changed_by: String,
}
```

**Description**: Emitted when a plan's active status is changed.

### 4. ConfigInitialized

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ConfigInitialized {
    pub platform_authority: Pubkey,
    pub max_platform_fee_bps: u16,
    pub min_platform_fee_bps: u16,
    pub min_period_seconds: u64,
    pub default_allowance_periods: u8,
    pub allowed_mint: Pubkey,
    pub max_withdrawal_amount: u64,
    pub max_grace_period_seconds: u64,
    pub timestamp: i64,
}
```

**Description**: Emitted when global program configuration is initialized.

### 5. MerchantInitialized

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct MerchantInitialized {
    pub merchant: Pubkey,
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub treasury_ata: Pubkey,
    pub platform_fee_bps: u16,
    pub timestamp: i64,
}
```

**Description**: Emitted when a merchant account is initialized.

### 6. PlanCreated

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PlanCreated {
    pub plan: Pubkey,
    pub merchant: Pubkey,
    pub plan_id: String,
    pub price_usdc: u64,
    pub period_secs: u64,
    pub grace_secs: u64,
    pub name: String,
    pub timestamp: i64,
}
```

**Description**: Emitted when a subscription plan is created.

### 7. ProgramPaused

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ProgramPaused {
    pub authority: Pubkey,
    pub timestamp: i64,
}
```

**Description**: Emitted when the program is paused.

### 8. ProgramUnpaused

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ProgramUnpaused {
    pub authority: Pubkey,
    pub timestamp: i64,
}
```

**Description**: Emitted when the program is unpaused.

### 9. LowAllowanceWarning

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct LowAllowanceWarning {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub current_allowance: u64,
    pub recommended_allowance: u64,
    pub plan_price: u64,
}
```

**Description**: Emitted when a subscription renewal succeeds but remaining allowance is low.

### 10. FeesWithdrawn

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct FeesWithdrawn {
    pub platform_authority: Pubkey,
    pub destination: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}
```

**Description**: Emitted when platform fees are withdrawn.

### 11. DelegateMismatchWarning

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct DelegateMismatchWarning {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub expected_delegate: Pubkey,
    pub actual_delegate: Option<Pubkey>,
}
```

**Description**: Emitted when a delegate mismatch is detected during subscription renewal.

### 12. ConfigUpdated

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct ConfigUpdated {
    pub keeper_fee_bps: u16,
    pub max_withdrawal_amount: u64,
    pub max_grace_period_seconds: u64,
    pub min_platform_fee_bps: u16,
    pub max_platform_fee_bps: u16,
    pub updated_by: Pubkey,
}
```

**Description**: Emitted when global configuration is updated.

### 13. MerchantTierChanged

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct MerchantTierChanged {
    pub merchant: Pubkey,
    pub old_tier: MerchantTier,
    pub new_tier: MerchantTier,
    pub new_fee_bps: u16,
}
```

**Description**: Emitted when a merchant's tier is changed.

**Dependencies**: Requires `MerchantTier` enum to be defined in SDK:

```rust
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub enum MerchantTier {
    Free,
    Pro,
    Enterprise,
}
```

### 14. PlanTermsUpdated

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct PlanTermsUpdated {
    pub plan: Pubkey,
    pub merchant: Pubkey,
    pub old_price: Option<u64>,
    pub new_price: Option<u64>,
    pub old_period: Option<u64>,
    pub new_period: Option<u64>,
    pub old_grace: Option<u64>,
    pub new_grace: Option<u64>,
    pub updated_by: Pubkey,
}
```

**Description**: Emitted when a plan's pricing or terms are updated.

### 15. TrialStarted

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct TrialStarted {
    pub subscription: Pubkey,
    pub subscriber: Pubkey,
    pub plan: Pubkey,
    pub trial_ends_at: i64,
}
```

**Description**: Emitted when a subscription starts with a free trial period.

### 16. TrialConverted

```rust
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub struct TrialConverted {
    pub subscription: Pubkey,
    pub subscriber: Pubkey,
    pub plan: Pubkey,
}
```

**Description**: Emitted when a trial subscription converts to paid.

## Event Discriminator Computation

All event discriminators are computed using the same formula:

```rust
fn compute_event_discriminator(event_name: &str) -> [u8; 8] {
    use anchor_lang::solana_program::hash;
    let preimage = format!("event:{event_name}");
    let hash_result = hash::hash(preimage.as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash_result.to_bytes()[..8]);
    discriminator
}
```

Event names match struct names exactly:
- `"SubscriptionReactivated"`
- `"SubscriptionClosed"`
- `"PlanStatusChanged"`
- `"ConfigInitialized"`
- `"MerchantInitialized"`
- `"PlanCreated"`
- `"ProgramPaused"`
- `"ProgramUnpaused"`
- `"LowAllowanceWarning"`
- `"FeesWithdrawn"`
- `"DelegateMismatchWarning"`
- `"ConfigUpdated"`
- `"MerchantTierChanged"`
- `"PlanTermsUpdated"`
- `"TrialStarted"`
- `"TrialConverted"`

## Complete TallyEvent Enum

The updated enum should include all 20 event variants:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TallyEvent {
    // Subscription lifecycle events (4 existing + 2 new)
    Subscribed(Subscribed),
    SubscriptionReactivated(SubscriptionReactivated),
    Renewed(Renewed),
    Canceled(Canceled),
    SubscriptionClosed(SubscriptionClosed),
    PaymentFailed(PaymentFailed),

    // Trial events (2 new)
    TrialStarted(TrialStarted),
    TrialConverted(TrialConverted),

    // Plan events (2 new)
    PlanCreated(PlanCreated),
    PlanStatusChanged(PlanStatusChanged),
    PlanTermsUpdated(PlanTermsUpdated),

    // Merchant events (2 new)
    MerchantInitialized(MerchantInitialized),
    MerchantTierChanged(MerchantTierChanged),

    // Config events (2 new)
    ConfigInitialized(ConfigInitialized),
    ConfigUpdated(ConfigUpdated),

    // Admin events (3 new)
    FeesWithdrawn(FeesWithdrawn),
    ProgramPaused(ProgramPaused),
    ProgramUnpaused(ProgramUnpaused),

    // Warning events (2 new)
    LowAllowanceWarning(LowAllowanceWarning),
    DelegateMismatchWarning(DelegateMismatchWarning),
}
```

## Implementation Plan

### Phase 1: Fix Existing Renewed Event (CRITICAL)

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`

1. Update `Renewed` struct definition (lines 30-39):
   ```rust
   #[derive(
       Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
   )]
   pub struct Renewed {
       pub merchant: Pubkey,
       pub plan: Pubkey,
       pub subscriber: Pubkey,
       pub amount: u64,
       pub keeper: Pubkey,      // ADD THIS
       pub keeper_fee: u64,     // ADD THIS
   }
   ```

2. Update `TallyEvent::Renewed` match arms in helper methods to handle new fields

3. Update `ParsedEventWithContext::to_streamable()` to include keeper info in metadata

### Phase 2: Add MerchantTier Enum (DEPENDENCY)

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`

Add at the top of the file (after imports):

```rust
/// Merchant tier determines platform fee rate
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize,
)]
pub enum MerchantTier {
    /// Free tier: 2.0% platform fee (200 basis points)
    Free,
    /// Pro tier: 1.5% platform fee (150 basis points)
    Pro,
    /// Enterprise tier: 1.0% platform fee (100 basis points)
    Enterprise,
}
```

### Phase 3: Add All Missing Event Structs

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`

Add all 16 missing event struct definitions (see complete definitions above).

Recommended organization:
1. Subscription lifecycle events (SubscriptionReactivated, SubscriptionClosed)
2. Trial events (TrialStarted, TrialConverted)
3. Plan events (PlanCreated, PlanStatusChanged, PlanTermsUpdated)
4. Merchant events (MerchantInitialized, MerchantTierChanged)
5. Config events (ConfigInitialized, ConfigUpdated)
6. Admin events (FeesWithdrawn, ProgramPaused, ProgramUnpaused)
7. Warning events (LowAllowanceWarning, DelegateMismatchWarning)

### Phase 4: Update TallyEvent Enum

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs` (lines 69-80)

Replace the current 4-variant enum with the complete 20-variant enum.

### Phase 5: Update get_event_discriminators()

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs` (lines 334-344)

Add discriminators for all 16 new events:

```rust
fn get_event_discriminators() -> HashMap<[u8; 8], &'static str> {
    let mut discriminators = HashMap::new();

    // Subscription lifecycle events
    discriminators.insert(compute_event_discriminator("Subscribed"), "Subscribed");
    discriminators.insert(compute_event_discriminator("SubscriptionReactivated"), "SubscriptionReactivated");
    discriminators.insert(compute_event_discriminator("Renewed"), "Renewed");
    discriminators.insert(compute_event_discriminator("Canceled"), "Canceled");
    discriminators.insert(compute_event_discriminator("SubscriptionClosed"), "SubscriptionClosed");
    discriminators.insert(compute_event_discriminator("PaymentFailed"), "PaymentFailed");

    // Trial events
    discriminators.insert(compute_event_discriminator("TrialStarted"), "TrialStarted");
    discriminators.insert(compute_event_discriminator("TrialConverted"), "TrialConverted");

    // Plan events
    discriminators.insert(compute_event_discriminator("PlanCreated"), "PlanCreated");
    discriminators.insert(compute_event_discriminator("PlanStatusChanged"), "PlanStatusChanged");
    discriminators.insert(compute_event_discriminator("PlanTermsUpdated"), "PlanTermsUpdated");

    // Merchant events
    discriminators.insert(compute_event_discriminator("MerchantInitialized"), "MerchantInitialized");
    discriminators.insert(compute_event_discriminator("MerchantTierChanged"), "MerchantTierChanged");

    // Config events
    discriminators.insert(compute_event_discriminator("ConfigInitialized"), "ConfigInitialized");
    discriminators.insert(compute_event_discriminator("ConfigUpdated"), "ConfigUpdated");

    // Admin events
    discriminators.insert(compute_event_discriminator("FeesWithdrawn"), "FeesWithdrawn");
    discriminators.insert(compute_event_discriminator("ProgramPaused"), "ProgramPaused");
    discriminators.insert(compute_event_discriminator("ProgramUnpaused"), "ProgramUnpaused");

    // Warning events
    discriminators.insert(compute_event_discriminator("LowAllowanceWarning"), "LowAllowanceWarning");
    discriminators.insert(compute_event_discriminator("DelegateMismatchWarning"), "DelegateMismatchWarning");

    discriminators
}
```

### Phase 6: Update parse_single_event()

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs` (lines 458-486)

Add match arms for all 16 new event types:

```rust
match *event_type {
    "Subscribed" => {
        let event = Subscribed::try_from_slice(event_data).map_err(|e| {
            TallyError::ParseError(format!("Failed to deserialize Subscribed event: {e}"))
        })?;
        Ok(TallyEvent::Subscribed(event))
    }
    "SubscriptionReactivated" => {
        let event = SubscriptionReactivated::try_from_slice(event_data).map_err(|e| {
            TallyError::ParseError(format!("Failed to deserialize SubscriptionReactivated event: {e}"))
        })?;
        Ok(TallyEvent::SubscriptionReactivated(event))
    }
    "Renewed" => {
        let event = Renewed::try_from_slice(event_data).map_err(|e| {
            TallyError::ParseError(format!("Failed to deserialize Renewed event: {e}"))
        })?;
        Ok(TallyEvent::Renewed(event))
    }
    // ... continue for all 20 events
}
```

### Phase 7: Update ParsedEventWithContext Helper Methods

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`

Update the following methods to handle all new event types:

1. **to_streamable()** (lines 143-213) - Add match arms for all new events
2. **get_merchant()** (lines 223-230) - Add events with merchant field
3. **get_plan()** (lines 234-241) - Add events with plan field
4. **get_subscriber()** (lines 245-252) - Add events with subscriber field
5. **get_amount()** (lines 256-262) - Add events with amount field
6. **get_event_type_string()** (lines 266-273) - Add string names for all events
7. **affects_revenue()** (lines 296-301) - Add SubscriptionReactivated, TrialConverted
8. **affects_subscription_count()** (lines 305-310) - Add SubscriptionReactivated, SubscriptionClosed

Add new helper methods:
```rust
/// Get the keeper pubkey from renewal events
pub const fn get_keeper(&self) -> Option<Pubkey> {
    match &self.event {
        TallyEvent::Renewed(e) => Some(e.keeper),
        _ => None,
    }
}

/// Get the keeper fee from renewal events
pub const fn get_keeper_fee(&self) -> Option<u64> {
    match &self.event {
        TallyEvent::Renewed(e) => Some(e.keeper_fee),
        _ => None,
    }
}

/// Get timestamp from events that include it
pub const fn get_event_timestamp(&self) -> Option<i64> {
    match &self.event {
        TallyEvent::ConfigInitialized(e) => Some(e.timestamp),
        TallyEvent::MerchantInitialized(e) => Some(e.timestamp),
        TallyEvent::PlanCreated(e) => Some(e.timestamp),
        TallyEvent::FeesWithdrawn(e) => Some(e.timestamp),
        TallyEvent::ProgramPaused(e) => Some(e.timestamp),
        TallyEvent::ProgramUnpaused(e) => Some(e.timestamp),
        _ => None,
    }
}

/// Check if this is a warning event
pub const fn is_warning(&self) -> bool {
    matches!(
        &self.event,
        TallyEvent::LowAllowanceWarning(_) | TallyEvent::DelegateMismatchWarning(_)
    )
}

/// Check if this is an admin operation
pub const fn is_admin_operation(&self) -> bool {
    matches!(
        &self.event,
        TallyEvent::ConfigInitialized(_)
            | TallyEvent::ConfigUpdated(_)
            | TallyEvent::FeesWithdrawn(_)
            | TallyEvent::ProgramPaused(_)
            | TallyEvent::ProgramUnpaused(_)
    )
}
```

### Phase 8: Update TallyReceipt Helper Methods

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs` (lines 608-659)

Add getter methods for all new event types:

```rust
impl TallyReceipt {
    // Existing methods...

    pub fn get_subscription_reactivated_event(&self) -> Option<&SubscriptionReactivated> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::SubscriptionReactivated(e) => Some(e),
            _ => None,
        })
    }

    pub fn get_subscription_closed_event(&self) -> Option<&SubscriptionClosed> {
        self.events.iter().find_map(|event| match event {
            TallyEvent::SubscriptionClosed(e) => Some(e),
            _ => None,
        })
    }

    // ... continue for all 16 new event types

    pub fn get_trial_started_event(&self) -> Option<&TrialStarted> { /* ... */ }
    pub fn get_trial_converted_event(&self) -> Option<&TrialConverted> { /* ... */ }
    pub fn get_plan_created_event(&self) -> Option<&PlanCreated> { /* ... */ }
    pub fn get_plan_status_changed_event(&self) -> Option<&PlanStatusChanged> { /* ... */ }
    pub fn get_plan_terms_updated_event(&self) -> Option<&PlanTermsUpdated> { /* ... */ }
    pub fn get_merchant_initialized_event(&self) -> Option<&MerchantInitialized> { /* ... */ }
    pub fn get_merchant_tier_changed_event(&self) -> Option<&MerchantTierChanged> { /* ... */ }
    pub fn get_config_initialized_event(&self) -> Option<&ConfigInitialized> { /* ... */ }
    pub fn get_config_updated_event(&self) -> Option<&ConfigUpdated> { /* ... */ }
    pub fn get_fees_withdrawn_event(&self) -> Option<&FeesWithdrawn> { /* ... */ }
    pub fn get_program_paused_event(&self) -> Option<&ProgramPaused> { /* ... */ }
    pub fn get_program_unpaused_event(&self) -> Option<&ProgramUnpaused> { /* ... */ }
    pub fn get_low_allowance_warning_event(&self) -> Option<&LowAllowanceWarning> { /* ... */ }
    pub fn get_delegate_mismatch_warning_event(&self) -> Option<&DelegateMismatchWarning> { /* ... */ }
}
```

### Phase 9: Update Tests

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs` (lines 661-1106)

Add tests for all new event types:

1. Update `test_get_event_discriminators()` to expect 20 discriminators instead of 4
2. Add parse tests for each new event type (following existing pattern)
3. Add integration tests for helper methods with new events
4. Add edge case tests for events with optional fields (DelegateMismatchWarning, PlanTermsUpdated)

Example test structure:
```rust
#[test]
fn test_parse_subscription_reactivated_event() {
    let merchant = Pubkey::from(Keypair::new().pubkey().to_bytes());
    let plan = Pubkey::from(Keypair::new().pubkey().to_bytes());
    let subscriber = Pubkey::from(Keypair::new().pubkey().to_bytes());

    let event = SubscriptionReactivated {
        merchant,
        plan,
        subscriber,
        amount: 10_000_000,
        total_renewals: 5,
        original_created_ts: 1_640_995_200,
    };

    let encoded_data = create_test_event_data("SubscriptionReactivated", &event);
    let parsed_event = parse_single_event(&encoded_data).unwrap();

    match parsed_event {
        TallyEvent::SubscriptionReactivated(parsed) => {
            assert_eq!(parsed.merchant, merchant);
            assert_eq!(parsed.plan, plan);
            assert_eq!(parsed.subscriber, subscriber);
            assert_eq!(parsed.amount, 10_000_000);
            assert_eq!(parsed.total_renewals, 5);
            assert_eq!(parsed.original_created_ts, 1_640_995_200);
        }
        _ => panic!("Expected SubscriptionReactivated event"),
    }
}
```

### Phase 10: Update Public Exports

**File**: `/home/rodzilla/projects/tally/tally-protocol/sdk/src/lib.rs`

Ensure all new event types are exported:

```rust
pub use events::{
    // Existing exports
    Subscribed,
    Renewed,
    Canceled,
    PaymentFailed,

    // New exports
    SubscriptionReactivated,
    SubscriptionClosed,
    PlanStatusChanged,
    ConfigInitialized,
    MerchantInitialized,
    PlanCreated,
    ProgramPaused,
    ProgramUnpaused,
    LowAllowanceWarning,
    FeesWithdrawn,
    DelegateMismatchWarning,
    ConfigUpdated,
    MerchantTierChanged,
    PlanTermsUpdated,
    TrialStarted,
    TrialConverted,

    // Enum and helper types
    MerchantTier,
    TallyEvent,
    ParsedEventWithContext,
    StreamableEventData,
    TallyReceipt,
    ReceiptParams,

    // Functions
    parse_events_with_context,
    parse_events_from_logs,
    parse_single_event,
    create_receipt,
    create_receipt_legacy,
    extract_memo_from_logs,
};
```

## Testing Strategy

### Unit Tests

1. **Discriminator Tests**
   - Verify all 20 event discriminators are unique
   - Verify discriminators are deterministic
   - Verify discriminator computation matches Anchor's implementation

2. **Parsing Tests**
   - Test each event type can be serialized and deserialized
   - Test malformed data handling for each event type
   - Test events with optional fields (None and Some cases)
   - Test events with complex types (MerchantTier enum)

3. **Helper Method Tests**
   - Test all getter methods return correct values
   - Test helper methods handle all event types
   - Test event classification methods (is_warning, is_admin_operation, etc.)

### Integration Tests

1. **Multi-Event Logs**
   - Test parsing logs with multiple different event types
   - Test parsing logs with events from different programs
   - Test parsing logs with some malformed events (should skip gracefully)

2. **Receipt Generation**
   - Test receipt creation with all event types
   - Test receipt getter methods for all events
   - Test receipt helper methods (is_subscription_success, etc.)

3. **Streamable Conversion**
   - Test to_streamable() for all event types
   - Test metadata extraction for all event types
   - Test timestamp formatting for events with timestamps

### Live Testing

1. **Program Integration**
   - Deploy test program and trigger all 20 event types
   - Verify SDK can parse all events from real transactions
   - Verify event fields match program state changes

2. **Event Query Testing**
   - Test event_query module with all new event types
   - Verify filtering works for all event types
   - Verify pagination and sorting with new events

## Edge Cases to Handle

### 1. Optional Fields

**Events with Optional Fields**:
- `DelegateMismatchWarning.actual_delegate: Option<Pubkey>`
- `PlanTermsUpdated` - all old/new fields are Option types

**Handling**: Ensure Borsh deserialization handles Option correctly. Test both None and Some cases.

### 2. String Fields

**Events with String Fields**:
- `PaymentFailed.reason`
- `PlanStatusChanged.changed_by`
- `PlanCreated.plan_id`
- `PlanCreated.name`

**Handling**: Ensure proper UTF-8 validation. Test with empty strings, unicode, and max-length strings.

### 3. Enum Fields

**Events with Enum Fields**:
- `MerchantTierChanged.old_tier: MerchantTier`
- `MerchantTierChanged.new_tier: MerchantTier`

**Handling**: Ensure MerchantTier enum matches program definition exactly. Test all three variants.

### 4. Large Structs

**Events with Many Fields**:
- `ConfigInitialized` (9 fields)
- `ConfigUpdated` (6 fields)
- `PlanTermsUpdated` (7 fields)

**Handling**: Ensure proper memory layout. Test serialization/deserialization with all fields populated.

### 5. Backwards Compatibility

**Critical**: The Renewed event field addition breaks backwards compatibility.

**Impact**:
- Existing code parsing old Renewed events will fail with new SDK
- New SDK cannot parse old Renewed events from historical transactions

**Mitigation Options**:
1. **Version the SDK** - Bump major version to indicate breaking change
2. **Provide migration guide** - Document the breaking change clearly
3. **Consider dual parsing** - Try parsing with old struct first, then new (complex)

**Recommendation**: Bump major version (e.g., 0.1.0 → 0.2.0) and document the breaking change.

### 6. Event Discriminator Collisions

**Risk**: Low but non-zero chance of hash collisions in 8-byte discriminators.

**Mitigation**: The test suite verifies all 20 discriminators are unique. If collision detected, would need to rename event (extremely unlikely).

## Quality Checklist

Before marking this task complete:

- [ ] All 16 new event structs added with correct field types
- [ ] Renewed event updated with keeper and keeper_fee fields
- [ ] MerchantTier enum added and matches program definition
- [ ] TallyEvent enum updated with all 20 variants
- [ ] get_event_discriminators() includes all 20 events
- [ ] parse_single_event() handles all 20 event types
- [ ] All ParsedEventWithContext helper methods updated
- [ ] All TallyReceipt getter methods added
- [ ] New helper methods added (get_keeper, get_keeper_fee, etc.)
- [ ] All public exports added to lib.rs
- [ ] Zero clippy lints
- [ ] All tests pass (cargo nextest run --package tally-sdk)
- [ ] Code formatted (cargo fmt --package tally-sdk)
- [ ] Documentation comments complete for all new public items
- [ ] No unsafe code blocks
- [ ] DRY principles followed
- [ ] Comprehensive test coverage for new events
- [ ] Edge cases tested (optional fields, enums, strings)
- [ ] Breaking change documented (Renewed event)

## Completion Criteria

This task is complete when:

1. SDK can parse all 20 event types emitted by the program
2. All event structs match program definitions exactly
3. All tests pass with zero clippy warnings
4. Documentation is complete and accurate
5. Breaking changes are clearly documented
6. Integration tests verify real transaction parsing

## Estimated Timeline

- Phase 1-2 (Fix Renewed + Add MerchantTier): 30 minutes
- Phase 3 (Add 16 event structs): 45 minutes
- Phase 4-6 (Update enum, discriminators, parser): 30 minutes
- Phase 7-8 (Update helper methods): 45 minutes
- Phase 9 (Add comprehensive tests): 60 minutes
- Phase 10 (Exports and documentation): 15 minutes
- Testing and validation: 30 minutes

**Total**: 3.5-4 hours

## Next Steps After Implementation

1. Update event_query module to support filtering by new event types
2. Update dashboard to display new event types
3. Add alerting for warning events (LowAllowanceWarning, DelegateMismatchWarning)
4. Consider adding event-specific analytics (trial conversion rates, tier changes, etc.)
5. Update CLI to query and display new event types
