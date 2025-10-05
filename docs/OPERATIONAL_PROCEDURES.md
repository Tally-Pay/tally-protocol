# Operational Procedures - Tally Protocol

## Platform Treasury ATA Management

### Critical Requirement: Treasury ATA Permanence

**CRITICAL:** The platform treasury Associated Token Account (ATA) MUST NEVER be closed or modified after initialization. Failure to maintain this account will cause a complete denial-of-service (DOS) for ALL subscription operations across the entire protocol.

**Severity:** CRITICAL - Complete Protocol Failure
**Impact:** All subscription starts, renewals, and fee distributions will fail
**Recovery Time:** Minutes to hours depending on operational readiness
**Affected Operations:** `start_subscription`, `renew_subscription`, any operation transferring platform fees

---

## Understanding the Treasury ATA Dependency

### Initialization Validation (One-Time Check)

During protocol initialization via `init_config`, the platform treasury ATA is validated to ensure:
- It exists as a valid SPL token account
- It is the canonical ATA for the platform authority + USDC mint
- It is owned by the platform authority
- It is configured for the correct USDC mint

This validation occurs ONCE during initialization and establishes the permanent treasury ATA address.

### Runtime Validation (Every Subscription Operation)

Every subscription operation (`start_subscription`, `renew_subscription`) performs runtime validation of the platform treasury ATA to ensure it remains valid. However, if the ATA has been closed, these validations will fail, causing:

**Immediate Impact:**
- All new subscription starts fail with `InvalidPlatformTreasuryAccount`
- All subscription renewals fail with `InvalidPlatformTreasuryAccount`
- Platform fee collection completely halted
- Merchant operations blocked (fees cannot be split)
- Complete protocol DOS until recovery

**User Experience:**
- Subscribers cannot start new subscriptions
- Existing subscriptions cannot renew automatically
- Error messages indicating treasury account issues
- Loss of trust and potential subscriber churn

---

## Prevention Measures

### Pre-Deployment Checklist

**Before deploying the Tally Protocol:**

1. **Create Platform Treasury ATA**
   ```bash
   # Using SPL Token CLI
   spl-token create-account <USDC_MINT_ADDRESS> --owner <PLATFORM_AUTHORITY>

   # Verify ATA creation
   spl-token account-info <PLATFORM_TREASURY_ATA>
   ```

2. **Verify ATA Derivation**
   ```bash
   # Ensure the ATA address matches canonical derivation
   spl-token gc --owner <PLATFORM_AUTHORITY> | grep <USDC_MINT_ADDRESS>
   ```

3. **Document ATA Address**
   - Record the platform treasury ATA address
   - Store in secure configuration management
   - Include in runbook and operational documentation

4. **Set Account Permissions**
   - Ensure platform authority keypair is secured (cold storage/multisig)
   - Implement strict access controls on platform authority
   - Never delegate close authority to automated systems

### Post-Deployment Safeguards

**Operational Controls:**

1. **Monitoring and Alerting**
   - Monitor platform treasury ATA balance and status every 5 minutes
   - Alert immediately if ATA shows any unexpected changes
   - Track all transactions involving the treasury ATA
   - Set up dead man's switch alerts if monitoring fails

2. **Access Control Policies**
   - Platform authority keypair must be multisig (e.g., Squads Protocol)
   - Require 3-of-5 approval for ANY operation using platform authority
   - Maintain separation of duties between key holders
   - Regular key holder audits and attestations

3. **Change Management**
   - NO automated processes should have platform authority access
   - Document and review all planned operations involving platform authority
   - Implement mandatory peer review for platform authority operations
   - Maintain detailed audit logs of all platform authority usage

4. **Regular Verification**
   - Daily verification that treasury ATA exists and is valid
   - Weekly manual inspection of treasury ATA configuration
   - Monthly comprehensive security review of treasury operations
   - Quarterly disaster recovery drills

---

## Monitoring and Alerting

### Critical Metrics

**Real-Time Monitoring (check every 5 minutes):**

```bash
# Check if treasury ATA exists and is valid
spl-token account-info <PLATFORM_TREASURY_ATA> || alert "CRITICAL: Platform treasury ATA not found"

# Verify account owner and mint
# Expected: owner = <PLATFORM_AUTHORITY>, mint = <USDC_MINT>
spl-token account-info <PLATFORM_TREASURY_ATA> --output json | jq '.owner, .mint'
```

