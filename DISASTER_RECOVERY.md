# Disaster Recovery

This is the operator runbook for Stellar Nebula Nomad contract-state backups and restores.

_Last verified: 2026-11-19 via `./scripts/test-disaster-recovery.sh` — 7/7 checks passed. Machine-generated results: [docs/DISASTER_RECOVERY_TEST_RESULTS.md](docs/DISASTER_RECOVERY_TEST_RESULTS.md)._

## Backup Strategy

Automated **Contract state** snapshots are taken daily at 02:00 UTC through Horizon:

- Latest ledger from `GET /ledgers`
- Contract record from `GET /contracts/{id}` (fallback `GET /accounts/{id}`)
- **Player profiles** via account effects
- **Leaderboard** / activity via operations
- **Contract WASM** when Stellar CLI is available

Archives are named `nebula_backup_YYYYMMDD_HHMMSS.tar.gz`, checksummed with SHA-256, and kept for 30 days locally. Optional S3 copies follow a 90-day lifecycle (`infrastructure/backup/`). Retention now also enforces a `BACKUP_MIN_KEEP` floor (default **7** archives) so an aging backup set is never pruned below the recovery window. A backup whose Horizon snapshots all fail is rejected instead of being archived as "successful" (`retention_policy.json` records `snapshots_succeeded` / `snapshots_attempted`).

RTO target: **< 1 hour**. RPO target: **< 24 hours** (last successful daily backup).

## Recovery Procedures

1. Pick the newest healthy archive under `backups/` (or S3).
2. Verify checksums: `./scripts/restore.sh --backup <file> --verify-only`
3. Dry-run: `./scripts/restore.sh --backup <file> --dry-run`
4. Materialise the archived payload: `./scripts/restore.sh --backup <file> --target-dir <dir>`
   (set `CONTRACT_ID` for a live restore; `SKIP_RPC_VERIFY=true` for offline operation).
5. Complete the **manual on-chain steps** printed by the restore script, then
   switch aliases under `deployment/aliases/` and run `./scripts/verify.sh`.

> **Important limitation discovered in testing:** Soroban has no general API for
> importing arbitrary ledger entries into a deployed contract. `restore.sh`
> therefore verifies the archive and materialises `state/`, `metadata/` and the
> WASM into a target directory, but the actual on-chain reinitialisation/replay
> is a manual operator step. The script reports
> `STATE_MATERIALISED_MANUAL_ONCHAIN_STEPS_REQUIRED` rather than a misleading
> "SUCCESS". Any tool claiming fully automated contract-state restore should be
> treated as unverified until it is demonstrated against a live testnet contract.

### Scenario 1 — Corrupt contract instance

Redeploy WASM from the archive (`state/contract.wasm`) or rebuild from the commit recorded in `metadata/contract_info.json`, then re-initialize from the Horizon snapshot metadata and reconcile the exported effects/operations.

- **Test result: PASS.** A byte-corrupted archive is rejected by `--verify-only`; the healthy archive verifies and its state payload materialises successfully.
- **RTO measured:** < 1 minute for verification + materialisation of the fixture; on-chain redeploy/replay time must be confirmed in the quarterly live drill.

### Scenario 2 — Lost operator keys

Use `metadata/contract_info.json` (contract id, network, RPC URL, `deployer_identity`) plus Horizon history to reconstruct the contract id, then rotate to a new deployer identity and update the repository/CI secret.

- **Test result: PASS.** The contract id was recovered from backup metadata and a rotated deployer identity record was written.
- **Gap:** backup metadata does **not** contain key material or an escrow. Rotation depends on the operator generating a new key and updating `DEPLOYER_SECRET_KEY` / `ADMIN_SECRET_KEY`. The `infrastructure/modules/secrets` module currently creates empty secret containers only.
- **Training requirement:** maintainers must know the location of the offline key escrow and the rotation runbook; this cannot be reconstructed from the archive.

### Scenario 3 — Horizon / RPC outage

Serve from the last local `.tar.gz`. The snapshot includes ledger JSON so you can resume once Horizon returns. Set `SKIP_RPC_VERIFY=true` so the live contract probe is skipped rather than failing the restore.

- **Test result: PASS.** With `HORIZON_URL` and `RPC_URL` pointed at an unreachable endpoint, restore from the local archive still verified and materialised state.
- **Gap:** there is no offline *serving* layer for end users — this scenario restores operator access to the data, not live application availability.

