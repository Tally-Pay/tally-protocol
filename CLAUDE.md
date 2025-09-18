# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

You are working on **Tally**, a Solana-native subscription platform currently under active development. This is a monorepo with Rust and TypeScript components implementing a delegate-based USDC subscription model where users approve bounded allowances for recurring payments.

**Current Status**: The core architecture is implemented but the platform is still being built and refined. You may be asked to implement new features, fix bugs, optimize performance, or extend functionality.

## Architecture Overview

Tally implements a Blink-native subscription engine for Solana. The system allows merchants to post Subscribe Blinks, users approve bounded USDC allowances and get charged immediately, then a Keeper service handles automatic renewals by pulling from those allowances. Everything is standards-based using Solana Actions and SPL Token delegate approvals.

### Core Components

**Anchor Program** (`programs/subs/`): Single on-chain program implementing subscription logic with accounts for Merchant, Plan, and Subscription. Uses SPL Token delegate approvals for secure recurring payments.

**Actions API** (`actions-api/`): Unified Rust/Axum HTTP service serving both Solana Actions/Blinks endpoints and merchant dashboard functionality. Returns prebuilt transactions for subscribe/cancel flows. Includes wallet-based authentication, SurrealDB integration, and HTMX templates with Basecoat UI and Tailwind v4 styling.

**Keeper** (`keeper/`): Off-chain renewal worker that scans for due subscriptions and submits `renew_subscription` transactions. Includes metrics, exponential backoff, and batch processing.

**Tally SDK** (`crates/tally-sdk/`): Rust library providing IDL loading, PDA/ATA computation, transaction builders, and event parsing. Used by all Rust services.

**TypeScript Packages** (`packages/`):
- `idl/` - Generated IDL JSON (build artifact, checked in)
- `types/` - TypeScript types for accounts and API payloads
- `sdk/` - Client helpers for loading IDL, computing PDAs/ATAs

**CLI** (`tally-cli/`): Developer utilities for merchant setup, plan creation, and state inspection.

### Key Architectural Patterns

**Delegate-Based Payments**: Users approve USDC allowances (default 3x plan price) to a program PDA. The program acts as delegate to pull funds for subscriptions without requiring user signatures for each renewal.

**Actions/Blinks Integration**: All user interactions happen via Solana Actions - no custom frontend required. Subscribe/Cancel Blinks can be shared anywhere links render.

**Event-Driven Observability**: Program emits structured events (`Subscribed`, `Renewed`, `Canceled`, `PaymentFailed`) for off-chain monitoring and analytics.

**Unified Dashboard Architecture**: The actions-api service serves dual purposes - public Solana Actions/Blinks for users and private merchant dashboard for subscription management. Dashboard features include wallet-based authentication, real-time analytics, plan management, and subscription monitoring through HTMX-powered interfaces.

## Development Workflow

When working on Tally, you'll primarily be:
- Implementing new features in the Anchor program
- Extending the Actions API with new endpoints
- Enhancing the Keeper with better renewal logic
- Building out the merchant dashboard UI
- Adding TypeScript SDK functionality
- Writing and updating tests

### Initial Setup
```bash
# Install dependencies
pnpm install

# Setup Solana environment
solana config set --url localhost  # or devnet
solana-test-validator --reset &    # for local development

# Build and deploy program
anchor build
anchor deploy

# Alternative using Taskfile (recommended)
task setup
task build:program
task deploy:program
```

### Local USDC Setup
```bash
# Create USDC mint and fund wallet (localnet only)
USDC_MINT=$(spl-token create-token --decimals 6 | awk '/Creating token/ {print $3}')
USDC_ATA=$(spl-token create-account $USDC_MINT | awk '/Creating account/ {print $3}')
spl-token mint $USDC_MINT 1000000000  # 1,000 USDC

# Alternative using Taskfile
task env:setup-usdc
```

### Running Services

**Actions API**:
```bash
cargo run --package actions-api
# Serves on localhost:8787 by default
```

**Keeper** (renewal worker):
```bash
cargo run --package keeper
# Runs renewal loop every 30 seconds
```

