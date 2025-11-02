# Tally SDK vs Program Disparity Analysis Report

**Generated:** 2025-11-02
**Scope:** Comprehensive comparison of `/home/rodzilla/projects/tally/tally-protocol/program/src/` vs `/home/rodzilla/projects/tally/tally-protocol/sdk/src/`

---

## Executive Summary

This report identifies **critical gaps** between the Anchor program implementation and the Rust SDK that prevent full SDK functionality. The most severe issues involve:

1. **Missing trial subscription support** in SDK types (CRITICAL)
2. **Missing 14 event types** in SDK event parser (CRITICAL)
3. **Missing program constants** for validation (HIGH)
4. **Type mismatches** in account structures (HIGH)
5. **Missing keeper field** in Renewed event (MEDIUM)

---

## 1. INSTRUCTIONS

### ✅ Coverage Status: COMPLETE

All 17 program instructions have corresponding SDK transaction builders.

| Instruction | Program | SDK Builder | Status |
|-------------|---------|-------------|--------|
| `init_config` | ✅ | ✅ `InitConfigBuilder` | ✅ Match |
| `init_merchant` | ✅ | ✅ `CreateMerchantBuilder` | ✅ Match |
| `create_plan` | ✅ | ✅ `CreatePlanBuilder` | ✅ Match |
| `start_subscription` | ✅ | ✅ `StartSubscriptionBuilder` | ⚠️ **Args Mismatch** |
| `renew_subscription` | ✅ | ✅ `RenewSubscriptionBuilder` | ✅ Match |
| `cancel_subscription` | ✅ | ✅ `CancelSubscriptionBuilder` | ✅ Match |
| `close_subscription` | ✅ | ✅ `CloseSubscriptionBuilder` | ✅ Match |
| `admin_withdraw_fees` | ✅ | ✅ `AdminWithdrawFeesBuilder` | ✅ Match |
| `transfer_authority` | ✅ | ✅ `TransferAuthorityBuilder` | ✅ Match |
| `accept_authority` | ✅ | ✅ `AcceptAuthorityBuilder` | ✅ Match |
| `cancel_authority_transfer` | ✅ | ✅ `CancelAuthorityTransferBuilder` | ✅ Match |
| `update_plan` | ✅ | ✅ `UpdatePlanBuilder` | ✅ Match |
| `pause` | ✅ | ✅ `PauseBuilder` | ✅ Match |
| `unpause` | ✅ | ✅ `UnpauseBuilder` | ✅ Match |
| `update_config` | ✅ | ✅ `UpdateConfigBuilder` | ✅ Match |
| `update_merchant_tier` | ✅ | ✅ `UpdateMerchantTierBuilder` | ✅ Match |
| `update_plan_terms` | ✅ | ✅ `UpdatePlanTermsBuilder` | ✅ Match |

### 🔴 CRITICAL: Instruction Argument Mismatch

**Location:** `/home/rodzilla/projects/tally/tally-protocol/sdk/src/program_types.rs:158-164`

**Issue:** `StartSubscriptionArgs` in SDK is missing the `trial_duration_secs` field that exists in the program.

**Program Definition** (`program/src/start_subscription.rs:75-79`):
```rust
pub struct StartSubscriptionArgs {
    pub allowance_periods: u8,
    pub trial_duration_secs: Option<u64>, // ⚠️ MISSING IN SDK
}
```

**SDK Definition** (`sdk/src/program_types.rs:158-164`):
```rust
pub struct StartSubscriptionArgs {
    /// Allowance periods multiplier (default 3)
    pub allowance_periods: u8,
    // ❌ Missing trial_duration_secs field
}
```

**Impact:**
- SDK cannot create trial subscriptions
- Serialization mismatch will cause transaction failures
- Users cannot access trial subscription feature via SDK

**Required Fix:**
```rust
// sdk/src/program_types.rs
pub struct StartSubscriptionArgs {
    pub allowance_periods: u8,
    pub trial_duration_secs: Option<u64>, // ADD THIS FIELD
}
```

---

## 2. ACCOUNTS

### ⚠️ Type Mismatches

