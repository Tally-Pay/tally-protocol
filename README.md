# Tally Protocol

A Solana-native subscription platform enabling merchants to collect recurring USDC payments through SPL Token delegate approvals. Tally implements delegate-based recurring payments, eliminating the need for user signatures on each renewal while maintaining full user control.

## Overview

Tally Protocol provides a decentralized subscription management system on Solana where:

- **Merchants** create subscription plans with flexible pricing and billing periods
- **Subscribers** approve multi-period USDC allowances through token delegates
- **Keepers** execute renewals permissionlessly via delegate transfers
- **Platform** earns fees while providing infrastructure and emergency controls

The protocol uses a single-delegate architecture where subscribers approve a merchant-specific delegate PDA for automatic payment collection, enabling seamless recurring billing without repeated user interactions.

## Key Features

### Merchant Capabilities
- Register with USDC treasury and configurable fee rates
- Create unlimited subscription plans with custom pricing and periods
- Update plan terms (price, period, grace period, name) without creating new plans
- Earn tiered revenue based on merchant tier (Free: 98%, Pro: 98.5%, Enterprise: 99%)
- Control plan availability and subscriber management

### Subscriber Experience
- Start subscriptions with single delegate approval
- Cancel subscriptions anytime and revoke delegate access
- Close canceled subscriptions to reclaim rent (~0.00099792 SOL)
- Benefit from grace periods on failed payments
- Maintain complete control over token approvals

### Platform Features
- Tiered merchant fee structure (2.0% / 1.5% / 1.0%)
- Configurable keeper incentives (0.5% renewal fee)
- Emergency pause mechanism for platform protection
- Two-step authority transfer for platform governance
- Fee withdrawal and treasury management

### Technical Architecture
- Built with Anchor 0.31.1 on Solana 3.0
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
│       ├── state.rs                  # Account structures
│       ├── errors.rs                 # Custom error types
│       ├── events.rs                 # Event definitions
│       ├── constants.rs              # Protocol constants
│       ├── start_subscription.rs     # Start new subscription
│       ├── renew_subscription.rs     # Renew existing subscription
│       ├── cancel_subscription.rs    # Cancel subscription
│       ├── close_subscription.rs     # Close canceled subscription
│       ├── create_plan.rs            # Create subscription plan
│       ├── update_plan.rs            # Update plan status
│       ├── update_plan_terms.rs      # Update plan pricing/terms
│       ├── init_merchant.rs          # Initialize merchant
│       ├── update_merchant_tier.rs   # Update merchant tier
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
│       ├── transactions.rs           # Transaction builders
│       ├── events.rs                 # Event parsing
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
    ├── SUBSCRIPTION_LIFECYCLE.md     # Lifecycle management guide
    ├── MULTI_MERCHANT_LIMITATION.md  # Single-delegate constraints
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
- `keeper_fee_bps` - Keeper incentive (basis points, max 100)
- `min_platform_fee_bps` - Minimum merchant tier fee (basis points)
- `max_platform_fee_bps` - Maximum merchant tier fee (basis points)
- `max_grace_period_secs` - Maximum subscription grace period
- `min_period_secs` - Minimum billing period length
- `is_paused` - Emergency pause status
- `bump` - PDA derivation seed

**PDA Derivation:** `["config", program_id]`

### Merchant (108 bytes)
Merchant-specific configuration and treasury.

**Fields:**
- `authority` - Merchant admin (manages plans and settings)
- `treasury` - USDC ATA receiving merchant revenue
- `platform_fee_bps` - Platform fee rate (tier-based)
- `bump` - PDA derivation seed

**PDA Derivation:** `["merchant", authority.key(), program_id]`

**Merchant Tiers:**
- Free: 200 bps (2.0% platform fee, 98% merchant revenue)
- Pro: 150 bps (1.5% platform fee, 98.5% merchant revenue)
- Enterprise: 100 bps (1.0% platform fee, 99% merchant revenue)

### Plan (129 bytes)
Subscription plan with pricing and billing configuration.

**Fields:**
- `merchant` - Merchant pubkey (plan owner)
- `plan_id` - Merchant-defined identifier
- `name` - Human-readable plan name
- `price_usdc` - Subscription price (USDC smallest units)
- `period_secs` - Billing period length (seconds)
- `grace_period_secs` - Payment failure grace period
- `active` - Plan accepts new subscriptions
- `created_ts` - Plan creation timestamp
- `bump` - PDA derivation seed

**PDA Derivation:** `["plan", merchant.key(), plan_id.as_bytes(), program_id]`

### Subscription (120 bytes)
Individual user subscription state.

**Fields:**
- `plan` - Plan pubkey
- `subscriber` - User pubkey (owns subscription)
- `subscriber_usdc_account` - User's USDC token account
- `active` - Subscription status (active/canceled)
- `renewals` - Lifetime renewal count (preserved across reactivations)
- `created_ts` - Original subscription creation timestamp
- `next_renewal_ts` - Next scheduled renewal
- `last_renewed_ts` - Last successful renewal timestamp
- `last_amount` - Last payment amount
- `in_trial` - Trial period status
- `bump` - PDA derivation seed

