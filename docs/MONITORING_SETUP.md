# Monitoring setup

This document describes the Prometheus-based monitoring stack for
**stellar-nebula-nomad**: metric collection, alerting, backup/recovery status,
and Grafana visualisation.

## Architecture overview

```
Horizon (testnet / Quickstart)
        │  HTTP /ledgers, /contracts, /accounts
        ▼
monitoring/exporters/contract_exporter.py  (:9200/metrics)
        │  contract health/performance/error counters
        │  + backup/recovery status from BACKUP_STATUS_FILE
        ▼
Prometheus (:9090) ── evaluates ──> alert-rules.yml
        │                       └─> backup-monitoring.yml
        ▼
Alertmanager (:9093) ──> Slack (#nebula-critical / #nebula-alerts) + email
        │
        ▼
Grafana (:3000)  <── dashboards provisioned from monitoring/grafana/dashboards/
```

Host-level metrics come from **node-exporter**; logs are shipped by Promtail to
Loki. On-chain counters also live in `src/metrics_exporter.rs`
(`record_tx_success`, `record_tx_failure`, `get_contract_health`); failed-tx
error frequencies are persisted under `MetricsKey::ErrorTypeFrequency`. The
Python exporter is the Prometheus-facing bridge used for alerting when a full
on-chain exporter is not deployed.

## Components

| Component | Source | Purpose |
|-----------|--------|---------|
| Prometheus | `monitoring/prometheus/prometheus.yml` | Scrapes + evaluates rules |
| Contract rules | `monitoring/prometheus/alert-rules.yml` | Contract, Horizon, host, Soroban alerts |
| Backup rules | `monitoring/backup-monitoring.yml` | Backup/recovery alerts (BackupFailed, NoRecentBackup, …) |
| Exporter | `monitoring/exporters/contract_exporter.py` | Publishes all metrics below |
| Alertmanager | `monitoring/alertmanager/alertmanager.yml` | Routes/severity, Slack + email |
| Grafana | `monitoring/grafana/` | Dashboards + Prometheus/Loki datasources |
| Validator | `monitoring/validate-monitoring.py` | Offline consistency checks |

`BackupFailed` and `NoRecentBackup` are defined **only** in
`backup-monitoring.yml`. They were removed from `alert-rules.yml` so Prometheus
never loads a duplicate alert name across rule files. `prometheus.yml` loads
both files:

```yaml
rule_files:
  - 'alert-rules.yml'
  - '../backup-monitoring.yml'
```