#### Issue 1: Merchant.tier Type Mismatch

**Location:** `/home/rodzilla/projects/tally/tally-protocol/sdk/src/program_types.rs:56`

**Program Definition** (`program/src/state.rs:39`):
```rust
pub struct Merchant {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub treasury_ata: Pubkey,
    pub platform_fee_bps: u16,
    pub tier: MerchantTier,  // ✅ Uses MerchantTier enum
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum MerchantTier {
    Free,
    Pro,
    Enterprise,
}
```

**SDK Definition** (`sdk/src/program_types.rs:43-59`):
```rust
pub struct Merchant {
    pub authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub treasury_ata: Pubkey,
    pub platform_fee_bps: u16,
    pub tier: u8,  // ❌ Uses u8 instead of MerchantTier enum
    pub bump: u8,
}

// SDK has separate MerchantTier enum but doesn't use it in Merchant struct
pub enum MerchantTier {
    Free = 0,
    Pro = 1,
    Enterprise = 2,
}
```

**Impact:**
- Type safety lost when working with merchant tiers
- Requires manual conversion between u8 and enum
- SDK users must know discriminant values (0, 1, 2)

**Workaround:** SDK provides `MerchantTier::from_discriminant()` helper, but still requires manual conversion.

**Recommendation:** This is likely intentional for serialization compatibility. Consider adding a helper method to Merchant:
```rust
impl Merchant {
    pub fn tier_enum(&self) -> Option<MerchantTier> {
        MerchantTier::from_discriminant(self.tier)
    }
}
```

---

## 3. EVENTS

### 🔴 CRITICAL: 14 Event Types Missing from SDK

**Location:** `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs`

The SDK only supports **4 basic events** while the program emits **20 event types**.

#### Events in SDK (4 total):
```rust
pub enum TallyEvent {
    Subscribed(Subscribed),
    Renewed(Renewed),
    Canceled(Canceled),
    PaymentFailed(PaymentFailed),
}
```

#### Missing Events (14 total):

| Event Name | Program Location | Severity | Impact |
|------------|------------------|----------|--------|
| `ConfigInitialized` | `program/src/events.rs:97-117` | HIGH | Cannot track config initialization |
| `ConfigUpdated` | `program/src/events.rs:268-286` | MEDIUM | Cannot track config changes |
| `MerchantInitialized` | `program/src/events.rs:120-134` | HIGH | Cannot track merchant registration |
| `MerchantTierChanged` | `program/src/events.rs:288-307` | MEDIUM | Cannot track tier upgrades/downgrades |
| `PlanCreated` | `program/src/events.rs:137-155` | HIGH | Cannot track new plan creation |
| `PlanStatusChanged` | `program/src/events.rs:84-94` | MEDIUM | Cannot track plan enable/disable |
| `PlanTermsUpdated` | `program/src/events.rs:309-337` | MEDIUM | Cannot track plan pricing changes |
| `ProgramPaused` | `program/src/events.rs:158-164` | HIGH | Cannot detect emergency pause |
| `ProgramUnpaused` | `program/src/events.rs:167-173` | HIGH | Cannot detect emergency unpause |
| `SubscriptionReactivated` | `program/src/events.rs:17-31` | HIGH | Cannot distinguish new vs reactivated subs |
| `SubscriptionClosed` | `program/src/events.rs:62-68` | MEDIUM | Cannot track rent reclamation |
| `TrialStarted` | `program/src/events.rs:340-360` | HIGH | Cannot track trial subscriptions |
| `TrialConverted` | `program/src/events.rs:363-381` | HIGH | Cannot track trial→paid conversion |
| `LowAllowanceWarning` | `program/src/events.rs:176-204` | MEDIUM | Cannot warn users about low allowance |
| `FeesWithdrawn` | `program/src/events.rs:207-224` | HIGH | Cannot audit platform fee withdrawals |
| `DelegateMismatchWarning` | `program/src/events.rs:227-265` | HIGH | Cannot detect multi-merchant conflicts |

