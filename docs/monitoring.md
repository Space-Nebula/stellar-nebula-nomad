# Monitoring & Alerting

## Overview

The monitoring stack provides real-time visibility into contract health,
performance, and errors using **Prometheus**, **Grafana**, and **Alertmanager**.

## Quick Start

```bash
chmod +x setup-monitoring.sh
./setup-monitoring.sh
```

Then open **http://localhost:3000** (Grafana) with the credentials from `monitoring/.env`.

## Stack Components

| Component      | Port | Purpose                          |
|----------------|------|----------------------------------|
| Prometheus     | 9090 | Metrics collection & storage     |
| Grafana        | 3000 | Dashboards & visualisation       |
| Alertmanager   | 9093 | Alert routing & notifications    |
| Node Exporter  | 9100 | Host metrics (CPU, RAM, disk)    |

## Alert Rules

| Alert                    | Severity | Condition                          |
|--------------------------|----------|------------------------------------|
| HighContractErrorRate    | Critical | Error rate > 5% for 2min          |
| HighGasUsage             | Warning  | p95 gas > 1M instructions          |
| LowTransactionThroughput | Warning  | < 1 tx/min for 10min              |
| SlowHorizonRpc           | Critical | p99 latency > 3s                  |
| HighMemoryUsage          | Critical | Memory > 90% for 5min             |
| HighDiskUsage            | Warning  | Disk > 85%                        |
| MonitoringTargetDown     | Critical | Any scrape target unreachable      |

## Data Retention

Prometheus retains 90 days of metrics data by default.
Adjust via `--storage.tsdb.retention.time` in `docker-compose.yml`.

## Configuration

Edit `monitoring/.env` to set:
- `GRAFANA_PASSWORD` — Grafana admin password
- `SLACK_WEBHOOK_URL` — Slack webhook for alerts
- `HORIZON_HOST` — Stellar Horizon endpoint