**Alert Thresholds:**

| Metric | Threshold | Severity | Response Time |
|--------|-----------|----------|---------------|
| ATA Not Found | Immediate | CRITICAL | < 5 minutes |
| Unexpected Owner Change | Immediate | CRITICAL | < 5 minutes |
| Unexpected Mint Change | Immediate | CRITICAL | < 5 minutes |
| Close Authority Set | Immediate | CRITICAL | < 15 minutes |
| Monitoring Failure | > 10 minutes | HIGH | < 15 minutes |

### Monitoring Script Example

```bash
#!/bin/bash
# platform_treasury_monitor.sh
# Run this script every 5 minutes via cron

PLATFORM_TREASURY_ATA="<YOUR_PLATFORM_TREASURY_ATA>"
EXPECTED_OWNER="<PLATFORM_AUTHORITY>"
EXPECTED_MINT="<USDC_MINT>"
ALERT_WEBHOOK="<YOUR_ALERTING_WEBHOOK>"

# Check if ATA exists
if ! spl-token account-info "$PLATFORM_TREASURY_ATA" &>/dev/null; then
    curl -X POST "$ALERT_WEBHOOK" \
         -H 'Content-Type: application/json' \
         -d "{\"severity\":\"CRITICAL\",\"message\":\"Platform treasury ATA does not exist: $PLATFORM_TREASURY_ATA\"}"
    exit 1
fi

# Verify owner and mint
ACCOUNT_INFO=$(spl-token account-info "$PLATFORM_TREASURY_ATA" --output json)
CURRENT_OWNER=$(echo "$ACCOUNT_INFO" | jq -r '.owner')
CURRENT_MINT=$(echo "$ACCOUNT_INFO" | jq -r '.mint')

if [ "$CURRENT_OWNER" != "$EXPECTED_OWNER" ]; then
    curl -X POST "$ALERT_WEBHOOK" \
         -H 'Content-Type: application/json' \
         -d "{\"severity\":\"CRITICAL\",\"message\":\"Platform treasury ATA owner mismatch. Expected: $EXPECTED_OWNER, Got: $CURRENT_OWNER\"}"
    exit 1
fi

if [ "$CURRENT_MINT" != "$EXPECTED_MINT" ]; then
    curl -X POST "$ALERT_WEBHOOK" \
         -H 'Content-Type: application/json' \
         -d "{\"severity\":\"CRITICAL\",\"message\":\"Platform treasury ATA mint mismatch. Expected: $EXPECTED_MINT, Got: $CURRENT_MINT\"}"
    exit 1
fi

# Success - log status
echo "$(date): Platform treasury ATA validation passed"
```

**Cron Configuration:**
```cron
# Monitor platform treasury ATA every 5 minutes
*/5 * * * * /opt/tally-protocol/platform_treasury_monitor.sh >> /var/log/tally/treasury-monitor.log 2>&1
```

---

## Recovery Procedures

### Scenario: Platform Treasury ATA Closed or Invalid

**Detection:**
- Monitoring alerts indicate ATA not found or invalid
- Subscription operations failing with `InvalidPlatformTreasuryAccount`
- User reports of failed subscription starts/renewals

**Immediate Response (< 5 minutes):**

1. **Acknowledge Incident**
   - Acknowledge monitoring alerts
   - Notify incident response team
   - Start incident timeline documentation

2. **Verify Problem Scope**
   ```bash
   # Confirm ATA status
   spl-token account-info <PLATFORM_TREASURY_ATA>

   # Check recent transactions for close operation
   solana transaction-history <PLATFORM_AUTHORITY> | grep -i "close"
   ```

3. **Activate Emergency Procedures**
   - Pause non-critical operations if possible
   - Notify stakeholders of incident
   - Prepare for ATA recreation

**Recovery Steps (< 30 minutes):**

1. **Recreate Platform Treasury ATA**
   ```bash
   # Recreate the ATA using platform authority
   spl-token create-account <USDC_MINT> --owner <PLATFORM_AUTHORITY>

   # Verify new ATA matches expected address
   # NOTE: ATA derivation is deterministic - address should be identical
   spl-token accounts <PLATFORM_AUTHORITY> | grep <USDC_MINT>
   ```