**Impact:**
- **Dashboard**: Cannot display complete subscription lifecycle
- **Analytics**: Missing critical business metrics (trial conversion, churn)
- **Monitoring**: Cannot detect security events (pause/unpause, fee withdrawals)
- **User Experience**: Cannot warn users about allowance issues or delegate conflicts
- **Compliance**: Cannot audit fee withdrawals or config changes

**Required Fix:**

1. Add all missing event structs to `sdk/src/events.rs`
2. Extend `TallyEvent` enum with all variants
3. Update event discriminator mapping in `get_event_discriminators()`
4. Add parsing logic in `parse_single_event()`

**Example for TrialStarted:**
```rust
// Add to sdk/src/events.rs

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, AnchorSerialize, AnchorDeserialize)]
pub struct TrialStarted {
    pub subscription: Pubkey,
    pub subscriber: Pubkey,
    pub plan: Pubkey,
    pub trial_ends_at: i64,
}

pub enum TallyEvent {
    Subscribed(Subscribed),
    Renewed(Renewed),
    Canceled(Canceled),
    PaymentFailed(PaymentFailed),
    TrialStarted(TrialStarted),  // ADD THIS
    // ... add all 14 missing events
}

fn get_event_discriminators() -> HashMap<[u8; 8], &'static str> {
    let mut discriminators = HashMap::new();
    discriminators.insert(compute_event_discriminator("Subscribed"), "Subscribed");
    discriminators.insert(compute_event_discriminator("Renewed"), "Renewed");
    discriminators.insert(compute_event_discriminator("Canceled"), "Canceled");
    discriminators.insert(compute_event_discriminator("PaymentFailed"), "PaymentFailed");
    discriminators.insert(compute_event_discriminator("TrialStarted"), "TrialStarted");  // ADD THIS
    // ... add all 14 missing events
    discriminators
}
```

---

### 🟡 MEDIUM: Renewed Event Missing keeper Field

**Location:** `/home/rodzilla/projects/tally/tally-protocol/sdk/src/events.rs:27-39`

**Program Definition** (`program/src/events.rs:34-48`):
```rust
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub keeper: Pubkey,      // ⚠️ MISSING IN SDK
    pub keeper_fee: u64,     // ⚠️ MISSING IN SDK
}
```

**SDK Definition** (`sdk/src/events.rs:27-39`):
```rust
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    // ❌ Missing keeper and keeper_fee fields
}
```

**Impact:**
- Cannot track which keeper performed the renewal
- Cannot audit keeper fee distribution
- Analytics missing keeper performance metrics

**Required Fix:**
```rust
// sdk/src/events.rs
pub struct Renewed {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub amount: u64,
    pub keeper: Pubkey,      // ADD THIS
    pub keeper_fee: u64,     // ADD THIS
}
```

---

## 4. ERRORS

### ✅ Coverage Status: COMPLETE

All 30 error codes from the program are mapped in the SDK's error handling.

**Program Errors** (`program/src/errors.rs:9-168`): 30 error variants (6000-6029)

**SDK Error Mapping** (`sdk/src/error.rs:187-260`): Maps error codes 6012-6019 to specific variants

**Note:** SDK provides enhanced error mapping for specific error codes (6012-6019) with user-friendly error messages. Other error codes fall back to generic `TallyError::Anchor` wrapper.

**Coverage:**
- ✅ All program errors are handled (either specifically or via generic wrapper)
- ✅ User-friendly error messages for common errors
- ✅ Proper error propagation through `From` traits

---

## 5. PDAs

### ✅ Coverage Status: COMPLETE

All PDA derivation seeds match between program and SDK.

| PDA Type | Program Seeds | SDK Function | Status |
|----------|---------------|--------------|--------|
| Config | `["config"]` | `pda::config()` | ✅ Match |
| Merchant | `["merchant", authority]` | `pda::merchant()` | ✅ Match |
| Plan | `["plan", merchant, plan_id_bytes]` | `pda::plan()` | ✅ Match |
| Subscription | `["subscription", plan, subscriber]` | `pda::subscription()` | ✅ Match |
| Delegate | `["delegate", merchant]` | `pda::delegate()` | ✅ Match |

