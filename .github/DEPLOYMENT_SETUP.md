# Devnet Deployment Setup

This guide explains how to set up GitHub secrets for automated devnet deployments.

## Required GitHub Organization Secrets

These secrets are configured at the **organization level** (`Tally-Pay` organization settings) and are available to all repositories:

### 1. `DEVNET_PROGRAM_ID`

**Purpose**: The on-chain program address for devnet.

**Value**: `6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5`

**How to create**:

```bash
# Set as org secret (requires admin permissions)
echo '6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5' | gh secret set DEVNET_PROGRAM_ID --org Tally-Pay --visibility all
```

### 2. `LOCALNET_PROGRAM_ID`

**Purpose**: The on-chain program address for localnet testing.

**Value**: `YourProgramIdHere111111111111111111111111111` (see Anchor.toml for actual value)

**How to create**:

```bash
# Set as org secret (use actual program ID from Anchor.toml)
echo 'YourProgramIdHere111111111111111111111111111' | gh secret set LOCALNET_PROGRAM_ID --org Tally-Pay --visibility all
```

### 3. `DEVNET_DEPLOYER_KEYPAIR`

**Purpose**: Wallet that pays for deployment fees and acts as the program upgrade authority.

**How to create**:

```bash
# Generate a new keypair (or use existing)
solana-keygen new --outfile ~/.config/solana/devnet-deployer.json

# Display the public address
solana address -k ~/.config/solana/devnet-deployer.json

# Fund the wallet on devnet (you'll need at least 5-10 SOL for deployments)
solana airdrop 5 $(solana address -k ~/.config/solana/devnet-deployer.json) --url devnet

# Get the keypair in array format for GitHub secret
cat ~/.config/solana/devnet-deployer.json

# Set as org secret
cat ~/.config/solana/devnet-deployer.json | gh secret set DEVNET_DEPLOYER_KEYPAIR --org Tally-Pay --visibility all
```

### 4. `DEVNET_PROGRAM_KEYPAIR`

**Purpose**: The program's keypair (only needed for initial deployment).

**How to get**:

```bash
# After running anchor build, the keypair is generated at:
cat target/deploy/tally_subs-keypair.json

# Set as org secret
cat target/deploy/tally_subs-keypair.json | gh secret set DEVNET_PROGRAM_KEYPAIR --org Tally-Pay --visibility all
```

**Note**: This is only used for the initial deployment. After that, upgrades use the deployer keypair as the upgrade authority.

## GitHub Environment Protection (Optional but Recommended)

1. Go to `Settings > Environments`
2. Create an environment named `devnet`
3. Add protection rules:
   - **Required reviewers**: Add team members who must approve deployments
   - **Wait timer**: Optional delay before deployment
   - **Deployment branches**: Restrict to specific branches/tags

## Creating a Signed Release

The workflow requires signed tags with the `program-v*.*.*` prefix to distinguish program deployments from SDK releases.

```bash
# Configure Git signing (one-time setup)
git config --global user.signingkey <your-gpg-key-id>
git config --global commit.gpgsign true
git config --global tag.gpgsign true

# Create and push a signed tag for program deployment
git tag -s program-v0.1.0 -m "Deploy program v0.1.0"
git push origin program-v0.1.0

# For SDK releases (separate workflow)
git tag -s sdk-v0.1.0 -m "Release SDK v0.1.0"
git push origin sdk-v0.1.0
```

## Workflow Behavior

### Initial Deployment
- Checks if program exists on devnet
- If not, performs initial deployment using `anchor deploy`
- Uses `DEVNET_PROGRAM_KEYPAIR` to create the program account

### Upgrades
- Checks if program exists on devnet
- If yes, performs upgrade using `anchor upgrade`
- Uses `DEVNET_DEPLOYER_KEYPAIR` as the upgrade authority
- Verifies the upgrade authority matches

## Tag Naming Convention

To avoid triggering multiple workflows, use prefixed tags:

- **`program-v*.*.*`** - Triggers devnet deployment only (e.g., `program-v0.1.0`)
- **`sdk-v*.*.*`** - Triggers SDK publish to crates.io only (e.g., `sdk-v0.1.0`)

## Monitoring Deployments

1. **GitHub Actions**: Watch the workflow run in the Actions tab
2. **Solana Explorer**: Check the program at https://explorer.solana.com/address/6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5?cluster=devnet
3. **CLI Verification**:
   ```bash
   solana program show 6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5 --url devnet
   ```

## Troubleshooting

### "Low SOL balance" warning
- Fund the deployer wallet: `solana airdrop 5 <deployer-address> --url devnet`

### "Tag is not signed" error
- Ensure you're creating signed tags with `git tag -s`
- Verify your GPG key is set up correctly

### "Upgrade authority mismatch" warning
- The deployer wallet must be the program's upgrade authority
- Check with: `solana program show <program-id> --url devnet`
- Update authority if needed: `solana program set-upgrade-authority <program-id> --new-upgrade-authority <deployer-address> --url devnet`

### Build failures
- Ensure all tests pass locally: `cargo nextest run -p tally-protocol`
- Verify clippy checks: `cargo clippy -p tally-protocol --all-targets -- -D warnings`

## Security Notes

- **Never commit keypairs** to the repository
- Use GitHub environment protection for production-like deploys
- Rotate deployer keypair periodically
- Monitor the deployer wallet balance
- After initial deployment, the program keypair secret can be removed (kept for re-deployments)

## Manual Deployment

If you need to deploy manually:

```bash
# Set cluster
solana config set --url devnet

# Set deployer keypair
solana config set --keypair ~/.config/solana/devnet-deployer.json

# Build
anchor build

# Upgrade existing program
anchor upgrade target/deploy/tally_protocol.so \
  --program-id 6jsdZp5TovWbPGuXcKvnNaBZr1EBYwVTWXW1RhGa2JM5 \
  --provider.cluster devnet

# OR: Initial deploy (first time only)
anchor deploy --provider.cluster devnet --program-name tally-protocol
```
