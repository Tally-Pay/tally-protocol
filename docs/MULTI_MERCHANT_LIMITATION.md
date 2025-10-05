# Multi-Merchant Subscription Limitation

## Overview

This document describes a **fundamental architectural limitation** of the Tally Protocol that stems from the SPL Token program's design. This is not a bug that can be fixed without a complete architectural redesign or migration to Token-2022 extensions.

## The Problem: SPL Token Single-Delegate Constraint

**Core Issue**: SPL Token accounts support **only one delegate at a time**.

In Tally Protocol, each merchant requires delegate approval on a user's token account to pull recurring subscription payments. When a user subscribes to a merchant, the protocol sets a merchant-specific program delegate PDA as the delegate on the user's USDC token account.

### Impact on Multi-Merchant Subscriptions

**Users cannot maintain active subscriptions with multiple merchants simultaneously using the same token account.**

When a user:
1. Starts a subscription with Merchant A → Sets delegate to `PDA(merchant=A)`
2. Starts a subscription with Merchant B → **Overwrites** delegate to `PDA(merchant=B)`
3. Cancels subscription with Merchant B → **Revokes** delegate entirely

**Result**: The subscription with Merchant A becomes non-functional because its delegate has been overwritten/revoked, even though the subscription account still shows `active = true`.

## Technical Details

### SPL Token Delegate Model

From the SPL Token program specification:

```rust
// TokenAccount structure (simplified)
pub struct Account {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub delegate: COption<Pubkey>,        // Only ONE delegate possible
    pub delegated_amount: u64,
    // ... other fields
}
```

The `delegate` field is an **Option<Pubkey>**, not a vector or mapping. The SPL Token program provides:

- `Approve`: Sets the delegate (overwrites any existing delegate)
- `Revoke`: Removes the delegate entirely

There is **no mechanism to have multiple simultaneous delegates** on a single token account.

### Tally Protocol Delegate Design

Tally uses merchant-specific delegate PDAs:

```rust
// Each merchant has a unique delegate PDA
PDA(seeds = [b"delegate", merchant.key().as_ref()])
```

**Why merchant-specific?**
- Security isolation: Each merchant can only access their own delegate
- Permission scoping: Delegates are scoped to specific merchant operations
- Audit trail: Clear attribution of token movements to specific merchants

### Failure Scenarios

#### Scenario 1: Delegate Overwrite
```
1. User approves delegate for Merchant A subscription
   Token Account: delegate = PDA(merchant=A), delegated_amount = 1000 USDC

2. User approves delegate for Merchant B subscription
   Token Account: delegate = PDA(merchant=B), delegated_amount = 1000 USDC
   ❌ Merchant A delegate is OVERWRITTEN

3. Merchant A renewal attempts
   ❌ FAILS: Expected delegate PDA(merchant=A), found PDA(merchant=B)
```

#### Scenario 2: Delegate Revocation
```
1. User has active subscriptions with Merchants A, B, and C
   Token Account: delegate = PDA(merchant=C), delegated_amount = 1000 USDC

2. User cancels subscription with Merchant C
   Token Account: delegate = None, delegated_amount = 0
   ❌ ALL merchant delegates are REVOKED

3. Renewals for Merchants A and B attempt
   ❌ FAILS: No delegate present
```

## Detection and Warning Mechanisms

The Tally Protocol implements detection logic to alert users and off-chain systems when delegate mismatches occur:

### DelegateMismatchWarning Event

Emitted during renewal when the expected delegate doesn't match the actual delegate:

```rust
#[event]
pub struct DelegateMismatchWarning {
    pub merchant: Pubkey,
    pub plan: Pubkey,
    pub subscriber: Pubkey,
    pub expected_delegate: Pubkey,
    pub actual_delegate: Option<Pubkey>,
}
```

**When emitted:**
- During `renew_subscription` if `subscriber_ata_data.delegate != expected_delegate_pda`
- Provides all details needed for off-chain systems to diagnose and notify users

**What to do when detected:**
- Notify user that their subscription with this merchant is non-functional
- Prompt user to reactivate the subscription (which will reset the delegate)
- Suggest using per-merchant token accounts (see workarounds below)

### Implementation Location

See `/home/rodzilla/projects/tally-protocol-m3/program/src/renew_subscription.rs` lines 196-210 for the detection logic implementation.

## Workarounds

### Option 1: Per-Merchant Token Accounts (Recommended)

**Solution**: Create separate USDC token accounts for each merchant subscription.

**How it works:**
```
User Wallet
├─ USDC Account A (for Merchant A subscriptions)
│  └─ delegate = PDA(merchant=A)
├─ USDC Account B (for Merchant B subscriptions)
│  └─ delegate = PDA(merchant=B)
└─ USDC Account C (for Merchant C subscriptions)
   └─ delegate = PDA(merchant=C)
```

**Advantages:**
- Full isolation: Each merchant's subscriptions are completely independent
- No delegate conflicts: Each token account has its own delegate
- Works with current SPL Token standard

**Disadvantages:**
- User experience complexity: Users must manage multiple token accounts
- Increased wallet setup: Requires creating additional ATAs
- Fund management: Users must distribute USDC across accounts

**Implementation:**
```bash
# Create dedicated token account for Merchant A
spl-token create-account <USDC_MINT> merchant-a-subscriptions.json

# Fund the account
spl-token transfer <USDC_MINT> <AMOUNT> <MERCHANT_A_TOKEN_ACCOUNT>

# Start subscription using dedicated account
tally-cli start-subscription \
  --plan <MERCHANT_A_PLAN> \
  --token-account <MERCHANT_A_TOKEN_ACCOUNT>
```

