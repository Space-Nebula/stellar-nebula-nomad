# Stellar Nebula Nomad - Disaster Recovery Plan

This document outlines the disaster recovery (DR) procedures for the Stellar Nebula Nomad contract to ensure high business continuity.

## 1. RTO & RPO Targets

- **Recovery Time Objective (RTO)**: 15 minutes. This is the target time to restore contract operations after a critical failure or catastrophic state corruption.
- **Recovery Point Objective (RPO)**: 24 hours (or up to the last successful backup). This means the maximum acceptable data loss is 24 hours of state updates.

## 2. Backup Procedures

Backups are handled by `scripts/backup.sh`.
- **Automated Backups**: Backups are run daily via a scheduled cron job.
- **Offsite Storage**: Compressed backups (`.tar.gz`) are pushed to an AWS S3 bucket and optionally to IPFS.
- **Manual Backups**: Before any major migration or upgrade, operators must run:
  ```bash
  ./scripts/backup.sh
  ```

## 3. Recovery Procedures (Failover)

If the active contract is corrupted or compromised:

1. **Identify Last Good State**: Locate the most recent healthy backup from the S3 bucket or IPFS.
2. **Download Backup**: Retrieve the `.tar.gz` file and extract it to the `backups/` directory.
3. **Execute Restore**: Use the restore script to apply the state and optionally deploy a fresh contract instance:
   ```bash
   ./scripts/restore.sh --backup nebula_backup_YYYYMMDD_HHMMSS
   ```
4. **Switch Traffic**: Update the `deployment/aliases/nebula-prod.txt` alias to point to the newly restored contract.
5. **Verify State**: Run state verification to confirm the contract is healthy:
   ```bash
   soroban contract invoke --id <CONTRACT_ID> --fn get_global_stats
   ```

## 4. Recovery Testing

To maintain readiness, recovery testing should be performed monthly:
1. Run `./scripts/backup.sh --test-restore` (which automatically restores a fresh backup into a local/temporary network).
2. Ensure the test environment passes all basic read and write assertions without affecting production data.
