# Tally Protocol

A Solana-native recurring payments platform enabling automated USDC transfers through SPL Token delegate approvals. Tally implements delegate-based recurring payments, eliminating the need for user signatures on each payment while maintaining full user control.

## Overview

Tally Protocol provides a decentralized recurring payment system on Solana where:

- **Payees** create payment terms with flexible amounts and billing periods
- **Payers** approve multi-period USDC allowances through a global delegate
- **Keepers** execute payments permissionlessly via delegate transfers
- **Platform** earns volume-based fees while providing infrastructure and emergency controls

The protocol uses a **global delegate architecture** where all payers approve a single global delegate PDA, enabling multi-payee subscriptions from a single token account without delegate conflicts.

## Key Features

### Universal Recurring Payments
- Support for subscriptions, payroll, investments, grants, and more
- Hierarchical payment structures (company → department → employee → vendor)
- Multi-payee support from single token account (global delegate)
- Volume-based fee tiers reward growth (0.15-0.25%)
- Extensions add use-case-specific features

### Payee Capabilities
- Register with USDC treasury and automatic volume-based fee tiers
- Create unlimited payment terms with custom amounts and periods
- Update plan terms (price, period, grace period, name) without creating new plans
- Earn revenue with low platform fees (0.15-0.25% depending on volume)
- Automatic tier upgrades based on 30-day rolling volume

### Payer Experience
- Single delegate approval for all payments
- Subscribe to multiple payees without delegate conflicts
- Cancel payments anytime and revoke delegate access
- Close canceled agreements to reclaim rent (~0.00099792 SOL)
- Benefit from grace periods on failed payments (extension-specific)
- Maintain complete control over token approvals

### Platform Features
- Volume-based fee tiers (Standard: 0.25%, Growth: 0.20%, Scale: 0.15%)
- Reduced keeper incentives (0.15% payment fee)
- Emergency pause mechanism for platform protection
- Two-step authority transfer for platform governance
- Fee withdrawal and treasury management
- Composable extension architecture

### Technical Architecture
- Built with Anchor 0.31.1 on Solana 3.0
- Global delegate PDA enables multi-payee payments
- Supports both SPL Token and Token-2022 programs
- Forbids unsafe code with comprehensive clippy lints
- Implements checked arithmetic and explicit access controls
- Emits detailed events for off-chain indexing

## Project Structure

```
tally-protocol/
├── program/              # Anchor program (Solana smart contract)
│   └── src/
│       ├── lib.rs                    # Program entry point
│       ├── state.rs                  # Account structures (Merchant, Plan, Subscription)
│       ├── errors.rs                 # Custom error types
│       ├── events.rs                 # Event definitions
│       ├── constants.rs              # Protocol constants (volume tiers, fees)
│       ├── start_subscription.rs     # Start new subscription
│       ├── renew_subscription.rs     # Renew existing subscription
│       ├── cancel_subscription.rs    # Cancel subscription
│       ├── close_subscription.rs     # Close canceled subscription
│       ├── create_plan.rs            # Create subscription plan
│       ├── update_plan.rs            # Update plan status
│       ├── update_plan_terms.rs      # Update plan pricing/terms
│       ├── init_merchant.rs          # Initialize merchant (payee)
│       ├── init_config.rs            # Initialize global config
│       ├── update_config.rs          # Update global config
│       ├── admin_withdraw_fees.rs    # Withdraw platform fees
│       ├── transfer_authority.rs     # Initiate authority transfer
│       ├── accept_authority.rs       # Accept authority transfer
│       ├── cancel_authority_transfer.rs # Cancel authority transfer
│       ├── pause.rs                  # Emergency pause
│       ├── unpause.rs                # Disable pause
│       └── utils.rs                  # Shared utilities
│
├── sdk/                  # Rust SDK for program interaction
│   └── src/
│       ├── lib.rs                    # SDK entry point
│       ├── client.rs                 # Client for program calls
│       ├── accounts.rs               # Account fetching utilities
│       ├── pda.rs                    # PDA derivation (global delegate)
│       ├── transaction_builder.rs    # Transaction builders
│       └── utils.rs                  # Helper functions
│
├── packages/             # TypeScript/JavaScript packages
│   ├── idl/              # Program IDL definitions
│   ├── sdk/              # TypeScript SDK
│   └── types/            # Shared type definitions
│
├── examples/             # Usage examples
│   ├── subscribe/        # Subscribe to a plan
│   ├── cancel/           # Cancel a subscription
│   └── list-plans/       # List available plans
│
└── docs/                 # Documentation
    ├── FEE_STRUCTURE.md              # Volume-based fee structure guide
    ├── SUBSCRIPTION_LIFECYCLE.md     # Lifecycle management guide
    ├── SPAM_DETECTION.md             # Spam prevention strategies
    ├── RATE_LIMITING_STRATEGY.md     # Rate limiting implementation
    └── OPERATIONAL_PROCEDURES.md     # Platform operations guide
```

