# Disaster Recovery Plan

## Overview

This document outlines the disaster recovery procedures for the Stellar Nebula Nomad smart contract, including backup strategies, recovery procedures, and testing protocols.

## Table of Contents

1. [Backup Strategy](#backup-strategy)
2. [Recovery Procedures](#recovery-procedures)
3. [Testing & Verification](#testing--verification)
4. [Monitoring & Alerts](#monitoring--alerts)
5. [Roles & Responsibilities](#roles--responsibilities)
6. [Emergency Contacts](#emergency-contacts)

---

## Backup Strategy

### Automated Daily Backups

**Schedule**: Daily at 02:00 UTC

**Components Backed Up**:

- Contract state (all storage entries)
- Contract WASM bytecode
- Player profiles and statistics
- Leaderboard snapshots
- Global analytics data
- Configuration metadata

**Backup Locations**:

1. **Primary**: Local filesystem (`./backups/`)
2. **Secondary**: AWS S3 (if configured)
3. **Tertiary**: IPFS (optional, for decentralized storage)

**Retention Policy**:

- Daily backups: 30 days
- Weekly backups: 90 days
- Monthly backups: 1 year

### Backup Script Usage

```bash
# Run manual backup
./scripts/backup.sh

# With custom configuration
CONTRACT_ID=<contract-id> \
STELLAR_NETWORK=mainnet \
RETENTION_DAYS=60 \
./scripts/backup.sh

# With S3 upload
S3_BUCKET=my-backup-bucket \
AWS_PROFILE=production \
./scripts/backup.sh

# With IPFS upload
IPFS_UPLOAD=true \
./scripts/backup.sh
```

### Automated Backup Setup

#### Using Cron (Linux/Mac)

```bash
# Edit crontab
crontab -e

# Add daily backup at 2 AM UTC
0 2 * * * cd /path/to/stellar-nebula-nomad && ./scripts/backup.sh >> /var/log/nebula-backup.log 2>&1
```

#### Using systemd Timer (Linux)

Create `/etc/systemd/system/nebula-backup.service`:

```ini
[Unit]
Description=Stellar Nebula Nomad Backup
After=network.target

[Service]
Type=oneshot
User=stellar
WorkingDirectory=/path/to/stellar-nebula-nomad
Environment="CONTRACT_ID=<your-contract-id>"
Environment="STELLAR_NETWORK=mainnet"
ExecStart=/path/to/stellar-nebula-nomad/scripts/backup.sh
```

Create `/etc/systemd/system/nebula-backup.timer`:

```ini
[Unit]
Description=Daily Stellar Nebula Nomad Backup
Requires=nebula-backup.service

[Timer]
OnCalendar=daily
OnCalendar=02:00
Persistent=true

[Install]
WantedBy=timers.target
```

Enable and start:

```bash
sudo systemctl enable nebula-backup.timer
sudo systemctl start nebula-backup.timer
sudo systemctl status nebula-backup.timer
```

#### Using GitHub Actions

Create `.github/workflows/backup.yml`:

```yaml
name: Daily Backup

on:
  schedule:
    - cron: "0 2 * * *" # Daily at 2 AM UTC
  workflow_dispatch: # Manual trigger

jobs:
  backup:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Stellar CLI
        run: |
          cargo install --locked stellar-cli

      - name: Run Backup
        env:
          CONTRACT_ID: ${{ secrets.CONTRACT_ID }}
          STELLAR_NETWORK: mainnet
          S3_BUCKET: ${{ secrets.S3_BUCKET }}
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
        run: |
          chmod +x scripts/backup.sh
          ./scripts/backup.sh

      - name: Upload Backup Artifact
        uses: actions/upload-artifact@v3
        with:
          name: backup-${{ github.run_number }}
          path: backups/*.tar.gz
          retention-days: 30
```

---

## Recovery Procedures

### Recovery Time Objective (RTO)

**Target**: < 1 hour from incident detection to full recovery

### Recovery Point Objective (RPO)

**Target**: < 24 hours (daily backups)

### Recovery Scenarios

#### Scenario 1: Contract State Corruption

**Symptoms**:

- Contract returns unexpected values
- Storage entries corrupted
- Transactions failing unexpectedly

**Recovery Steps**:

1. **Identify Issue** (5 minutes)

   ```bash
   # Query contract state
   stellar contract invoke \
     --id $CONTRACT_ID \
     --network mainnet \
     -- get_global_stats
   ```

2. **Locate Latest Valid Backup** (5 minutes)

   ```bash
   # List available backups
   ls -lht backups/*.tar.gz | head -n 5

   # Verify backup integrity
   ./scripts/restore.sh \
     --backup backups/nebula_backup_YYYYMMDD_HHMMSS.tar.gz \
     --verify-only
   ```

3. **Perform Dry Run** (10 minutes)

   ```bash
   ./scripts/restore.sh \
     --backup backups/nebula_backup_YYYYMMDD_HHMMSS.tar.gz \
     --contract $CONTRACT_ID \
     --network mainnet \
     --dry-run
   ```

4. **Execute Restore** (30 minutes)

   ```bash
   ./scripts/restore.sh \
     --backup backups/nebula_backup_YYYYMMDD_HHMMSS.tar.gz \
     --contract $CONTRACT_ID \
     --network mainnet
   ```

5. **Verify Recovery** (10 minutes)

   ```bash
   # Test contract functionality
   stellar contract invoke \
     --id $CONTRACT_ID \
     --network mainnet \
     -- get_global_stats

   # Run integration tests
   cargo test --test integration_tests
   ```

#### Scenario 2: Complete Contract Loss

**Symptoms**:

- Contract not found on network
- All contract data inaccessible

**Recovery Steps**:

1. **Deploy New Contract** (15 minutes)

   ```bash
   # Build contract
   cargo build --release --target wasm32-unknown-unknown

   # Deploy
   stellar contract deploy \
     --wasm target/wasm32-unknown-unknown/release/stellar_nebula_nomad.wasm \
     --network mainnet \
     --source <admin-key>
   ```

2. **Restore State** (30 minutes)

   ```bash
   # Use latest backup
   NEW_CONTRACT_ID=<new-contract-id> \
   ./scripts/restore.sh \
     --backup backups/nebula_backup_YYYYMMDD_HHMMSS.tar.gz \
     --contract $NEW_CONTRACT_ID \
     --network mainnet
   ```

3. **Update References** (10 minutes)
   - Update frontend configuration
   - Update API endpoints
   - Notify users of new contract ID

4. **Verify and Test** (5 minutes)

#### Scenario 3: Network Outage

**Symptoms**:

- Unable to connect to Stellar network
- RPC endpoints unresponsive

**Recovery Steps**:

1. **Switch to Backup RPC** (immediate)

   ```bash
   RPC_URL=https://backup-rpc.stellar.org \
   stellar contract invoke --id $CONTRACT_ID ...
   ```

2. **Monitor Network Status**
   - Check https://status.stellar.org
   - Monitor community channels

3. **Resume Operations** (when network recovers)

---

## Testing & Verification

### Monthly Disaster Recovery Drills

**Schedule**: First Monday of each month

**Procedure**:

1. **Select Random Backup**

   ```bash
   BACKUP=$(ls backups/*.tar.gz | shuf -n 1)
   echo "Testing backup: $BACKUP"
   ```

2. **Verify Integrity**

   ```bash
   ./scripts/restore.sh --backup $BACKUP --verify-only
   ```

3. **Perform Test Restore** (on testnet)

   ```bash
   ./scripts/restore.sh \
     --backup $BACKUP \
     --contract $TESTNET_CONTRACT_ID \
     --network testnet
   ```

4. **Validate Restored Data**

   ```bash
   # Run validation tests
   cargo test --test disaster_recovery_tests
   ```

5. **Document Results**
   - Record recovery time
   - Note any issues
   - Update procedures if needed

### Backup Verification Checklist

- [ ] Backup file exists and is not empty
- [ ] Archive can be extracted successfully
- [ ] Checksums match original files
- [ ] Metadata file is present and valid
- [ ] Contract state file is present
- [ ] Snapshots directory contains expected files
- [ ] File sizes are reasonable (not truncated)
- [ ] Backup completed within expected time
- [ ] No errors in backup log

### Recovery Verification Checklist

- [ ] Contract is accessible on network
- [ ] Global stats query returns valid data
- [ ] Leaderboard query works correctly
- [ ] Player profiles are accessible
- [ ] Ship NFTs are present
- [ ] Resource balances are correct
- [ ] All contract functions work as expected
- [ ] Integration tests pass
- [ ] Recovery completed within RTO (< 1 hour)

---

## Monitoring & Alerts

### Backup Monitoring

**Metrics to Track**:

- Backup success/failure rate
- Backup duration
- Backup file size
- Storage space available
- Last successful backup timestamp

**Alert Conditions**:

- ⚠️ Backup failed
- ⚠️ Backup duration > 30 minutes
- ⚠️ No successful backup in 48 hours
- ⚠️ Storage space < 10% free
- ⚠️ Backup file size anomaly (>50% change)

### Monitoring Setup

#### Prometheus Metrics

```yaml
# Add to monitoring/prometheus/prometheus.yml
- job_name: "backup-monitoring"
  static_configs:
    - targets: ["localhost:9090"]
  metrics_path: "/metrics"
```

#### Alert Rules

```yaml
# monitoring/prometheus/alert-rules.yml
groups:
  - name: backup_alerts
    rules:
      - alert: BackupFailed
        expr: backup_success == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Backup failed"
          description: "Last backup attempt failed"

      - alert: NoRecentBackup
        expr: time() - backup_last_success_timestamp > 172800
        labels:
          severity: critical
        annotations:
          summary: "No recent backup"
          description: "No successful backup in 48 hours"

      - alert: BackupDurationHigh
        expr: backup_duration_seconds > 1800
        labels:
          severity: warning
        annotations:
          summary: "Backup taking too long"
          description: "Backup duration exceeded 30 minutes"
```

#### Webhook Notifications

```bash
# Set webhook URL for notifications
export WEBHOOK_URL="https://hooks.slack.com/services/YOUR/WEBHOOK/URL"

# Or use Discord
export WEBHOOK_URL="https://discord.com/api/webhooks/YOUR/WEBHOOK"
```

---

## Roles & Responsibilities

### Disaster Recovery Team

| Role                | Responsibilities              | Contact      |
| ------------------- | ----------------------------- | ------------ |
| **DR Coordinator**  | Overall DR process management | [Name/Email] |
| **Technical Lead**  | Execute recovery procedures   | [Name/Email] |
| **DevOps Engineer** | Infrastructure and automation | [Name/Email] |
| **QA Lead**         | Verify recovered state        | [Name/Email] |
| **Communications**  | Stakeholder updates           | [Name/Email] |

### Escalation Path

1. **Level 1**: On-call engineer (immediate response)
2. **Level 2**: Technical lead (< 15 minutes)
3. **Level 3**: DR coordinator (< 30 minutes)
4. **Level 4**: Executive team (< 1 hour)

---

## Emergency Contacts

### Internal Team

- **On-Call Engineer**: [Phone/Email]
- **Technical Lead**: [Phone/Email]
- **DR Coordinator**: [Phone/Email]

### External Contacts

- **Stellar Support**: support@stellar.org
- **AWS Support**: [Account-specific]
- **Infrastructure Provider**: [Contact info]

### Communication Channels

- **Slack**: #incident-response
- **Discord**: #emergency
- **Email**: dr-team@yourcompany.com
- **Phone**: [Emergency hotline]

---

## Appendix

### A. Backup File Structure

```
nebula_backup_YYYYMMDD_HHMMSS/
├── state/
│   ├── contract_state.json      # Full contract storage
│   ├── contract.wasm            # Contract bytecode
│   └── snapshots/
│       ├── global_stats.json
│       ├── leaderboard.json
│       └── player_profiles.json
├── metadata/
│   └── contract_info.json       # Backup metadata
└── verification/
    └── checksums.txt            # SHA256 checksums
```

### B. Environment Variables

| Variable          | Description               | Required | Default                             |
| ----------------- | ------------------------- | -------- | ----------------------------------- |
| `CONTRACT_ID`     | Contract identifier       | Yes      | -                                   |
| `STELLAR_NETWORK` | Network (testnet/mainnet) | No       | testnet                             |
| `RPC_URL`         | Stellar RPC endpoint      | No       | https://soroban-testnet.stellar.org |
| `BACKUP_DIR`      | Backup directory          | No       | ./backups                           |
| `RETENTION_DAYS`  | Backup retention period   | No       | 30                                  |
| `S3_BUCKET`       | S3 bucket for backups     | No       | -                                   |
| `IPFS_UPLOAD`     | Enable IPFS upload        | No       | false                               |
| `WEBHOOK_URL`     | Notification webhook      | No       | -                                   |

### C. Troubleshooting

#### Backup Issues

**Problem**: Backup script fails with "CONTRACT_ID not set"

```bash
# Solution: Set environment variable
export CONTRACT_ID=<your-contract-id>
./scripts/backup.sh
```

**Problem**: Insufficient disk space

```bash
# Solution: Clean old backups or increase retention
RETENTION_DAYS=7 ./scripts/backup.sh
```

**Problem**: S3 upload fails

```bash
# Solution: Check AWS credentials
aws s3 ls s3://$S3_BUCKET/
```

#### Restore Issues

**Problem**: Checksum verification fails

```bash
# Solution: Try previous backup
./scripts/restore.sh --backup <previous-backup>
```

**Problem**: Contract not accessible after restore

```bash
# Solution: Verify network and contract ID
stellar contract invoke --id $CONTRACT_ID --network mainnet -- get_global_stats
```

### D. Change Log

| Date       | Version | Changes                        | Author      |
| ---------- | ------- | ------------------------------ | ----------- |
| 2026-04-28 | 1.0     | Initial disaster recovery plan | [Your Name] |

---

## Review Schedule

This disaster recovery plan should be reviewed and updated:

- **Quarterly**: Review procedures and update contacts
- **After incidents**: Document lessons learned
- **After major changes**: Update for new features/infrastructure

**Next Review Date**: [Date]

---

**Document Owner**: [Name]  
**Last Updated**: April 28, 2026  
**Version**: 1.0