**Verification:**
- All seeds match exactly
- SDK provides both tuple `(Pubkey, u8)` and address-only `Pubkey` functions
- SDK provides string convenience functions for plan IDs
- All PDA computations use correct program ID from environment

---

## 6. CONSTANTS

### 🔴 HIGH: All Program Constants Missing from SDK

**Location:** Program constants defined in `/home/rodzilla/projects/tally/tally-protocol/program/src/constants.rs`

**Issue:** SDK has **zero constants module** while program defines 6 critical constants.

| Constant | Program Value | SDK Value | Severity |
|----------|---------------|-----------|----------|
| `FEE_BASIS_POINTS_DIVISOR` | `10_000` | ❌ Missing | HIGH |
| `ABSOLUTE_MIN_PERIOD_SECONDS` | `86400` (24 hours) | ❌ Missing | HIGH |
| `MAX_PLAN_PRICE_USDC` | `1_000_000_000_000` (1M USDC) | ❌ Missing | MEDIUM |
| `TRIAL_DURATION_7_DAYS` | `604_800` | ❌ Missing | HIGH |
| `TRIAL_DURATION_14_DAYS` | `1_209_600` | ❌ Missing | HIGH |
| `TRIAL_DURATION_30_DAYS` | `2_592_000` | ❌ Missing | HIGH |

**Impact:**
- **Client-side validation impossible**: SDK users cannot validate inputs before sending transactions
- **Poor UX**: Users get transaction failures instead of immediate validation errors
- **Code duplication**: SDK users must hardcode these values or extract from program
- **Trial subscriptions broken**: Cannot validate trial duration without constants

**Required Fix:**

Create `/home/rodzilla/projects/tally/tally-protocol/sdk/src/constants.rs`:

```rust
//! Program constants mirrored from the on-chain program
//!
//! These constants must match the program exactly for validation purposes.

/// Basis points divisor for percentage calculations (10,000 bp = 100%)
pub const FEE_BASIS_POINTS_DIVISOR: u128 = 10_000;

/// Absolute minimum subscription period (24 hours)
pub const ABSOLUTE_MIN_PERIOD_SECONDS: u64 = 86400;

/// Maximum plan price limit (1 million USDC with 6 decimals)
pub const MAX_PLAN_PRICE_USDC: u64 = 1_000_000_000_000;

/// Valid trial duration: 7 days
pub const TRIAL_DURATION_7_DAYS: u64 = 604_800;

/// Valid trial duration: 14 days
pub const TRIAL_DURATION_14_DAYS: u64 = 1_209_600;

/// Valid trial duration: 30 days
pub const TRIAL_DURATION_30_DAYS: u64 = 2_592_000;
```

Then export in `sdk/src/lib.rs`:
```rust
pub mod constants;
pub use constants::*;
```

**Validation Usage Example:**
```rust
use tally_sdk::constants::*;

// Client-side validation before sending transaction
if price_usdc > MAX_PLAN_PRICE_USDC {
    return Err("Price exceeds maximum allowed");
}

if period_secs < ABSOLUTE_MIN_PERIOD_SECONDS {
    return Err("Period must be at least 24 hours");
}

// Validate trial duration
match trial_duration {
    Some(TRIAL_DURATION_7_DAYS) => Ok(()),
    Some(TRIAL_DURATION_14_DAYS) => Ok(()),
    Some(TRIAL_DURATION_30_DAYS) => Ok(()),
    Some(_) => Err("Invalid trial duration, must be 7, 14, or 30 days"),
    None => Ok(()),
}
```

---

## 7. TYPES (Structs & Enums)

### ✅ Account Type Coverage: COMPLETE

All program account types are defined in SDK:
- ✅ `Config` (matches exactly)
- ✅ `Merchant` (tier field uses u8 instead of enum - see Section 2)
- ✅ `Plan` (matches exactly)
- ✅ `Subscription` (matches exactly)
- ✅ `MerchantTier` enum (defined but not used in Merchant struct)

### ✅ Instruction Args Coverage: COMPLETE (with 1 mismatch)