2. **Verify ATA Configuration**
   ```bash
   # Confirm all properties match requirements
   spl-token account-info <PLATFORM_TREASURY_ATA> --output json | jq '{owner, mint, amount}'
   ```

3. **Test Recovery**
   ```bash
   # Attempt a test subscription operation
   # Use testnet or small mainnet transaction if possible
   # Verify platform fee transfer succeeds
   ```

4. **Resume Operations**
   - Verify monitoring shows healthy status
   - Confirm subscription operations working
   - Notify stakeholders of recovery completion

**Post-Incident Actions (< 24 hours):**

1. **Root Cause Analysis**
   - Determine how ATA was closed
   - Identify gaps in access controls
   - Review transaction history for unauthorized access
   - Check for compromised keys or processes

2. **Implement Preventive Measures**
   - Strengthen access controls based on findings
   - Update monitoring to detect similar issues faster
   - Improve change management processes
   - Conduct security review of platform authority usage

3. **Documentation and Communication**
   - Document incident timeline and resolution
   - Share lessons learned with team
   - Update runbooks and procedures
   - Conduct post-mortem meeting

---

## Automated Recovery Script

**WARNING:** Only use automated recovery in pre-approved scenarios. Manual verification is recommended.

```bash
#!/bin/bash
# platform_treasury_recovery.sh
# Emergency recovery script for platform treasury ATA

PLATFORM_AUTHORITY_KEYPAIR="/secure/path/to/platform_authority.json"
USDC_MINT="<USDC_MINT_ADDRESS>"
EXPECTED_ATA="<EXPECTED_PLATFORM_TREASURY_ATA>"
ALERT_WEBHOOK="<YOUR_ALERTING_WEBHOOK>"

# Function to send alerts
send_alert() {
    local severity=$1
    local message=$2
    curl -X POST "$ALERT_WEBHOOK" \
         -H 'Content-Type: application/json' \
         -d "{\"severity\":\"$severity\",\"message\":\"$message\"}"
}

# Check if ATA exists
if spl-token account-info "$EXPECTED_ATA" &>/dev/null; then
    echo "Platform treasury ATA exists. No recovery needed."
    exit 0
fi

# ATA does not exist - begin recovery
send_alert "CRITICAL" "Platform treasury ATA missing. Starting automated recovery."

# Recreate ATA
echo "Recreating platform treasury ATA..."
RECREATE_OUTPUT=$(spl-token create-account "$USDC_MINT" \
                  --owner "$PLATFORM_AUTHORITY_KEYPAIR" \
                  --fee-payer "$PLATFORM_AUTHORITY_KEYPAIR" 2>&1)

if [ $? -ne 0 ]; then
    send_alert "CRITICAL" "Failed to recreate platform treasury ATA: $RECREATE_OUTPUT"
    exit 1
fi

# Verify ATA address matches expected
ACTUAL_ATA=$(spl-token accounts "$PLATFORM_AUTHORITY_KEYPAIR" --output json | \
             jq -r ".accounts[] | select(.mint==\"$USDC_MINT\") | .address")

if [ "$ACTUAL_ATA" != "$EXPECTED_ATA" ]; then
    send_alert "CRITICAL" "Recreated ATA address mismatch. Expected: $EXPECTED_ATA, Got: $ACTUAL_ATA"
    exit 1
fi

# Verify ATA configuration
ACCOUNT_INFO=$(spl-token account-info "$EXPECTED_ATA" --output json)
OWNER=$(echo "$ACCOUNT_INFO" | jq -r '.owner')
MINT=$(echo "$ACCOUNT_INFO" | jq -r '.mint')

if [ "$MINT" != "$USDC_MINT" ]; then
    send_alert "CRITICAL" "Recreated ATA has wrong mint. Expected: $USDC_MINT, Got: $MINT"
    exit 1
fi

# Success
send_alert "WARNING" "Platform treasury ATA successfully recreated. Manual verification recommended."
echo "Recovery completed successfully. ATA address: $EXPECTED_ATA"
exit 0
```

---

## Testing and Validation

### Pre-Deployment Testing

**Test Checklist:**

1. **ATA Creation Verification**
   - [ ] Create platform treasury ATA on devnet/testnet
   - [ ] Verify ATA address matches expected derivation
   - [ ] Confirm ATA owner and mint are correct
   - [ ] Document ATA address in configuration