**PDA Derivation:** `["subscription", plan.key(), subscriber.key(), program_id]`

**Note:** The `renewals` counter tracks lifetime renewals across all sessions, not just the current active session. This design maintains complete historical records for loyalty programs and analytics. See [Subscription Lifecycle](docs/SUBSCRIPTION_LIFECYCLE.md) for details.

## Payment Flow

### Initial Subscription
1. User calls `start_subscription` with USDC delegate approval
2. Program validates plan status and user balance
3. First payment transfers USDC (deducting keeper fee on renewals only)
4. Subscription account created with `active = true`
5. Delegate approval remains for automatic renewals
6. `Subscribed` or `SubscriptionReactivated` event emitted

### Renewals
1. Keeper calls `renew_subscription` when `current_time >= next_renewal_ts`
2. Program validates subscription status and delegate approval
3. Payment transfers via delegate: User USDC → Keeper fee → Platform fee → Merchant treasury
4. Subscription updated: `renewals++`, `next_renewal_ts += period_secs`
5. `Renewed` event emitted with payment details

### Cancellation
1. User calls `cancel_subscription` to stop renewals
2. Delegate approval revoked on USDC account
3. Subscription marked `active = false`
4. `Canceled` event emitted

### Account Closure
1. User calls `close_subscription` on canceled subscription
2. Subscription account closed and rent reclaimed (~0.00099792 SOL)
3. `SubscriptionClosed` event emitted

### Fee Distribution
Each renewal payment is split sequentially:
1. **Keeper Fee**: 0.5% (configurable, max 1%) to renewal executor
2. **Platform Fee**: 1-2% (tier-based) to platform treasury
3. **Merchant Revenue**: Remainder (98-99%) to merchant treasury

Example (100 USDC renewal, Pro merchant):
- Keeper: 0.50 USDC (0.5%)
- Platform: 1.50 USDC (1.5%)
- Merchant: 98.00 USDC (98%)

## Program Instructions

### Merchant Operations
- `init_merchant` - Initialize merchant account with treasury and fee configuration
- `create_plan` - Create new subscription plan with pricing and billing terms
- `update_plan` - Toggle plan active status (does not affect existing subscriptions)
- `update_plan_terms` - Update plan price, period, grace period, or name
- `update_merchant_tier` - Change merchant tier and platform fee rate

### Subscriber Operations
- `start_subscription` - Start new subscription or reactivate canceled subscription
- `renew_subscription` - Execute renewal payment via delegate (permissionless)
- `cancel_subscription` - Cancel subscription and revoke delegate approval
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
- `MerchantInitialized` - New merchant registered
- `MerchantTierUpdated` - Merchant tier changed
- `PlanCreated` - New subscription plan created
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
- `DelegateMismatchWarning` - Renewal failed due to delegate mismatch

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

# Run program tests
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
export TALLY_PROGRAM_ID=eUV3U3e6zdQRXmAJFrvEFF9qEdWvjnQMA9BRxJef4d7

# Start local validator
solana-test-validator

# Deploy to localnet
anchor deploy --provider.cluster localnet
```

### Testing
```bash
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
- **Medium (1)**: SPL Token single-delegate limitation (architectural constraint, documented)
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

### Known Limitations

#### Single-Delegate Constraint (M-1)
SPL Token accounts support only one delegate at a time. Subscribing to multiple merchants using the same USDC account will overwrite previous delegate approvals, breaking existing subscriptions.

**Recommended Mitigation:**
- Use separate USDC token accounts for each merchant subscription
- Frontend UI should detect and warn about existing delegates
- Monitor `DelegateMismatchWarning` events for renewal failures

See [MULTI_MERCHANT_LIMITATION.md](docs/MULTI_MERCHANT_LIMITATION.md) for comprehensive details and integration guidance.

## Documentation

- [Subscription Lifecycle](docs/SUBSCRIPTION_LIFECYCLE.md) - Complete lifecycle management guide
- [Multi-Merchant Limitation](docs/MULTI_MERCHANT_LIMITATION.md) - Single-delegate constraint details
- [Spam Detection](docs/SPAM_DETECTION.md) - Spam prevention strategies
- [Rate Limiting Strategy](docs/RATE_LIMITING_STRATEGY.md) - Rate limiting implementation
- [Operational Procedures](docs/OPERATIONAL_PROCEDURES.md) - Platform operations guide
- [Security Audit Report](SECURITY_AUDIT_REPORT.md) - Comprehensive security audit

## Examples

Examples demonstrate common usage patterns (implementations coming soon):

- [Subscribe](examples/subscribe/README.md) - Start a subscription
- [Cancel](examples/cancel/README.md) - Cancel an active subscription
- [List Plans](examples/list-plans/README.md) - Query available plans

## SDK Usage

### Rust SDK
```rust
use tally_sdk::{TallyClient, accounts::*, transactions::*};
use solana_sdk::signer::Signer;

// Initialize client
let client = TallyClient::new(rpc_url, payer)?;

// Start a subscription
let subscription_pubkey = client.start_subscription(
    &plan_pubkey,
    &subscriber_usdc_account,
    &delegate_pubkey,
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
  delegate: delegatePubkey,
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