### Option 2: Single-Merchant Constraint (Current Behavior)

**Solution**: Accept that users can only subscribe to one merchant at a time per token account.

**How it works:**
- Users can only maintain active subscriptions with one merchant
- Starting a new subscription overwrites the previous merchant's delegate
- Canceling revokes the delegate for all merchants

**Advantages:**
- Simplest implementation
- No changes required to protocol or user workflow
- Transparent current behavior

**Disadvantages:**
- Severely limits protocol utility for users who want multiple subscriptions
- Poor user experience for multi-merchant scenarios
- Subscriptions appear active but become non-functional

**Best for:**
- Single-merchant use cases
- Pilot deployments
- Simple recurring payment scenarios

### Option 3: Manual Delegate Re-approval (Not Recommended)

**Solution**: Users manually re-approve delegates before each renewal.

**Why not recommended:**
- Defeats the purpose of automated recurring payments
- Requires user intervention for every renewal
- High likelihood of missed payments
- Poor user experience

## Future Migration Paths

### Token-2022 Extensions

The SPL Token-2022 program introduces extensions that could potentially address this limitation:

#### 1. Transfer Hook Extension

**Concept**: Implement a transfer hook that routes payments to different merchants based on subscription state.

**How it works:**
- Single global delegate PDA for all Tally subscriptions
- Transfer hook program determines correct merchant based on subscription account
- Routes payments to appropriate merchant treasury during transfer

**Advantages:**
- Single delegate for all merchants
- Maintains automated renewal functionality
- Supports unlimited merchants per token account

**Challenges:**
- Requires complete protocol redesign
- All merchants must migrate to Token-2022
- Transfer hook complexity and security considerations
- Gas cost implications for hook execution

**Migration effort:** High (6-12 months)

#### 2. Delegate Extension (Hypothetical)

**Concept**: Future Token-2022 extension supporting multiple simultaneous delegates.

**Status:** Not currently available in Token-2022 specification

**How it would work:**
```rust
// Hypothetical multiple delegate support
pub struct Account {
    pub delegates: Vec<(Pubkey, u64)>,  // Multiple delegates with amounts
}
```

**Advantages:**
- Direct solution to root problem
- Minimal protocol changes required
- Backward compatible migration path

**Challenges:**
- Extension doesn't exist yet
- Would require SPL governance approval
- Uncertain timeline for implementation

**Migration effort:** Unknown (depends on extension availability)

### Global Delegate Architecture

**Concept**: Single global delegate PDA that manages all merchant subscriptions.

**Design:**
```rust
// Single global delegate for all merchants
PDA(seeds = [b"global_delegate"])

// Subscription state tracks merchant-specific allowances
pub struct Subscription {
    pub merchant: Pubkey,
    pub allowed_amount: u64,
    pub last_withdrawal: i64,
    // ... other fields
}
```

**Advantages:**
- Solves multi-merchant problem without Token-2022
- Single delegate approval for all subscriptions
- Simpler user experience

**Challenges:**
- Complete architectural redesign required
- Security implications of global delegate
- Complex authorization logic needed
- Requires protocol upgrade and migration

**Migration effort:** High (4-8 months)

## Recommendations

### For Protocol Developers

1. **Document prominently**: Include this limitation in all user-facing documentation
2. **Implement detection**: Ensure `DelegateMismatchWarning` events are properly emitted
3. **UI warnings**: Display clear warnings when users attempt multi-merchant subscriptions
4. **Plan migration**: Evaluate Token-2022 transfer hook as long-term solution

### For Integration Developers

1. **Monitor events**: Listen for `DelegateMismatchWarning` events
2. **Notify users**: Alert users when delegate mismatches are detected
3. **Suggest workarounds**: Guide users toward per-merchant token accounts
4. **Validate state**: Check delegate before attempting renewals

### For End Users

1. **Single merchant**: Use one merchant per token account for simplicity
2. **Multiple accounts**: Create separate token accounts for each merchant if needed
3. **Watch for warnings**: Pay attention to renewal failure notifications
4. **Reactivate if needed**: Reactivate subscriptions if delegate issues occur

## Conclusion

The single-delegate limitation is an **inherent constraint of the SPL Token program**, not a Tally Protocol bug. It cannot be fixed without either:

1. Migrating to Token-2022 with transfer hooks (requires extensive redesign)
2. Implementing a global delegate architecture (requires protocol upgrade)
3. Accepting the per-merchant token account workaround (current best practice)

The Tally Protocol implements detection and warning mechanisms to make this limitation transparent and manageable. Users and integrators should be aware of this constraint and plan their subscription management accordingly.

## References

- [SPL Token Program Documentation](https://spl.solana.com/token)
- [Token-2022 Extensions](https://www.solana-program.com/docs/token-2022/extensions)
- [Transfer Hook Interface](https://github.com/solana-program/transfer-hook)
- Tally Protocol: `/home/rodzilla/projects/tally-protocol-m3/program/src/renew_subscription.rs`
- Tally Protocol: `/home/rodzilla/projects/tally-protocol-m3/program/src/cancel_subscription.rs`
- Tally Protocol: `/home/rodzilla/projects/tally-protocol-m3/program/src/start_subscription.rs`

---

**Document Version**: 1.0
**Last Updated**: 2025-10-05
**Audit Reference**: M-3 - Delegate Revocation Affects Multiple Merchants