## Account Structure

### Config (138 bytes)
Global program configuration managed by platform authority.

**Fields:**
- `platform_authority` - Platform admin with governance rights
- `pending_authority` - Two-step authority transfer staging
- `platform_treasury` - USDC destination for platform fees
- `usdc_mint` - USDC token mint address
- `keeper_fee_bps` - Keeper incentive (15 bps = 0.15%)
- `min_platform_fee_bps` - Minimum platform fee (10 bps = 0.1%)
- `max_platform_fee_bps` - Maximum platform fee (50 bps = 0.5%)
- `max_grace_period_secs` - Maximum grace period
- `min_period_secs` - Minimum billing period length
- `is_paused` - Emergency pause status
- `bump` - PDA derivation seed

**PDA Derivation:** `["config", program_id]`

### Merchant (124 bytes)
Payee configuration with volume tracking.

**Fields:**
- `authority` - Payee admin (manages plans and settings)
- `usdc_mint` - Pinned USDC mint address
- `treasury_ata` - USDC ATA receiving payee revenue
- `platform_fee_bps` - Current platform fee rate (tier-based)
- `volume_tier` - Current volume tier (Standard/Growth/Scale)
- `monthly_volume_usdc` - Rolling 30-day payment volume
- `last_volume_update_ts` - Last volume calculation timestamp
- `bump` - PDA derivation seed

**PDA Derivation:** `["merchant", authority.key(), program_id]`

**Volume Tiers (auto-calculated):**
- **Standard**: Up to $10K monthly → 0.25% platform fee
- **Growth**: $10K-$100K monthly → 0.20% platform fee
- **Scale**: Over $100K monthly → 0.15% platform fee

### Plan (129 bytes)
Payment terms with pricing and billing configuration.

**Fields:**
- `merchant` - Merchant pubkey (plan owner)
- `plan_id` - Merchant-defined identifier
- `name` - Human-readable plan name
- `price_usdc` - Payment amount (USDC smallest units)
- `period_secs` - Billing period length (seconds)
- `grace_period_secs` - Payment failure grace period
- `active` - Plan accepts new subscriptions
- `created_ts` - Plan creation timestamp
- `bump` - PDA derivation seed

**PDA Derivation:** `["plan", merchant.key(), plan_id.as_bytes(), program_id]`

### Subscription (120 bytes)
Individual user payment agreement state.

**Fields:**
- `plan` - Plan pubkey
- `subscriber` - User pubkey (payer)
- `subscriber_usdc_account` - User's USDC token account
- `active` - Subscription status (active/canceled)
- `renewals` - Lifetime payment count (preserved across reactivations)
- `created_ts` - Original subscription creation timestamp
- `next_renewal_ts` - Next scheduled payment
- `last_renewed_ts` - Last successful payment timestamp
- `last_amount` - Last payment amount
- `in_trial` - Trial period status (extension-specific)
- `bump` - PDA derivation seed

