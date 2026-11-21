# Performance Monitoring and Alerts Guide

This document details the performance monitoring, metrics collection, alerting infrastructure, and operational dashboards for Stellar Nebula Nomad (Issue #676).

## Architecture

The monitoring infrastructure consists of:
1. **Contract & RPC Exporter (`monitoring/exporters/contract_exporter.py`)**:
   - Gathers ledger sequences, invocation telemetry, gas consumption, response times, error rates, and user KPIs from Horizon and contract events.
   - Exposes Prometheus-compatible metrics on port `9200` (`/metrics`).
   - Serves structured JSON system health checks on `/health`.

2. **Prometheus (`monitoring/prometheus/`)**:
   - Scrapes exporter endpoints every 15–30 seconds.
   - Evaluates alerts against defined operational SLA rules (`monitoring/prometheus/alert-rules.yml`).

3. **Alertmanager (`monitoring/alertmanager/`)**:
   - Routes critical and warning notifications to operators.

4. **Grafana Dashboards (`monitoring/grafana/dashboards/`)**:
   - `performance-monitoring.json`: Real-time operational dashboard for response times, error rates, gas usage, user activity, and health status.

---

## Core Metrics Tracked

### 1. Response Times
- `nebula_contract_response_time_seconds`: Current invocation latency.
- `nebula_contract_response_time_p95_seconds`: 95th percentile contract execution time.
- `nebula_contract_response_time_p99_seconds`: 99th percentile contract execution time.
- `horizon_request_duration_seconds`: Upstream Horizon RPC latency.

### 2. Error Rates
- `nebula_contract_invocations_total`: Cumulative contract invocation counter.
- `nebula_contract_errors_total`: Cumulative failed execution counter.
- `nebula_contract_error_rate`: Current error rate fraction (target: `< 0.05`).

### 3. Gas Usage
- `nebula_contract_gas_instructions`: Histogram of CPU instructions per invocation.
- `nebula_contract_gas_instructions_avg`: Average gas instructions per call.
- `nebula_contract_gas_batch_savings_percent`: Percentage gas saved via batching.

### 4. User Metrics
- `nebula_active_users{period="daily"}`: Daily Active Users (DAU).
- `nebula_active_users{period="hourly"}`: Hourly active players.
- `nebula_total_unique_users`: Total unique player accounts created.
- `nebula_user_operations_total`: Total user gameplay interactions.
- `nebula_user_retention_ratio`: Active player retention ratio.

### 5. System Health
- `nebula_system_health_status`: Binary health flag (`1` = healthy, `0` = degraded).
- `nebula_subsystem_health{subsystem="..."}`: Component status for `horizon`, `contract`, and `exporter`.

---

## Health Check Endpoint

Query `GET /health` on the exporter (port `9200`):

```json
{
  "status": "healthy",
  "timestamp": 1700000000,
  "subsystems": {
    "exporter": "healthy",
    "horizon": "healthy",
    "contract": "healthy"
  },
  "metrics_summary": {
    "error_rate": 0.01,
    "response_time_seconds": 0.045,
    "gas_avg_instructions": 420000,
    "daily_active_users": 320
  }
}
```

HTTP Status Codes:
- `200 OK`: All subsystems healthy.
- `503 Service Unavailable`: One or more subsystems degraded or unreachable.

---

## Alert Rules and Thresholds

| Alert Name | Condition | Severity | Description |
|---|---|---|---|
| `SystemHealthDegraded` | `nebula_system_health_status == 0` for 1m | Critical | Subsystems reporting failure |
| `HighContractErrorRate` | Error rate `> 5%` for 2m | Critical | Contract failure spike |
| `HighContractResponseTime` | p95 latency `> 2.0s` for 3m | Warning | SLA degradation |
| `HighGasUsage` | Gas `> 1,000,000` instructions for 5m | Warning | Unusually high gas cost |
| `GasSpikeAnomaly` | 50% gas surge over 1h baseline | Warning | Sudden contract load anomaly |
| `LowTransactionThroughput` | `< 1 tx/min` for 10m | Warning | Throughput stalled |
| `SlowHorizonRpc` | Horizon p99 latency `> 3s` | Critical | Upstream RPC latency |
| `UserActivityDropAnomaly` | Hourly active users `< 5` | Warning | Unexpected user dropoff |