**CLI Operations**:
```bash
# Initialize merchant
cargo run --package tally-cli -- init-merchant --authority $(solana address) --usdc $USDC_MINT --treasury $MERCHANT_TREASURY --fee-bps 50

# Create subscription plan
cargo run --package tally-cli -- create-plan --merchant <MERCHANT_PDA> --id pro --price 5000000 --period 2592000 --grace 432000

# List plans and subscriptions
cargo run --package tally-cli -- list-plans --merchant <MERCHANT_PDA>
cargo run --package tally-cli -- list-subs --plan <PLAN_PDA>
```

### Testing

**Program Tests** (Anchor):
```bash
anchor test
```

**TypeScript Tests** (Vitest):
```bash
cd tests && pnpm test
# or
anchor test
```

**Keeper Tests**:
```bash
cargo test --package keeper
```

### Code Quality

**Rust**:
```bash
cargo fmt              # Format code
cargo clippy --all-targets --all-features  # Lint
```

**TypeScript**:
```bash
pnpm lint              # ESLint + Prettier
```

## Key Data Flows

### Subscription Flow
1. User clicks Subscribe Blink → Actions API returns metadata
2. User confirms → Actions API returns transaction with `ApproveChecked` + `start_subscription`
3. Program transfers funds (merchant + platform fee), sets `next_renewal_ts`
4. Keeper scans for due renewals and submits `renew_subscription` transactions

### Cancellation Flow
1. User clicks Cancel Blink → Actions API returns `Revoke` + `cancel_subscription` transaction
2. Program marks subscription inactive, user's allowance is revoked

## Important Configuration

### Environment Variables
- **Actions API**: `PORT`, `PROGRAM_ID`, `USDC_MINT`, `RPC_URL`, `DATABASE_URL`
- **Keeper**: `RPC_URL`, `PROGRAM_ID`, `USDC_MINT`, `PLATFORM_USDC_TREASURY`, `JITO_TIP_LAMPORTS`, `RENEW_BATCH_SIZE`
- **Global**: Copy `.env.example` to `.env.local` for local development

### PDA Seeds
- Merchant: `["merchant", authority.key()]`
- Plan: `["plan", merchant.key(), plan_id.as_bytes()]`
- Subscription: `["subscription", plan.key(), subscriber.key()]`

### Task Runner
The project uses [go-task](https://taskfile.dev) for build automation. Key commands:
- `task setup` - Install dependencies and check prerequisites
- `task build` - Build all components
- `task dev` - Run development services
- `task test` - Run all tests
- `task env:setup-usdc` - Create local USDC mint
- `task logs` - View service logs

See `Taskfile.yml` for complete command reference.

## Security Considerations

- All USDC transfers use checked arithmetic and strict mint validation
- Delegate allowances are only used within program instructions
- Idempotency protection prevents double-charging in same slot
- Program holds no SOL, only acts as USDC delegate

## UI/Frontend Notes

The system uses HTMX fragments for any web UI, styled with Basecoat UI components and Tailwind v4 utilities. Configuration is centralized in `tailwind.config.ts`. Reference `htmx-docs.md` for fragment conventions and https://basecoatui.com/ for component patterns.

## Monitoring and Observability

Keeper exports Prometheus metrics for subscription renewals, failures, and system health. All services use structured JSON logging with consistent field names (`service`, `event`, `plan`, `sub`, `txSig`).

## Development Notes

- **PRD Reference**: See `solana-subscriptions-prd.md` for complete product requirements and specifications
- **Active Development**: This platform is being actively built - expect to implement missing features and optimize existing ones
- **Testing**: Always run tests after changes: `anchor test` for program tests, `cd tests && pnpm test` for integration tests
- **Code Quality**: Run `cargo fmt` and `cargo clippy` before committing Rust changes
- **Architecture Decisions**: Refer to the PRD for context on why certain design decisions were made
- **Future Extensions**: The PRD outlines V2+ features that may be implemented later (Token-2022, multi-chain, etc.)