**PDA Derivation:** `["subscription", plan.key(), subscriber.key(), program_id]`

**Note:** The `renewals` counter tracks lifetime payments across all sessions. See [Subscription Lifecycle](docs/SUBSCRIPTION_LIFECYCLE.md) for details.

### Delegate (Global)
Single global delegate PDA shared by all payees and payers.

**PDA Derivation:** `["delegate", program_id]`

**Architecture:**
- All payers approve the **same delegate**
- Enables multi-payee subscriptions from single token account
- No delegate conflicts when subscribing to multiple merchants
- Program validation ensures correct payee receives payment

## Payment Flow

### Initial Subscription
1. User calls `start_subscription` with USDC delegate approval
2. Program validates plan status and user balance
3. First payment transfers USDC to merchant treasury
4. Platform fee (0.15-0.25%) and keeper fee (0.15%) deducted
5. Subscription account created with `active = true`
6. Global delegate approval remains for automatic renewals
7. `Subscribed` or `SubscriptionReactivated` event emitted

### Renewals
1. Keeper calls `renew_subscription` when `current_time >= next_renewal_ts`
2. Program validates subscription status and global delegate approval
3. Payment transfers via delegate: Payer USDC → Keeper fee → Platform fee → Merchant treasury
4. Payee volume updated, tier potentially upgraded
5. Subscription updated: `renewals++`, `next_renewal_ts += period_secs`
6. `Renewed` event emitted with payment details

### Cancellation
1. User calls `cancel_subscription` to stop renewals
2. Global delegate approval optionally revoked on USDC account
3. Subscription marked `active = false`
4. `Canceled` event emitted

### Account Closure
1. User calls `close_subscription` on canceled subscription
2. Subscription account closed and rent reclaimed (~0.00099792 SOL)
3. `SubscriptionClosed` event emitted

### Fee Distribution
Each renewal payment is split sequentially:
1. **Keeper Fee**: 0.15% to renewal executor
2. **Platform Fee**: 0.15-0.25% (volume tier-based) to platform treasury
3. **Payee Revenue**: Remainder (99.60-99.70%) to payee treasury

Example (100 USDC renewal, Growth tier merchant):
- Keeper: 0.15 USDC (0.15%)
- Platform: 0.20 USDC (0.20%)
- Merchant: 99.65 USDC (99.65%)

## Volume Tier Mechanics

### How It Works
1. **30-Day Rolling Window**: Volume tracked over most recent 30 days
2. **Automatic Upgrades**: Tier upgrades when volume crosses threshold
3. **Automatic Downgrades**: Tier downgrades if volume drops below threshold
4. **Volume Reset**: After 30 days without payments, volume resets to zero

### Tier Thresholds
- **Standard → Growth**: $10,000 monthly volume
- **Growth → Scale**: $100,000 monthly volume

### Example Progression
```
Day 1-15: Process $5,000 → Standard tier (0.25%)
Day 16: Process $6,000 → Total $11K → Upgraded to Growth tier (0.20%)
Day 30: All future payments use 0.20% until volume drops below $10K
```

## Program Instructions

### Payee Operations
- `init_merchant` - Initialize payee account with treasury (auto Standard tier)
- `create_plan` - Create new payment terms with pricing and billing period
- `update_plan` - Toggle plan active status (does not affect existing subscriptions)
- `update_plan_terms` - Update plan price, period, grace period, or name

### Payer Operations
- `start_subscription` - Start new subscription or reactivate canceled subscription
- `renew_subscription` - Execute payment via delegate (permissionless)
- `cancel_subscription` - Cancel subscription and optionally revoke delegate
- `close_subscription` - Close canceled subscription and reclaim rent