Paths are relative to `prometheus.yml`; `../backup-monitoring.yml` resolves to
`monitoring/backup-monitoring.yml` when Prometheus is run from the repo. See
[Docker note](#docker-compose-note) if running the bundled compose stack.

## Start the stack

```bash
# With the full local environment (Quickstart + hot reload + monitoring)
docker compose -f docker-compose.dev.yml up --build

# Monitoring only (public Horizon testnet by default)
docker compose -f monitoring/docker-compose.yml up -d
# or
./setup-monitoring.sh
```

Run Prometheus from source instead:

```bash
prometheus --config.file=monitoring/prometheus/prometheus.yml \
           --storage.tsdb.path=/tmp/prometheus-data
```

| Service | URL |
|---------|-----|
| Grafana | http://localhost:3000 |
| Prometheus | http://localhost:9090 |
| Alertmanager | http://localhost:9093 |
| Contract exporter | http://localhost:9200/metrics |
| Exporter health | http://localhost:9200/health |

**Grafana credentials:** default user `admin`, password from
`GRAFANA_PASSWORD` (compose default `changeme`; set it via `monitoring/.env`).
Set `CONTRACT_ID` and optionally `HORIZON_URL` in the environment (or
`monitoring/.env`) so the exporter can scrape the live contract. Other exporter
tunables: `EXPORTER_PORT` (default `9200`), `SCRAPE_TIMEOUT` (default `8`),
`CONTRACT_METHOD` (default `invoke`), `CONTRACT_GAS_INSTRUCTIONS` (default
`250000`), and `BACKUP_STATUS_FILE` (see below).

### Docker compose note

The shipped `monitoring/docker-compose.yml` mounts only
`prometheus/alert-rules.yml` into `/etc/prometheus/`. Because `prometheus.yml`
also references `../backup-monitoring.yml`, add that file to the prometheus
service's `volumes` and align the path when running the containerised stack:

```yaml
volumes:
  - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
  - ./prometheus/alert-rules.yml:/etc/prometheus/alert-rules.yml:ro
  - ./backup-monitoring.yml:/etc/prometheus/backup-monitoring.yml:ro
```
then use `- 'backup-monitoring.yml'` as the second `rule_files` entry inside
the container, or keep `- '../backup-monitoring.yml'` and mount the file at
`/etc/backup-monitoring.yml`. Validate with
`promtool check config monitoring/prometheus/prometheus.yml` after any change.

## Publishing backup/recovery status

The exporter exposes `backup_*` / `recovery_*` gauges. The backup workflow
writes their values as JSON to the path in `BACKUP_STATUS_FILE`, which the
exporter reads on every `/metrics` request:

```bash
export BACKUP_STATUS_FILE=/var/lib/nebula/backup-status.json
cat > "$BACKUP_STATUS_FILE" <<'JSON'
{
  "backup_success": 1,
  "backup_last_success_timestamp": 1710000000,
  "backup_duration_seconds": 92.5,
  "backup_size_bytes": 1048576,
  "backup_storage_available_bytes": 900000000,
  "backup_storage_total_bytes": 1000000000,
  "recovery_success": 1,
  "recovery_duration_seconds": 600,
  "last_drill_timestamp": 1710000000,
  "yield_forecast_stale_entries": 0,
  "migration_started_timestamp": 0,
  "migration_status": 0
}
JSON
```

Missing keys degrade to safe defaults (`0`), so a fresh stack with no status
file will intentionally trigger `BackupFailed` / `NoRecentBackup` until the
workflow publishes a result. `migration_status` is numeric: `1` =
`in_progress`, `0` = idle/complete.

## Alert catalog

Rules are evaluated every 30s (contract/host) or 5m (backup/recovery).
Runbooks live in [DISASTER_RECOVERY.md](../DISASTER_RECOVERY.md).

| Alert | Severity | Condition / threshold | For | Source |
|-------|----------|-----------------------|-----|--------|
| ContractUnhealthy | critical | `nebula_contract_health_status == 0` | 2m | alert-rules |
| HighContractErrorRate | critical | error-rate ratio `> 0.05` | 2m | alert-rules |
| HighGasUsage | warning | p95 gas `> 1,000,000` instructions | 5m | alert-rules |
| LowTransactionThroughput | warning | invocation rate `< 0.016/s` (<1/min) | 10m | alert-rules |
| SlowHorizonRpc | critical | p99 Horizon latency `> 3s` | 5m | alert-rules |
| HighMemoryUsage | critical | host memory used `> 90%` | 5m | alert-rules |
| HighDiskUsage | warning | host disk used `> 85%` | 10m | alert-rules |
| MonitoringTargetDown | critical | `up == 0` | 1m | alert-rules |
| SorobanTxSuccessRateLow | warning | success ratio `< 0.95` | 3m | alert-rules |
| SorobanGasUsageSpike | warning | 5m gas rate `> 1.5×` 1h-ago rate | 2m | alert-rules |
| CacheTTLExpirationsHigh | warning | `rate(cache_expirations_total[5m]) > 100` | 5m | alert-rules |
| StaleYieldForecastData | critical | `yield_forecast_stale_entries > 0` | 1m | alert-rules |
| MigrationTimeout | critical | `time()-migration_started_timestamp > 3600` and `migration_status == 1` | 1m | alert-rules |
| ActiveUserCountAnomalous | warning | user-count rate deviates `> 2σ` from 24h baseline | 10m | alert-rules |
| **BackupFailed** | critical | `backup_success == 0` | 5m | backup-monitoring |
| **NoRecentBackup** | critical | `time()-backup_last_success_timestamp > 172800` (48h) | 10m | backup-monitoring |
| BackupDurationHigh | warning | `backup_duration_seconds > 1800` (30m) | 5m | backup-monitoring |
| BackupStorageLow | warning | available/total `< 0.1` | 15m | backup-monitoring |
| BackupSizeAnomaly | warning | size deviates `> 50%` from 7d average | 10m | backup-monitoring |
| BackupSuccess | info | `backup_success == 1` | 1m | backup-monitoring |
| RecoveryFailed | critical | `recovery_success == 0` | 1m | backup-monitoring |
| RecoveryTimeExceededRTO | warning | `recovery_duration_seconds > 3600` | 1m | backup-monitoring |
| MonthlyDrillDue | info | `time()-last_drill_timestamp > 2592000` (30d) | 1h | backup-monitoring |

Runbook links are attached as `runbook` annotations on the backup/recovery and
contract-health alerts and point at the
[disaster recovery plan](https://github.com/Mikey-222/stellar-nebula-nomad/blob/main/DISASTER_RECOVERY.md)
sections. Alertmanager routes `critical` → `#nebula-critical` + on-call email
(1h repeat), `warning` → `#nebula-alerts` (6h repeat), everything else →
`default`.

## Exported metrics catalog

All metrics below are emitted by `monitoring/exporters/contract_exporter.py`
unless noted.

**Contract health / performance / errors**

| Metric | Type | Labels / notes |
|--------|------|----------------|
| `nebula_exporter_up` | gauge | exporter process health |
| `nebula_horizon_up` | gauge | Horizon reachable |
| `nebula_horizon_latest_ledger` | gauge | latest ledger sequence |
| `nebula_horizon_closed_at` | gauge | ledger timestamp present |
| `nebula_contract_scrape_success` | gauge | contract scrape succeeded |
| `nebula_contract_health_status` | gauge | 1 healthy / 0 unhealthy |
| `nebula_contract_last_success_timestamp` | gauge | Unix timestamp |
| `nebula_contract_invocations_total` | counter | `contract`, `method` |
| `nebula_contract_errors_total` | counter | `contract`, `method`, `reason` |
| `nebula_contract_gas_instructions` | histogram | `_bucket{le}`, `_sum`, `_count` |
| `horizon_request_duration_seconds` | histogram | `_bucket{le}`, `_sum`, `_count` |
| `horizon_request_duration_seconds_last` | gauge | last scrape latency |
| `soroban_tx_success_total` | counter | successful transactions |
| `soroban_tx_failure_total` | counter | failed transactions |
| `soroban_gas_used_total` | counter | gas consumed |
| `cache_expirations_total` | counter | expired cache entries |
| `soroban_active_users` | counter | use `rate()` for per-interval users |

**Backup / recovery (from `BACKUP_STATUS_FILE`)**

`backup_success`, `backup_last_success_timestamp`, `backup_duration_seconds`,
`backup_size_bytes`, `backup_storage_available_bytes`,
`backup_storage_total_bytes`, `recovery_success`,
`recovery_duration_seconds`, `last_drill_timestamp`.

**On-chain / app gauges (status file or on-chain exporter)**

`yield_forecast_stale_entries`, `migration_started_timestamp`,
`migration_status` (numeric 1 = in progress).

**Node / Prometheus (not from our exporter)**

`node_memory_MemAvailable_bytes`, `node_memory_MemTotal_bytes`,
`node_filesystem_avail_bytes`, `node_filesystem_size_bytes` (node-exporter) and
`up` (Prometheus-generated per scrape target).

## Grafana dashboards

Provisioning is folder-based — `monitoring/grafana/provisioning/dashboards/dashboards.yml`
loads every JSON file in `monitoring/grafana/dashboards/` into the
**Stellar Nebula Nomad** folder. Dashboards:

| Dashboard | UID | Contents |
|-----------|-----|----------|
| Stellar Nebula Nomad — Overview | `nebula-overview-v1` | Error rate, throughput, p95 gas, Horizon latency |
| **Contract Health** | `nebula-contract-health` | Health status, last success, error rate, invocations, gas percentiles, Soroban outcomes |
| **Backup & Recovery** | `nebula-backup-recovery` | Backup success/age/duration/size, storage utilisation, recovery + drill status |
| Economic | `economic` | Economic metrics |
| Game metrics | `game-metrics` | Game activity |
| Alerts | `alerts` | Alert overview |
| System health | `system-health` | Host-level metrics |

Access Grafana at http://localhost:3000, log in with `admin` /
`GRAFANA_PASSWORD` (default `changeme`), then open **Dashboards → Stellar
Nebula Nomad**. The Prometheus datasource is provisioned in
`monitoring/grafana/provisioning/datasources/prometheus.yml`.

## Verify alerting

1. **Offline config validation** (no dependencies required):

   ```bash
   python3 monitoring/validate-monitoring.py
   python3 -m py_compile monitoring/exporters/contract_exporter.py monitoring/validate-monitoring.py
   python3 -c "import json,glob; [json.load(open(f)) for f in glob.glob('monitoring/grafana/dashboards/*.json')]"
   ```

   The validator checks required alerts and duplicate names across rule files,
   that `rule_files` entries exist, that the exporter source contains every
   required metric, and that dashboards are valid JSON with a title/panels. It
   exits non-zero on failure.

2. **Rule/config validation with promtool** (if installed):

   ```bash
   promtool check rules monitoring/prometheus/alert-rules.yml monitoring/backup-monitoring.yml
   promtool check config monitoring/prometheus/prometheus.yml
   ```

3. **Live smoke test**:

   ```bash
   curl -s http://localhost:9200/metrics | grep -E 'backup_success|nebula_contract_health_status'
   curl -s http://localhost:9090/-/healthy
   # In Prometheus UI: Status > Rules and Alerts — all groups should be "OK".
   ```

4. **Fire a backup alert deliberately**: set `backup_success` to `0` in the
   status JSON (or remove `BACKUP_STATUS_FILE` entirely) and wait for the `for`
   window (5m). `BackupFailed` should appear in Alertmanager; publish a
   successful JSON to resolve it.

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| `BackupFailed` / `NoRecentBackup` firing on a fresh stack | No status file published; write `BACKUP_STATUS_FILE` (see above). |
| Prometheus fails to start with "field rules not found in rule file" / missing file | `rule_files` path wrong for your layout, or backup rule file not mounted — see [Docker note](#docker-compose-note). |
| `duplicate alert` errors / alerts flapping | `BackupFailed`/`NoRecentBackup` reintroduced into `alert-rules.yml`; keep them only in `backup-monitoring.yml`. |
| `HighContractErrorRate` always 0 or absent | `CONTRACT_ID` unset (exporter can't reach the contract) or Horizon unreachable; check `/metrics` and exporter logs. |
| Histogram panels empty | Need at least a few scrapes so `rate(...)` has samples; wait 2–3 minutes. |
| Grafana shows "Datasource not found" | Prometheus datasource not provisioned; check `monitoring/grafana/provisioning/datasources/prometheus.yml`. |
| Slack/email alerts not delivered | `SLACK_WEBHOOK_URL` / SMTP vars missing in `monitoring/alertmanager/alertmanager.yml`; check Alertmanager status page. |
| Node metrics missing | `node-exporter` service not running or not in the `monitoring` network. |