All program instruction argument types are defined in SDK:
- ✅ `InitConfigArgs` (matches exactly)
- ✅ `InitMerchantArgs` (matches exactly)
- ✅ `CreatePlanArgs` (matches exactly)
- ⚠️ `StartSubscriptionArgs` (missing `trial_duration_secs` - see Section 1)
- ✅ `RenewSubscriptionArgs` (matches exactly)
- ✅ `CancelSubscriptionArgs` (matches exactly)
- ✅ `CloseSubscriptionArgs` (matches exactly)
- ✅ `AdminWithdrawFeesArgs` (matches exactly)
- ✅ `TransferAuthorityArgs` (matches exactly)
- ✅ `AcceptAuthorityArgs` (matches exactly)
- ✅ `CancelAuthorityTransferArgs` (matches exactly)
- ✅ `UpdatePlanArgs` (matches exactly)
- ✅ `PauseArgs` (matches exactly)
- ✅ `UnpauseArgs` (matches exactly)
- ✅ `UpdateConfigArgs` (matches exactly)
- ✅ `UpdateMerchantTierArgs` (matches exactly)
- ✅ `UpdatePlanTermsArgs` (matches exactly)

---

## 8. UTILITIES

### ✅ SDK-Specific Utilities

The SDK provides additional utilities not present in the program (which is expected):

**Account Fetching** (`sdk/src/client.rs`):
- ✅ `TallyClient::fetch_merchant()`
- ✅ `TallyClient::fetch_plan()`
- ✅ `TallyClient::fetch_subscription()`
- ✅ `TallyClient::get_all_merchants()`
- ✅ `TallyClient::get_merchant_plans()`
- ✅ `TallyClient::get_plan_subscriptions()`

**General Utilities** (`sdk/src/utils.rs`):
- ✅ `micro_lamports_to_usdc()` - Currency conversion
- ✅ `usdc_to_micro_lamports()` - Currency conversion
- ✅ `basis_points_to_percentage()` - Fee calculation
- ✅ `is_valid_pubkey()` - Address validation
- ✅ `system_programs()` - Common program addresses
- ✅ `format_duration()` - Time formatting
- ✅ `calculate_next_renewal()` - Subscription math
- ✅ `is_renewal_due()` - Subscription status
- ✅ `is_subscription_overdue()` - Subscription status

**Event Utilities** (`sdk/src/events.rs`):
- ✅ `parse_events_from_logs()` - Log parsing
- ✅ `parse_events_with_context()` - Context-aware parsing
- ✅ `create_receipt()` - Receipt generation
- ✅ `extract_memo_from_logs()` - Memo extraction

**Transaction Utilities** (`sdk/src/transaction_utils.rs`):
- ✅ `build_transaction()` - Transaction construction
- ✅ `get_user_usdc_ata()` - ATA lookup
- ✅ `create_memo_instruction()` - Memo instruction builder

**ATA Utilities** (`sdk/src/ata.rs`):
- ✅ `get_associated_token_address_with_program()` - ATA computation
- ✅ `detect_token_program_for_mint()` - Token program detection

---

## Summary of Critical Issues

### 🔴 CRITICAL (Must Fix Immediately)

1. **StartSubscriptionArgs missing trial_duration_secs** - SDK cannot create trial subscriptions
   - **File:** `sdk/src/program_types.rs:158-164`
   - **Fix:** Add `pub trial_duration_secs: Option<u64>` field

2. **14 missing event types** - SDK cannot parse majority of program events
   - **File:** `sdk/src/events.rs`
   - **Fix:** Add all 14 missing event structs and update parser

3. **Renewed event missing keeper fields** - Cannot track keeper attribution
   - **File:** `sdk/src/events.rs:27-39`
   - **Fix:** Add `keeper: Pubkey` and `keeper_fee: u64` fields

### 🟡 HIGH (Should Fix Soon)

4. **All program constants missing** - No client-side validation possible
   - **File:** SDK has no constants module
   - **Fix:** Create `sdk/src/constants.rs` with all 6 constants

### 🟢 MEDIUM (Document or Accept)

5. **Merchant.tier type mismatch** - Uses u8 instead of enum
   - **File:** `sdk/src/program_types.rs:56`
   - **Status:** Likely intentional for serialization; workaround exists
   - **Fix:** Add helper method `tier_enum()` to Merchant impl