### Platform Operations
- `init_config` - Initialize global program configuration (one-time)
- `update_config` - Update global parameters (keeper fee, rate limits, fee bounds)
- `admin_withdraw_fees` - Withdraw accumulated platform fees
- `transfer_authority` - Initiate two-step platform authority transfer
- `accept_authority` - Complete authority transfer as pending authority
- `cancel_authority_transfer` - Cancel pending authority transfer
- `pause` - Enable emergency pause (disables user operations)
- `unpause` - Disable emergency pause (re-enables user operations)

## Events

The program emits detailed events for off-chain indexing and analytics:

- `ConfigInitialized` - Global configuration created
- `ConfigUpdated` - Configuration parameters changed
- `MerchantInitialized` - New payee registered
- `VolumeTierUpgraded` - Payee tier upgraded based on volume
- `PlanCreated` - New payment terms created
- `PlanUpdated` - Plan status changed
- `PlanTermsUpdated` - Plan terms modified
- `Subscribed` - New subscription started
- `SubscriptionReactivated` - Canceled subscription reactivated
- `Renewed` - Subscription renewed successfully
- `Canceled` - Subscription canceled
- `SubscriptionClosed` - Subscription account closed
- `FeesWithdrawn` - Platform fees withdrawn
- `AuthorityTransferInitiated` - Authority transfer proposed
- `AuthorityTransferAccepted` - Authority transfer completed
- `AuthorityTransferCanceled` - Authority transfer canceled
- `Paused` - Emergency pause enabled
- `Unpaused` - Emergency pause disabled
- `DelegateMismatchWarning` - Payment failed due to delegate mismatch

## Development

### Prerequisites
- Rust 1.70+
- Solana CLI 3.0+
- Anchor CLI 0.31.1+
- Node.js 18+ (for TypeScript SDK)
- pnpm (for package management)

### Build Program
```bash
# Build the Anchor program
anchor build

# Run program tests (requires TALLY_PROGRAM_ID)
export TALLY_PROGRAM_ID=eUV3U3e6zdQRXmAJFrvEFF9qEdWvjnQMA9BRxJef4d7
anchor test

# Run Rust tests with nextest
cargo nextest run
```

### Build SDK
```bash
# Build Rust SDK
cd sdk
cargo build
cargo test

# Build TypeScript SDK
cd packages/sdk
pnpm install
pnpm build
```

### Deployment

#### Devnet
```bash
# Set program ID (required)
export TALLY_PROGRAM_ID=6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5

# Build program
anchor build

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

#### Localnet
```bash
# Set program ID (required)
export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111

# Start local validator
solana-test-validator

# Deploy to localnet
anchor deploy --provider.cluster localnet
```

### Testing
```bash
# Set program ID (required for all tests)
export TALLY_PROGRAM_ID=eUV3U3e6zdQRXmAJFrvEFF9qEdWvjnQMA9BRxJef4d7

# Run all tests
anchor test

# Run specific test file
anchor test tests/subscription.ts

# Run Rust unit tests with nextest (faster, better output)
cargo nextest run

# Run with code coverage
cargo llvm-cov nextest
```

## Security

### Audit Status
The program has undergone a comprehensive security audit. See [SECURITY_AUDIT_REPORT.md](SECURITY_AUDIT_REPORT.md) for complete findings and resolutions.

**Key Findings:**
- **Medium (1)**: SPL Token single-delegate limitation - RESOLVED via global delegate architecture
- **Low (3)**: All resolved through code improvements and documentation
- **Informational (4)**: All addressed with enhanced documentation and operational procedures

### Security Features
- `#![forbid(unsafe_code)]` - No unsafe Rust code allowed
- Comprehensive clippy lints (`arithmetic_side_effects`, `default_trait_access`)
- Checked arithmetic operations preventing overflow/underflow
- Explicit access control on all privileged instructions
- Two-step authority transfer preventing accidental ownership loss
- Emergency pause mechanism for platform protection
- Detailed event logging for transparency and auditability
- Global delegate architecture eliminates multi-merchant conflicts

### Global Delegate Security

