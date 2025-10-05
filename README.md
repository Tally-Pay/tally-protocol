# Tally Protocol

A Solana-native subscription platform implementing delegate-based recurring payments using SPL Token delegate approvals. Tally enables merchants to create subscription plans and collect automated USDC payments without requiring user signatures on each renewal.

**Tally Pay** is an organization of [Govcraft](https://govcraft.ai) enterprise.
- Web3 App: [tallybl.ink](https://tallybl.ink)
- Organization: [tallypay.click](https://tallypay.click)

## Overview

Tally Protocol provides a complete subscription infrastructure on Solana with:

- **On-Chain Program**: Anchor-based Solana program for subscription management
- **Rust SDK**: Comprehensive SDK for building subscription integrations
- **TypeScript Packages**: IDL and type definitions for web integrations

> **Note**: The CLI tool has been moved to a separate repository: [tally-cli](https://github.com/Tally-Pay/tally-cli)

### Key Features

- **Delegate-Based Payments**: Users approve once, renewals happen automatically
- **USDC Native**: Built on SPL Token standard with USDC support
- **Flexible Plans**: Configure pricing, periods, and grace periods
- **Platform Fees**: Configurable merchant fees with admin controls
- **Event System**: Comprehensive event logging for subscriptions
- **Dashboard API**: Real-time subscription metrics and analytics

## Project Structure

```
tally-protocol/
├── program/           # Solana program (Anchor)
│   └── src/
│       ├── lib.rs                    # Program entry point
│       ├── state.rs                  # Account structures
│       ├── init_config.rs            # Global config initialization
│       ├── init_merchant.rs          # Merchant registration
│       ├── create_plan.rs            # Subscription plan creation
│       ├── start_subscription.rs     # New subscription with delegate
│       ├── renew_subscription.rs     # Automated renewal via delegate
│       ├── cancel_subscription.rs    # Subscription cancellation
│       ├── admin_withdraw_fees.rs    # Platform fee withdrawal
│       ├── events.rs                 # Event definitions
│       └── errors.rs                 # Error types
├── sdk/              # Rust SDK
│   └── src/
│       ├── lib.rs                    # SDK entry point
│       ├── simple_client.rs          # High-level client API
│       ├── transaction_builder.rs    # Transaction construction
│       ├── pda.rs                    # PDA computation utilities
│       ├── ata.rs                    # Associated token account helpers
│       ├── events.rs                 # Event parsing
│       ├── event_query.rs            # Event querying with caching
│       ├── dashboard.rs              # Dashboard data aggregation
│       ├── validation.rs             # Input validation
│       └── error.rs                  # SDK error types
├── packages/         # TypeScript packages
│   ├── idl/          # Program IDL
│   ├── sdk/          # TypeScript SDK (WIP)
│   └── types/        # Type definitions (WIP)
└── examples/         # Usage examples (WIP)
```

## Installation

### Prerequisites

- Rust 1.75+ with Cargo
- Solana CLI 1.18+
- Anchor CLI 0.31.1
- Node.js 18+ with pnpm (for TypeScript packages)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/Tally-Pay/tally-protocol
cd tally-protocol

# Build the entire workspace
cargo build --release

# Build specific components
cargo build -p tally_subs    # Solana program
cargo build -p tally-sdk     # Rust SDK
# CLI tool is now in a separate repository: https://github.com/Tally-Pay/tally-cli

# Build TypeScript packages
pnpm install
pnpm build
```

### Running Tests

```bash
# Run all tests
cargo nextest run

# Test specific packages
cargo nextest run -p tally_subs
cargo nextest run -p tally-sdk
# CLI tests are now in tally-cli repository
```

## Quick Start

### 1. Deploy the Program

```bash
# Build and deploy to localnet
anchor build
anchor deploy

# Or deploy to devnet
anchor deploy --provider.cluster devnet
```

**Program IDs:**
- Localnet: `Fwrs8tRRtw8HwmQZFS3XRRVcKBQhe1nuZ5heB4FgySXV`
- Devnet: `6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5`

### 2. Initialize Configuration

Install the CLI tool first:
```bash
cargo install --git https://github.com/Tally-Pay/tally-cli
```

Then initialize the configuration:
```bash
# Initialize global program config (admin only)
tally-cli init-config \
  --platform-authority <ADMIN_PUBKEY> \
  --max-fee-bps 1000 \
  --min-period 86400

# Initialize merchant account
tally-cli init-merchant \
  --usdc-mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --platform-fee-bps 500

# Create a subscription plan
tally-cli create-plan \
  --plan-id "premium" \
  --name "Premium Plan" \
  --price 10000000 \
  --period 2592000 \
  --grace 86400
```

For more CLI commands, see the [tally-cli repository](https://github.com/Tally-Pay/tally-cli).

### 3. Using the Rust SDK

```rust
use tally_sdk::{SimpleTallyClient, pda, ata};
use anchor_client::solana_sdk::signature::{Keypair, Signer};
use anchor_client::solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

// Initialize client
let client = SimpleTallyClient::new("https://api.devnet.solana.com")?;

// Compute addresses
let merchant = Keypair::new();
let merchant_pda = pda::merchant_address(&merchant.pubkey())?;
let plan_pda = pda::plan_address_from_string(&merchant_pda, "premium")?;

// Get merchant's USDC ATA
let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
let treasury_ata = ata::get_associated_token_address_for_mint(
    &merchant.pubkey(),
    &usdc_mint
)?;

// Fetch subscription data
let subscription = client.get_subscription(&plan_pda, &user.pubkey()).await?;
println!("Next renewal: {}", subscription.next_renewal_ts);
println!("Renewals: {}", subscription.renewals);
```

## Architecture

### Program Accounts

**Config Account** (PDA: `["config"]`)
- Global program configuration
- Platform authority and fee settings
- Min/max validation parameters

**Merchant Account** (PDA: `["merchant", authority]`)
- Merchant registration and settings
- USDC mint and treasury configuration
- Platform fee percentage

**Plan Account** (PDA: `["plan", merchant, plan_id]`)
- Subscription plan definition
- Pricing, period, and grace period
- Active/inactive status

**Subscription Account** (PDA: `["subscription", plan, subscriber]`)
- Individual user subscription state
- Next renewal timestamp
- Renewal count and last amount charged

### Payment Flow

1. **Start Subscription**
   - User approves USDC delegate to program
   - Program transfers initial payment
   - Creates subscription account with renewal schedule

2. **Automated Renewal** (via off-chain keeper)
   - Keeper calls `renew_subscription` when due
   - Program pulls funds via delegate approval
   - Updates next renewal timestamp
   - Emits renewal event

3. **Cancel Subscription**
   - User or merchant cancels subscription
   - Program revokes delegate approval
   - Marks subscription as inactive

### Fee Distribution

For each payment:
- **Merchant Fee**: `amount * (1 - platform_fee_bps / 10000)` → Merchant treasury
- **Platform Fee**: `amount * (platform_fee_bps / 10000)` → Platform fee vault

## CLI Tool

The CLI tool has been moved to a separate repository for easier distribution and maintenance.

**Repository**: [https://github.com/Tally-Pay/tally-cli](https://github.com/Tally-Pay/tally-cli)

**Installation**:
```bash
cargo install --git https://github.com/Tally-Pay/tally-cli
```

For complete CLI documentation and usage examples, please refer to the [tally-cli repository](https://github.com/Tally-Pay/tally-cli).

## SDK Features

### Transaction Building

The SDK provides high-level transaction builders:

```rust
use tally_sdk::transaction_builder::TransactionBuilder;

// Start subscription transaction
let tx = TransactionBuilder::start_subscription(
    &subscriber_keypair,
    &plan_pda,
    &usdc_mint,
    approval_amount,
)?;

// Cancel subscription transaction
let tx = TransactionBuilder::cancel_subscription(
    &subscriber_keypair,
    &plan_pda,
)?;
```

### Event Querying

```rust
use tally_sdk::event_query::EventQuery;

// Query subscription events with caching
let query = EventQuery::new(client, program_id);
let events = query.query_subscription_events(
    &subscription_pda,
    start_time,
    end_time
).await?;
```

### Dashboard Data

```rust
use tally_sdk::dashboard::Dashboard;

// Aggregate subscription metrics
let dashboard = Dashboard::new(&client);
let metrics = dashboard.get_merchant_metrics(&merchant_pda).await?;

println!("Active subscriptions: {}", metrics.active_count);
println!("Total revenue: {}", metrics.total_revenue);
println!("MRR: {}", metrics.monthly_recurring_revenue);
```

## Events

The program emits comprehensive events for off-chain indexing:

- **SubscriptionStarted**: New subscription created
- **SubscriptionRenewed**: Successful renewal payment
- **SubscriptionCancelled**: Subscription cancelled
- **PlanCreated**: New plan created
- **PlanDeactivated**: Plan deactivated
- **MerchantInitialized**: Merchant registered
- **FeesWithdrawn**: Platform fees withdrawn

## Development

### Code Quality

The project enforces strict code quality standards:

- **Zero Unsafe Code**: `#![forbid(unsafe_code)]` across all crates
- **Clippy Lints**: `all`, `pedantic`, `nursery` enabled
- **Test Coverage**: Comprehensive unit and integration tests
- **Test Runner**: Uses `cargo nextest` for parallel test execution

### Safety Standards

Following Solana SDK patterns:
- Arithmetic overflow checks in release builds
- No unsafe code blocks allowed
- Strict clippy lints for security-critical operations
- Comprehensive input validation

### Contributing

1. Fork the repository
2. Create a feature branch
3. Write tests for new functionality
4. Ensure `cargo nextest run` passes
5. Ensure `cargo clippy` shows no warnings
6. Submit a pull request

## Deployment

### Localnet

```bash
# Start local validator
solana-test-validator

# Deploy program
anchor build
anchor deploy

# Run CLI commands against localnet
tally-cli --url http://localhost:8899 <COMMAND>
```

### Devnet

```bash
# Configure CLI for devnet
solana config set --url https://api.devnet.solana.com

# Deploy to devnet
anchor deploy --provider.cluster devnet

# Run CLI commands
tally-cli --cluster devnet <COMMAND>
```

### Mainnet

**⚠️ Not recommended for production yet - under active development**

## Security

### Audit Status

This project has not been formally audited. Use at your own risk.

### Known Limitations

#### Multi-Merchant Subscriptions (SPL Token Architectural Limitation)

**Critical**: Users **cannot** have active subscriptions with multiple merchants using the same token account.

**Root Cause**: SPL Token accounts support only **one delegate at a time**. When a user:
1. Subscribes to Merchant A → Sets delegate to `PDA(merchant=A)`
2. Subscribes to Merchant B → **Overwrites** delegate to `PDA(merchant=B)`
3. Cancels subscription with Merchant B → **Revokes** all delegates

**Impact**: Merchant A's subscription becomes non-functional even though it appears active.

**This is a fundamental architectural limitation of SPL Token**, not a bug. It cannot be fixed without migrating to Token-2022 or implementing a global delegate architecture.

**Workarounds**:
- **Recommended**: Create separate token accounts for each merchant
- **Alternative**: Only subscribe to one merchant at a time per token account

**Detection**: The protocol emits `DelegateMismatchWarning` events when renewal attempts detect incorrect delegates.

**Full Details**: See [docs/MULTI_MERCHANT_LIMITATION.md](./docs/MULTI_MERCHANT_LIMITATION.md) for:
- Complete technical explanation
- Detailed workarounds and migration paths
- Implementation guidance for integrators

#### Other Limitations

- Relies on off-chain keeper for renewal timing
- Delegate approval must be maintained by users
- No automatic grace period recovery mechanism
- Platform fee changes don't affect existing subscriptions

### Reporting Issues

Please report security issues privately to the maintainers.

## License

MIT License - see LICENSE file for details

## Resources

- [Anchor Framework](https://www.anchor-lang.com/)
- [Solana Documentation](https://docs.solana.com/)
- [SPL Token Program](https://spl.solana.com/token)

## Support

For questions and support:
- GitHub Issues: [tally-protocol/issues](https://github.com/Tally-Pay/tally-protocol/issues)
- Web3 App: [tallybl.ink](https://tallybl.ink)
- Organization: [tallypay.click](https://tallypay.click)

---

**Status**: Active Development
**Version**: 0.1.0
**Last Updated**: 2025-10-01