### Scenario 4 — Complete infrastructure loss

Provision fresh infrastructure from `infrastructure/` (Terraform), restore the analytics database from the latest `scripts/backup/backup-postgres.sh` dump (`restore-postgres.sh --dump <file> --target-db <new-db-url>`), restore contract state per Scenario 1, and re-point DNS/secrets at the new instances.

- **Test result: PARTIAL/PASS for the exercised path.** Synthetic DB dump checksum verification and cold-storage contract archive verification both passed.
- **Not exercised:** real `pg_restore` into a live Postgres instance, Terraform apply, and DNS/secret re-pointing. These require a provisioned environment and remain the quarterly manual drill.
- **Known blockers:** root Terraform defines a VPC but no subnets, no `terraform.tfvars` is checked in, and `infrastructure/backend.tf` uses placeholder state-bucket names. Resolve these before a real Scenario 4 drill.

## Database Backups (issue #517)

`scripts/backup/backup-postgres.sh` runs a `pg_dump` of the analytics database, checksums it, optionally ships it to S3, and prunes anything older than `RETENTION_DAYS` (default 90). `scripts/backup/restore-postgres.sh --dump <file> --verify-only` checks integrity; drop `--verify-only` and add `--target-db <url>` to actually restore.

RTO target for the analytics DB: **4 hours**. RPO target: **24 hours**. (The < 1 hour RTO above applies to contract state, not the analytics database.)

**Gap found:** `backup-postgres.sh` is not scheduled by any workflow and `DATABASE_URL` is absent from `.env.example`, so the documented *daily* pg_dump is not currently automated. Add a scheduled job and configure `DATABASE_URL` before relying on it.

## Testing & Verification

Run the full scenario suite locally (no network or cloud account required):

```bash
./scripts/test-disaster-recovery.sh
```

It creates synthetic fixtures, exercises all four scenarios, checks that archives without checksums are rejected, and verifies the RTO/RPO targets. Results are written to `docs/DISASTER_RECOVERY_TEST_RESULTS.md`.

Additional verification:

- `./scripts/backup.sh --test-restore` takes a snapshot and runs `restore.sh --test-mode`.
- GitHub Actions `backup.yml` verifies archive integrity after each scheduled run.
- A monthly drill performs a dry-run restore of contract state.
- **Quarterly drill (manual):** full Scenario 4 against throwaway infrastructure. Record the result in `docs/DISASTER_RECOVERY_TEST_RESULTS.md`.

## Monitoring & Alerts

`monitoring/prometheus/alert-rules.yml` and `monitoring/backup-monitoring.yml` define **BackupFailed** and **NoRecentBackup**.

Fixes applied after testing:

- `backup-monitoring.yml` is now mounted and listed in `prometheus.yml`'s `rule_files`, so the rules are actually loaded.
- `NoRecentBackup` now fires after **25h** instead of 48h, matching the 24h RPO with a 1h grace period.
- Runbook URLs now point at the real repository and runbook.

**Remaining gap:** no exporter currently emits the `backup_success` / `backup_last_success_timestamp` series the alerts depend on, so the alerts cannot fire until a backup-metrics exporter is added. Treat backup freshness verification as a manual check (`ls -lt backups/`) in the meantime.

Grafana: http://localhost:3000 (see `docs/MONITORING_SETUP.md`).

## Emergency Contacts

| Role | Contact |
|------|---------|
| On-call maintainer | GitHub `@Space-Nebula` maintainers |
| Stellar status | https://status.stellar.org |

### Escalation Path

1. Check backup logs (`backups/backup.log`, `backups/restore.log`) and the Actions run.
2. Page the on-call maintainer if RPO is at risk (no backup in 24 hours).
3. If restore fails, open an incident and freeze further deploys.

## Restoration procedures (quick)

```bash
chmod +x scripts/backup.sh scripts/restore.sh
CONTRACT_ID=<id> STELLAR_NETWORK=testnet ./scripts/backup.sh
./scripts/restore.sh --backup backups/nebula_backup_*.tar.gz --verify-only
./scripts/restore.sh --backup backups/nebula_backup_*.tar.gz --dry-run
CONTRACT_ID=<id> ./scripts/restore.sh \
  --backup backups/nebula_backup_*.tar.gz \
  --target-dir backups/restored/latest
```

Full policy: `infrastructure/backup/README.md` and `docs/BACKUP_RECOVERY_README.md`.