2. **Initialization Testing**
   - [ ] Run `init_config` with valid platform treasury ATA
   - [ ] Verify initialization succeeds and validates ATA
   - [ ] Confirm audit logs show correct ATA address
   - [ ] Test initialization fails with invalid ATA

3. **Subscription Operation Testing**
   - [ ] Create test subscription with valid treasury ATA
   - [ ] Verify platform fees transfer to treasury ATA
   - [ ] Confirm renewal operations succeed
   - [ ] Test operations fail when ATA is invalid

4. **Monitoring Testing**
   - [ ] Deploy monitoring script and verify alerts work
   - [ ] Test alert triggers for missing ATA
   - [ ] Verify alert triggers for invalid configuration
   - [ ] Confirm monitoring recovery procedures

### Disaster Recovery Testing

**Quarterly DR Drill:**

1. **Simulated ATA Closure** (Testnet Only)
   - Close platform treasury ATA in testnet environment
   - Verify monitoring detects closure immediately
   - Confirm alerts fire correctly
   - Execute recovery procedures
   - Measure time to recovery

2. **Recovery Verification**
   - Recreate ATA following recovery procedures
   - Verify ATA address matches expected
   - Test subscription operations post-recovery
   - Document lessons learned and timing

3. **Process Improvement**
   - Review recovery time against SLA targets
   - Identify bottlenecks in recovery process
   - Update procedures based on findings
   - Train additional team members on recovery

---

## Security Considerations

### Access Control Best Practices

**Platform Authority Management:**

1. **Multisig Requirements**
   - Use Squads Protocol or similar multisig solution
   - Minimum 3-of-5 signature threshold
   - Geographically distributed key holders
   - Regular key holder rotation and audits

2. **Key Storage**
   - Platform authority keys in cold storage (hardware wallets)
   - No platform authority keys on internet-connected systems
   - Encrypted backups in multiple secure locations
   - Regular backup verification and testing

3. **Operational Security**
   - All platform authority operations require dual authorization
   - Mandatory waiting periods for high-risk operations
   - Comprehensive audit logging of all key usage
   - Regular security training for key holders

### Attack Vectors and Mitigations

**Potential Attack Vectors:**

1. **Malicious ATA Closure**
   - **Threat:** Attacker gains platform authority access and closes ATA
   - **Mitigation:** Multisig requirements, cold storage, monitoring
   - **Detection:** Real-time monitoring alerts on ATA changes
   - **Response:** Immediate key rotation, ATA recreation, incident response

2. **Social Engineering**
   - **Threat:** Attacker tricks key holder into authorizing ATA closure
   - **Threat:** Security awareness training, mandatory peer review
   - **Detection:** Anomaly detection in authorization patterns
   - **Response:** Incident investigation, additional training, process improvements

3. **Compromised Infrastructure**
   - **Threat:** Attacker compromises monitoring or operational systems
   - **Mitigation:** Defense in depth, network segmentation, regular audits
   - **Detection:** SIEM alerts, infrastructure monitoring, regular penetration testing
   - **Response:** Isolate compromised systems, forensic analysis, remediation

---

## Appendix

### Glossary

**ATA (Associated Token Account):** Deterministic token account address derived from owner and mint
**Platform Authority:** Primary administrative keypair controlling protocol configuration
**Platform Treasury:** Token account receiving platform fees from subscriptions
**DOS (Denial of Service):** Attack or failure causing service unavailability
**SPL Token:** Solana Program Library token standard

### Related Documentation

- [Solana SPL Token Documentation](https://spl.solana.com/token)
- [Associated Token Account Program](https://spl.solana.com/associated-token-account)
- [Squads Protocol Multisig](https://squads.so/)
- [Security Audit Report](../SECURITY_AUDIT_REPORT.md)

### Contact Information

**Emergency Contacts:**

| Role | Contact | Availability |
|------|---------|--------------|
| Platform Operations Lead | [Contact Info] | 24/7 |
| Security Team | [Contact Info] | 24/7 |
| Infrastructure Team | [Contact Info] | 24/7 |
| Executive Escalation | [Contact Info] | Business Hours |

### Revision History

| Date | Version | Author | Changes |
|------|---------|--------|---------|
| 2025-10-05 | 1.0 | Security Audit Team | Initial operational procedures for L-5 audit finding |

---

**Document Classification:** INTERNAL - Operations Critical
**Review Frequency:** Quarterly or after any security incident
**Next Review Date:** 2025-01-05