---

## Recommended Action Plan

### Phase 1: Trial Subscriptions (IMMEDIATE)
1. Add `trial_duration_secs: Option<u64>` to `StartSubscriptionArgs`
2. Add `TRIAL_DURATION_*` constants to SDK
3. Update transaction builders to support trial parameters
4. Test trial subscription flow end-to-end

### Phase 2: Event System (HIGH PRIORITY)
1. Add all 14 missing event struct definitions
2. Extend `TallyEvent` enum with all variants
3. Update event discriminator mapping
4. Add parsing logic for each new event
5. Update tests to cover all event types

### Phase 3: Constants & Validation (MEDIUM PRIORITY)
1. Create `constants.rs` module in SDK
2. Add all program constants
3. Add client-side validation utilities
4. Document validation patterns

### Phase 4: Type Safety Improvements (LOW PRIORITY)
1. Add `tier_enum()` helper to Merchant
2. Consider creating typed wrappers for common operations
3. Add integration tests comparing SDK vs program

---

## Testing Recommendations

### Critical Test Gaps

1. **Trial Subscription Flow**: No SDK tests exist for trial subscriptions (feature is not accessible)
2. **Event Parsing**: Only 4 of 20 event types have test coverage
3. **Constant Validation**: No tests validate that SDK constants match program
4. **Keeper Attribution**: No tests verify keeper field in Renewed events

### Recommended Test Suite

```rust
// sdk/tests/trial_subscriptions.rs
#[test]
fn test_start_trial_subscription_7_days() {
    let args = StartSubscriptionArgs {
        allowance_periods: 3,
        trial_duration_secs: Some(TRIAL_DURATION_7_DAYS),
    };
    // Test serialization matches program expectation
    // Test transaction success
    // Verify TrialStarted event emitted
}

// sdk/tests/event_parsing.rs
#[test]
fn test_parse_all_event_types() {
    // Test parsing for all 20 event types
    // Verify discriminators match program
    // Ensure no events are lost during parsing
}

// sdk/tests/constants.rs
#[test]
fn test_constants_match_program() {
    // Compare SDK constants against program constants
    // Fail if any mismatch detected
}
```

---

## Files Requiring Updates

### SDK Files to Modify

1. **`sdk/src/program_types.rs`**
   - Line 158-164: Add `trial_duration_secs` to `StartSubscriptionArgs`

2. **`sdk/src/events.rs`**
   - Line 27-39: Add `keeper` and `keeper_fee` to `Renewed`
   - Add 14 new event struct definitions
   - Line 70-80: Extend `TallyEvent` enum with 14 variants
   - Line 334-344: Update discriminator map with 14 events
   - Line 458-486: Add parsing logic for 14 events

3. **`sdk/src/constants.rs`** (NEW FILE)
   - Create new file with all 6 program constants

4. **`sdk/src/lib.rs`**
   - Add `pub mod constants;`
   - Add `pub use constants::*;` to re-exports

5. **`sdk/src/transaction_builder.rs`**
   - Update `StartSubscriptionBuilder` to accept trial duration parameter

### Program Files (No Changes Required)

All program files are correctly implemented. SDK must be updated to match.

---

## Conclusion

The Tally SDK has **excellent coverage of core functionality** (instructions, PDAs, basic events, errors) but **critical gaps** exist for:

1. **Trial subscriptions** - Completely inaccessible via SDK
2. **Event monitoring** - 70% of events unparseable (14 of 20 missing)
3. **Client-side validation** - No constants available

These gaps prevent production use of trial subscriptions and comprehensive event monitoring. The recommended fixes are straightforward and follow existing patterns in the SDK.

**Estimated Fix Effort:**
- Phase 1 (Trial Support): 2-4 hours
- Phase 2 (Events): 6-8 hours
- Phase 3 (Constants): 1-2 hours
- Phase 4 (Type Safety): 2-3 hours
- **Total**: 11-17 hours of development + testing

All fixes maintain backward compatibility and follow existing SDK patterns.