The global delegate PDA is secure because:
- **Program validation is the security boundary**, not delegate uniqueness
- Each payment validated against specific subscription PDA
- Subscription PDAs derived from `[plan, subscriber]` - unforgeable
- Plan PDAs derived from `[merchant, plan_id]` - merchant-isolated
- Cannot pay wrong merchant (PDA validation fails)
- Cannot exceed approved amount (amount validation fails)
- Cannot pay before due date (timing validation fails)

See `.claude/GLOBAL_DELEGATE_REFACTOR.md` for detailed security analysis.

## Documentation

- [Fee Structure](docs/FEE_STRUCTURE.md) - Volume-based fee tiers and economics
- [Subscription Lifecycle](docs/SUBSCRIPTION_LIFECYCLE.md) - Complete lifecycle management guide
- [Spam Detection](docs/SPAM_DETECTION.md) - Spam prevention strategies
- [Rate Limiting Strategy](docs/RATE_LIMITING_STRATEGY.md) - Rate limiting implementation
- [Operational Procedures](docs/OPERATIONAL_PROCEDURES.md) - Platform operations guide
- [Security Audit Report](SECURITY_AUDIT_REPORT.md) - Comprehensive security audit
- [Global Delegate Architecture](.claude/GLOBAL_DELEGATE_REFACTOR.md) - Architecture specification
- [Recurring Payments Architecture](.claude/RECURRING_PAYMENTS_ARCHITECTURE.md) - Platform evolution

## Examples

Examples demonstrate common usage patterns (implementations coming soon):

- [Subscribe](examples/subscribe/README.md) - Start a subscription
- [Cancel](examples/cancel/README.md) - Cancel an active subscription
- [List Plans](examples/list-plans/README.md) - Query available plans

## SDK Usage

### Rust SDK
```rust
use tally_sdk::{TallyClient, accounts::*, pda};
use solana_sdk::signer::Signer;

// Initialize client
let client = TallyClient::new(rpc_url, payer)?;

// Get global delegate PDA
let (delegate_pda, _) = pda::delegate()?;

// Start a subscription
let subscription_pubkey = client.start_subscription(
    &plan_pubkey,
    &subscriber_usdc_account,
    approve_amount,
).await?;

// Cancel a subscription
client.cancel_subscription(&subscription_pubkey).await?;

// Renew a subscription (keeper)
client.renew_subscription(&subscription_pubkey).await?;
```

### TypeScript SDK
```typescript
import { TallyClient } from '@tally-protocol/sdk';
import { Connection, Keypair } from '@solana/web3.js';

// Initialize client
const connection = new Connection('https://api.devnet.solana.com');
const client = new TallyClient(connection, wallet);

// Start a subscription
const subscriptionPubkey = await client.startSubscription({
  plan: planPubkey,
  subscriberUsdcAccount: usdcAccount,
  approveAmount: amount,
});

// Cancel a subscription
await client.cancelSubscription(subscriptionPubkey);

// Renew a subscription (keeper)
await client.renewSubscription(subscriptionPubkey);
```

## Contributing

Contributions are welcome! Please follow these guidelines:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes with conventional commits (`git commit -S -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Standards
- All Rust code must pass `cargo clippy` with zero warnings
- All tests must pass via `cargo nextest run`
- Unsafe code is forbidden (`#![forbid(unsafe_code)]`)
- Follow existing code style and documentation patterns
- Sign all commits (`git commit -S`)

## License

Apache License 2.0 - see LICENSE file for details

## Support

- GitHub Issues: https://github.com/Tally-Pay/tally-protocol/issues
- Documentation: https://github.com/Tally-Pay/tally-protocol/tree/main/docs
- Security: Report vulnerabilities via GitHub Security Advisories

## Acknowledgments

Built with:
- [Anchor](https://www.anchor-lang.com/) - Solana development framework
- [Solana](https://solana.com/) - High-performance blockchain
- [SPL Token](https://spl.solana.com/) - Token program standards
