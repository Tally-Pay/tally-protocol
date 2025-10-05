# Operational Procedures for Spam Prevention and Incident Response

## Overview

This document provides operational procedures, RPC configuration, monitoring setup, and incident response workflows for managing spam attacks on Tally Protocol. It complements the strategic guidance in [RATE_LIMITING_STRATEGY.md](./RATE_LIMITING_STRATEGY.md) and implementation details in [SPAM_DETECTION.md](./SPAM_DETECTION.md).

---

## Table of Contents

1. [RPC Configuration](#rpc-configuration)
2. [Monitoring Dashboard Setup](#monitoring-dashboard-setup)
3. [Incident Response Procedures](#incident-response-procedures)
4. [Escalation Paths](#escalation-paths)
5. [Post-Incident Review](#post-incident-review)

---

## RPC Configuration

### Nginx Rate Limiting Configuration

**Production-Ready Nginx Configuration for Solana RPC:**

```nginx
# /etc/nginx/nginx.conf

user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 4096;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for" '
                    'account=$http_x_account_pubkey';

    access_log /var/log/nginx/access.log main;

    sendfile on;
    tcp_nopush on;
    keepalive_timeout 65;
    gzip on;

    # Rate limit zones
    limit_req_zone $binary_remote_addr zone=rpc_ip:10m rate=100r/m;
    limit_req_zone $http_x_account_pubkey zone=rpc_account:10m rate=50r/m;
    limit_req_zone $binary_remote_addr zone=plan_creation:10m rate=10r/h;
    limit_req_zone $binary_remote_addr zone=subscription_ops:10m rate=20r/h;

    # Connection limits
    limit_conn_zone $binary_remote_addr zone=addr:10m;

    upstream solana_rpc {
        server 127.0.0.1:8899 max_fails=3 fail_timeout=30s;
        # Add more backend servers for load balancing
        # server 127.0.0.1:8900 backup;
        keepalive 32;
    }

    server {
        listen 80;
        server_name rpc.tallypay.click;

        # Redirect to HTTPS
        return 301 https://$host$request_uri;
    }

    server {
        listen 443 ssl http2;
        server_name rpc.tallypay.click;

        ssl_certificate /etc/letsencrypt/live/rpc.tallypay.click/fullchain.pem;
        ssl_certificate_key /etc/letsencrypt/live/rpc.tallypay.click/privkey.pem;

        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers HIGH:!aNULL:!MD5;
        ssl_prefer_server_ciphers on;

        # Connection limits
        limit_conn addr 10;

        # Global rate limiting
        limit_req zone=rpc_ip burst=20 nodelay;
        limit_req zone=rpc_account burst=10 nodelay;

        location / {
            # Extract account from request body for rate limiting
            # This requires custom Lua script or request parsing

            proxy_pass http://solana_rpc;
            proxy_http_version 1.1;
            proxy_set_header Connection "";
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            # Timeouts
            proxy_connect_timeout 60s;
            proxy_send_timeout 60s;
            proxy_read_timeout 60s;

            # Buffering
            proxy_buffering off;
        }

        # Stricter limits for plan creation (parse method from JSON-RPC)
        location ~ /create_plan {
            limit_req zone=plan_creation burst=2 nodelay;
            proxy_pass http://solana_rpc;
            proxy_http_version 1.1;
            proxy_set_header Connection "";
        }

        # Stricter limits for subscription operations
        location ~ /(start_subscription|cancel_subscription) {
            limit_req zone=subscription_ops burst=5 nodelay;
            proxy_pass http://solana_rpc;
            proxy_http_version 1.1;
            proxy_set_header Connection "";
        }

        # Health check endpoint
        location /health {
            access_log off;
            return 200 "healthy\n";
            add_header Content-Type text/plain;
        }
    }
}
```

**Key Configuration Elements:**

1. **IP-Based Rate Limiting**: 100 requests/minute per IP
2. **Account-Based Limiting**: 50 transactions/minute per account (requires custom header)
3. **Operation-Specific Limits**: 10 plan creations/hour, 20 subscription ops/hour
4. **Connection Limits**: Max 10 concurrent connections per IP
5. **SSL/TLS**: HTTPS with modern cipher suites

**Deployment Steps:**

```bash
# 1. Install Nginx
sudo apt update
sudo apt install nginx

# 2. Copy configuration
sudo cp nginx.conf /etc/nginx/nginx.conf

# 3. Test configuration
sudo nginx -t

# 4. Reload Nginx
sudo systemctl reload nginx

# 5. Monitor logs
sudo tail -f /var/log/nginx/access.log
sudo tail -f /var/log/nginx/error.log
```

### HAProxy Rate Limiting Configuration

**Alternative: HAProxy for Advanced Load Balancing:**

```haproxy
# /etc/haproxy/haproxy.cfg

global
    log /dev/log local0
    log /dev/log local1 notice
    maxconn 4096
    user haproxy
    group haproxy
    daemon

defaults
    log global
    mode http
    option httplog
    option dontlognull
    timeout connect 5000ms
    timeout client 60000ms
    timeout server 60000ms

frontend rpc_frontend
    bind *:443 ssl crt /etc/ssl/certs/tallypay.pem
    mode http

    # Rate limiting using stick tables
    stick-table type ip size 100k expire 1m store http_req_rate(1m)
    acl too_many_requests sc_http_req_rate(0) gt 100
    http-request track-sc0 src
    http-request deny if too_many_requests

    # Route to backend
    default_backend solana_rpc_backend

backend solana_rpc_backend
    mode http
    balance roundrobin
    option httpchk GET /health
    server rpc1 127.0.0.1:8899 check
    server rpc2 127.0.0.1:8900 check backup
```

### Cloudflare Configuration

**For DDoS Protection and Rate Limiting:**

1. **Enable Cloudflare Proxy** for `rpc.tallypay.click`
2. **Configure Rate Limiting Rules**:

```yaml
# Cloudflare Dashboard -> Security -> WAF -> Rate Limiting Rules

Rule 1: General RPC Rate Limit
  - Match: All requests to rpc.tallypay.click
  - Threshold: 100 requests per minute per IP
  - Action: Block for 60 seconds

Rule 2: High-Value Operations
  - Match: Requests containing "create_plan" or "start_subscription"
  - Threshold: 10 requests per hour per IP
  - Action: Challenge (CAPTCHA) then block if exceeded

Rule 3: Known Attackers
  - Match: IP addresses in "Spam IPs" list
  - Action: Block indefinitely
```

3. **Enable Bot Fight Mode** (Business plan)
4. **Configure Firewall Rules** for geo-blocking if needed

---

## Monitoring Dashboard Setup

### Grafana Dashboard Deployment

**Docker Compose Setup:**

```yaml
# docker-compose.monitoring.yml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    container_name: tally-prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    ports:
      - "9091:9090"
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=30d'
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    container_name: tally-grafana
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
      - GF_INSTALL_PLUGINS=grafana-clock-panel,grafana-simple-json-datasource
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning
      - ./grafana/dashboards:/var/lib/grafana/dashboards
    ports:
      - "3000:3000"
    depends_on:
      - prometheus
    restart: unless-stopped

  alertmanager:
    image: prom/alertmanager:latest
    container_name: tally-alertmanager
    volumes:
      - ./alertmanager.yml:/etc/alertmanager/alertmanager.yml
      - alertmanager_data:/alertmanager
    ports:
      - "9093:9093"
    command:
      - '--config.file=/etc/alertmanager/alertmanager.yml'
      - '--storage.path=/alertmanager'
    restart: unless-stopped

volumes:
  prometheus_data:
  grafana_data:
  alertmanager_data:
```

**Prometheus Configuration (prometheus.yml):**

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'tally-protocol'
    environment: 'production'

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']

rule_files:
  - 'alerts.yml'

scrape_configs:
  - job_name: 'tally-indexer'
    static_configs:
      - targets: ['indexer:9090']

  - job_name: 'tally-rpc'
    static_configs:
      - targets: ['rpc-node:9091']

  - job_name: 'nginx'
    static_configs:
      - targets: ['nginx-exporter:9113']
```

**Alert Rules (alerts.yml):**

```yaml
groups:
  - name: spam_detection
    interval: 30s
    rules:
      - alert: HighPlanCreationRate
        expr: rate(tally_plans_created_total[1h]) > 10
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High plan creation rate detected"
          description: "Merchant {{ $labels.merchant }} created {{ $value }} plans in the last hour"

      - alert: SubscriptionChurnSpike
        expr: (tally_subscriptions_canceled_total / tally_subscriptions_started_total) > 0.8
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High subscription churn rate"
          description: "Churn rate is {{ $value | humanizePercentage }}"

      - alert: CancellationSpam
        expr: rate(tally_cancellations_total{account=~".*"}[1h]) > 10
        for: 5m
        labels:
          severity: info
        annotations:
          summary: "Cancellation spam detected"
          description: "Account {{ $labels.account }} canceled {{ $value }} subscriptions in 1h"
```

**Alertmanager Configuration (alertmanager.yml):**

```yaml
global:
  slack_api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK/URL'

route:
  receiver: 'default'
  group_by: ['alertname', 'severity']
  group_wait: 10s
  group_interval: 5m
  repeat_interval: 4h
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
      continue: true
    - match:
        severity: warning
      receiver: 'slack'
    - match:
        severity: info
      receiver: 'email'

receivers:
  - name: 'default'
    slack_configs:
      - channel: '#tally-alerts'
        title: 'Tally Protocol Alert'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_SERVICE_KEY'
        description: '{{ .CommonAnnotations.summary }}'

  - name: 'slack'
    slack_configs:
      - channel: '#tally-warnings'
        title: 'Tally Protocol Warning'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

  - name: 'email'
    email_configs:
      - to: 'ops@tallypay.click'
        from: 'alerts@tallypay.click'
        smarthost: 'smtp.gmail.com:587'
        auth_username: 'alerts@tallypay.click'
        auth_password: 'YOUR_EMAIL_PASSWORD'
        headers:
          Subject: 'Tally Protocol Info Alert'
```

**Grafana Dashboard JSON:**

Save as `grafana/dashboards/spam-detection.json`:

```json
{
  "dashboard": {
    "title": "Tally Protocol - Spam Detection",
    "uid": "tally-spam",
    "version": 1,
    "timezone": "browser",
    "panels": [
      {
        "id": 1,
        "title": "Plans Created (1h rate)",
        "type": "graph",
        "gridPos": {"h": 8, "w": 12, "x": 0, "y": 0},
        "targets": [
          {
            "expr": "rate(tally_plans_created_total[1h])",
            "legendFormat": "Plans/hour"
          }
        ],
        "alert": {
          "conditions": [
            {
              "evaluator": {"params": [10], "type": "gt"},
              "operator": {"type": "and"},
              "query": {"params": ["A", "5m", "now"]},
              "type": "query"
            }
          ],
          "name": "High Plan Creation Rate"
        }
      },
      {
        "id": 2,
        "title": "Subscription Churn Rate",
        "type": "stat",
        "gridPos": {"h": 8, "w": 12, "x": 12, "y": 0},
        "targets": [
          {
            "expr": "tally_subscriptions_canceled_total / tally_subscriptions_started_total",
            "legendFormat": "Churn Rate"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "unit": "percentunit",
            "thresholds": {
              "mode": "absolute",
              "steps": [
                {"value": 0, "color": "green"},
                {"value": 0.5, "color": "yellow"},
                {"value": 0.8, "color": "red"}
              ]
            }
          }
        }
      },
      {
        "id": 3,
        "title": "Top Merchants by Activity",
        "type": "table",
        "gridPos": {"h": 8, "w": 24, "x": 0, "y": 8},
        "targets": [
          {
            "expr": "topk(10, sum by (merchant) (rate(tally_operations_total[1h])))",
            "format": "table"
          }
        ]
      },
      {
        "id": 4,
        "title": "System-Wide Operation Rate",
        "type": "graph",
        "gridPos": {"h": 8, "w": 24, "x": 0, "y": 16},
        "targets": [
          {
            "expr": "sum(rate(tally_operations_total[5m])) by (operation_type)",
            "legendFormat": "{{ operation_type }}"
          }
        ]
      }
    ],
    "refresh": "30s",
    "time": {"from": "now-6h", "to": "now"}
  }
}
```

**Deployment:**

```bash
# 1. Create directory structure
mkdir -p grafana/{provisioning,dashboards}
mkdir -p prometheus

# 2. Copy configuration files
# (Copy prometheus.yml, alerts.yml, alertmanager.yml to respective dirs)

# 3. Start monitoring stack
docker-compose -f docker-compose.monitoring.yml up -d

# 4. Access Grafana
open http://localhost:3000
# Username: admin
# Password: ${GRAFANA_PASSWORD}

# 5. Import dashboard
# Navigate to Dashboards -> Import -> Upload JSON
```

---

## Incident Response Procedures

### Incident Classification

**Severity Levels:**

| Level | Description | Response Time | Examples |
|-------|-------------|---------------|----------|
| **P0 - Critical** | System-wide impact, service degradation | Immediate (<5 min) | DDoS attack, >1000 spam operations/min |
| **P1 - High** | Significant spam, multiple merchants affected | <30 minutes | Coordinated Sybil attack, >100 plans/h |
| **P2 - Medium** | Single merchant spam, contained impact | <2 hours | Single merchant creating excessive plans |
| **P3 - Low** | Nuisance spam, minimal impact | <24 hours | Cancellation spam from single user |

### Response Workflows

#### P0 - Critical Incident Response

**Trigger Conditions:**
- System-wide operation rate >1000/minute
- Multiple merchants flagged simultaneously
- RPC node resource exhaustion (CPU >90%, memory >85%)

**Response Procedure:**

```
1. ALERT RECEIVED (0-2 minutes)
   - PagerDuty notification sent to on-call engineer
   - Incident channel created in Slack (#incident-YYYYMMDD-NNN)
   - Incident commander assigned

2. INITIAL ASSESSMENT (2-5 minutes)
   - Review Grafana dashboard for attack patterns
   - Check Prometheus alerts for severity and scope
   - Identify attack vector (plan spam, subscription churn, etc.)
   - Determine affected accounts (merchants, subscribers, IPs)

3. IMMEDIATE MITIGATION (5-10 minutes)
   Action: Enable aggressive rate limiting
   Command:
     # Update Nginx rate limits (reduce thresholds by 90%)
     sudo vim /etc/nginx/nginx.conf
     # Change: rate=100r/m -> rate=10r/m
     sudo nginx -s reload

   Action: Block attacking IPs
   Command:
     # Add IPs to Cloudflare block list
     curl -X POST "https://api.cloudflare.com/client/v4/zones/${ZONE_ID}/firewall/access_rules/rules" \
       -H "Authorization: Bearer ${CF_API_TOKEN}" \
       -d '{"mode":"block","configuration":{"target":"ip","value":"ATTACKER_IP"}}'

   Action: Throttle specific accounts
   Query:
     -- Get top 10 spamming accounts
     SELECT account, COUNT(*) as operations
     FROM indexed_operations
     WHERE timestamp > NOW() - INTERVAL '10 minutes'
     GROUP BY account
     ORDER BY operations DESC
     LIMIT 10;

   Command:
     # Add accounts to throttle list via admin API
     curl -X POST http://admin-api/throttle \
       -d '{"account":"SPAM_ACCOUNT","duration_hours":24}'

4. INVESTIGATION (10-30 minutes)
   - Analyze attack pattern (coordinated? Sybil? single attacker?)
   - Review transaction signatures and funding sources
   - Document attack timeline and evidence
   - Estimate financial cost to attacker

5. RECOVERY (30-60 minutes)
   - Monitor operation rates for stabilization
   - Gradually relax rate limits if attack subsides
   - Verify legitimate users are not blocked
   - Update firewall rules and throttle lists

6. COMMUNICATION (Throughout)
   - Update incident channel every 15 minutes
   - Notify stakeholders of impact and ETA
   - Post-mortem scheduled within 24 hours
```

**Escalation Criteria:**
- Attack continues >30 minutes despite mitigation
- RPC node becomes unresponsive
- Legitimate users significantly impacted (>10 complaints)
- Financial loss exceeds $1,000 (rent deposits from spam accounts)

#### P1 - High Severity Response

**Trigger Conditions:**
- Single merchant creating >100 plans/hour
- Coordinated Sybil attack detected (>10 accounts, shared funding)
- Subscription churn rate >80% across >50 subscriptions

**Response Procedure:**

```
1. ALERT (0-5 minutes)
   - Slack notification to #tally-alerts channel
   - On-call engineer acknowledges

2. ASSESSMENT (5-15 minutes)
   - Review Grafana dashboard for affected accounts
   - Query indexer for operation details
   - Identify attack pattern and scope

3. MITIGATION (15-30 minutes)
   - Apply targeted rate limits for affected accounts
   - Block attacking IPs if identified
   - Alert merchant if plan spam detected

4. DOCUMENTATION (30+ minutes)
   - Log incident details in incident tracker
   - Update runbooks if new attack pattern discovered
```

#### P2 - Medium Severity Response

**Trigger Conditions:**
- Single merchant spam (10-100 plans/hour)
- Moderate subscription churn (50-80% over 10+ subscriptions)

**Response Procedure:**

```
1. REVIEW (0-30 minutes)
   - Investigate alert details
   - Confirm spam pattern vs. legitimate usage

2. THROTTLE (30-60 minutes)
   - Apply soft rate limits (reduce by 50%)
   - Contact merchant/subscriber if pattern unclear

3. MONITOR (1-2 hours)
   - Track if behavior continues
   - Escalate to P1 if pattern persists or worsens
```

#### P3 - Low Severity Response

**Trigger Conditions:**
- Cancellation spam (>10 cancellations/hour from single user)
- Minor anomalies flagged by automated systems

**Response Procedure:**

```
1. LOG (0-24 hours)
   - Document in monitoring logs
   - Review during next operational review

2. PASSIVE MONITORING
   - Track for pattern escalation
   - No immediate action required
```

### Incident Response Checklist

**Before Incident:**
- [ ] On-call rotation schedule published
- [ ] PagerDuty integration configured
- [ ] Slack incident channels template ready
- [ ] Admin API access credentials secured
- [ ] Cloudflare API tokens documented
- [ ] RPC node access (SSH, admin console)
- [ ] Indexer database credentials available
- [ ] Runbooks reviewed and updated

**During Incident:**
- [ ] Incident commander assigned
- [ ] Incident channel created and stakeholders invited
- [ ] Initial assessment completed (attack vector, scope)
- [ ] Immediate mitigation actions executed
- [ ] Status updates posted every 15 minutes (P0) or 30 minutes (P1)
- [ ] Evidence collected (logs, screenshots, metrics)
- [ ] Escalation decision made if applicable
- [ ] Recovery actions executed
- [ ] Service stability confirmed

**After Incident:**
- [ ] Post-mortem scheduled (within 24h for P0/P1)
- [ ] Incident timeline documented
- [ ] Root cause identified
- [ ] Action items created with owners
- [ ] Runbooks updated with lessons learned
- [ ] Monitoring thresholds adjusted if needed
- [ ] Stakeholders notified of resolution

---

## Escalation Paths

### On-Call Rotation

**Roles:**

1. **L1 - Operations Engineer** (First Responder)
   - Monitors alerts and responds to P2-P3 incidents
   - Executes standard mitigation procedures
   - Escalates to L2 if unable to resolve within SLA

2. **L2 - Senior Engineer** (Incident Commander)
   - Responds to P0-P1 incidents
   - Coordinates cross-functional response
   - Makes escalation decisions
   - Leads post-mortem reviews

3. **L3 - Engineering Lead / CTO**
   - Escalation point for prolonged P0 incidents
   - Approves emergency procedure deviations
   - External communication for major incidents

**Escalation Decision Matrix:**

| Condition | Action | Time Limit |
|-----------|--------|------------|
| P3 incident, standard mitigation fails | Escalate to L2 | 24 hours |
| P2 incident, standard mitigation fails | Escalate to L2 | 2 hours |
| P1 incident, standard mitigation fails | Escalate to L2 | 30 minutes |
| P0 incident (automatic) | Alert L2 immediately | Immediate |
| P0 incident unresolved after mitigation | Escalate to L3 | 1 hour |
| Any incident causing service outage | Escalate to L3 | Immediate |

### Contact Information

**Template for ops team:**

```yaml
# contacts.yml
on_call:
  l1_primary:
    name: "Jane Doe"
    phone: "+1-555-0101"
    slack: "@jane"
    email: "jane@tallypay.click"

  l1_backup:
    name: "John Smith"
    phone: "+1-555-0102"
    slack: "@john"
    email: "john@tallypay.click"

  l2_primary:
    name: "Alice Engineer"
    phone: "+1-555-0201"
    slack: "@alice"
    email: "alice@tallypay.click"
    pagerduty: "alice@tallypay.pagerduty.com"

  l3_escalation:
    name: "Bob CTO"
    phone: "+1-555-0301"
    slack: "@bob"
    email: "bob@tallypay.click"

external:
  rpc_provider:
    name: "Helius Support"
    email: "support@helius.dev"
    sla: "4 hours"

  cloudflare:
    name: "Cloudflare Enterprise Support"
    phone: "+1-888-993-5273"
    portal: "https://dash.cloudflare.com/support"
```

---

## Post-Incident Review

### Post-Mortem Template

**Incident Report: [INCIDENT-YYYYMMDD-NNN]**

**Date:** YYYY-MM-DD
**Duration:** HH:MM start - HH:MM end (Duration: XX hours)
**Severity:** P0 / P1 / P2 / P3
**Incident Commander:** Name

---

**1. Executive Summary**

One-paragraph summary of what happened, impact, and resolution.

**2. Timeline**

| Time (UTC) | Event | Action Taken |
|------------|-------|--------------|
| 14:32 | Alert triggered: High plan creation rate | Acknowledged by on-call |
| 14:35 | Investigation started | Reviewed Grafana dashboard |
| 14:40 | Attack confirmed: 500 plans/min | Applied rate limits |
| 14:45 | Attacker IP identified | Added to Cloudflare block list |
| 15:00 | Operations stabilized | Monitoring continued |
| 15:30 | All-clear declared | Rate limits relaxed |

**3. Root Cause**

Detailed explanation of why the incident occurred.

**4. Impact Assessment**

- **Users Affected:** X merchants, Y subscribers
- **Operations Blocked:** Z legitimate transactions delayed
- **Financial Impact:** $X in spam account rent deposits
- **Downtime:** X minutes of degraded performance

**5. What Went Well**

- Detection systems worked as expected
- Response time within SLA
- Mitigation effective

**6. What Didn't Go Well**

- Initial rate limits too permissive
- Manual IP blocking process slow
- Communication delay to stakeholders

**7. Action Items**

| Item | Owner | Priority | Due Date | Status |
|------|-------|----------|----------|--------|
| Reduce default rate limits by 50% | Jane | High | 2025-10-10 | Open |
| Automate IP blocking from alerts | John | Medium | 2025-10-15 | Open |
| Update communication templates | Alice | Low | 2025-10-20 | Open |

**8. Lessons Learned**

- Rate limits should be more aggressive by default
- Automated response systems reduce MTTR significantly
- Pre-staging communication templates saves time

---

### Continuous Improvement Process

**Quarterly Security Review:**

1. **Metrics Review** (Week 1)
   - Incident frequency and severity trends
   - MTTR (Mean Time To Resolve) analysis
   - False positive/negative rates for alerts

2. **Threat Model Update** (Week 2)
   - Review new attack vectors observed
   - Update attack cost analysis
   - Adjust monitoring thresholds

3. **Runbook Refresh** (Week 3)
   - Incorporate post-mortem action items
   - Update contact information
   - Test escalation procedures

4. **Tabletop Exercise** (Week 4)
   - Simulate P0 incident response
   - Train new team members
   - Validate runbooks and tooling

---

## Summary

This document provides production-ready operational procedures for:

1. **RPC Configuration**: Nginx, HAProxy, and Cloudflare setups
2. **Monitoring Setup**: Prometheus, Grafana, and Alertmanager deployment
3. **Incident Response**: P0-P3 workflows with clear SLAs
4. **Escalation Paths**: On-call rotation and contact information
5. **Post-Incident Process**: Post-mortem template and continuous improvement

**Key Takeaways:**

- Rate limiting at RPC layer is the first line of defense
- Automated monitoring and alerting enable fast response
- Clear escalation paths minimize incident duration
- Post-mortems drive continuous improvement
- Regular testing ensures readiness

**Related Documentation:**
- [RATE_LIMITING_STRATEGY.md](./RATE_LIMITING_STRATEGY.md) - Overall strategy
- [SPAM_DETECTION.md](./SPAM_DETECTION.md) - Implementation details

---

**Document Version:** 1.0
**Last Updated:** 2025-10-05
**Security Audit Reference:** M-6 - No Rate Limiting on Operations
