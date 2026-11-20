#!/usr/bin/env python3
"""Prometheus exporter for Stellar Nebula Nomad contract metrics.

Scrapes Horizon for ledger/contract snapshots and exposes Prometheus text
so Grafana dashboards and alert rules have a live target. Also publishes
contract health / performance / error-rate series and (optionally) the
backup & recovery status written by the backup workflow to BACKUP_STATUS_FILE.
"""

from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HORIZON_URL = os.environ.get("HORIZON_URL", "https://horizon-testnet.stellar.org").rstrip("/")
CONTRACT_ID = os.environ.get("CONTRACT_ID", "")
PORT = int(os.environ.get("EXPORTER_PORT", "9200"))
SCRAPE_TIMEOUT = float(os.environ.get("SCRAPE_TIMEOUT", "8"))

# Optional: path to a JSON file published by scripts/backup.sh after each run.
# When unset, backup_* / recovery_* metrics fall back to their defaults (0),
# which intentionally keeps BackupFailed / NoRecentBackup meaningful instead
# of silently reporting a healthy backup that never happened.
BACKUP_STATUS_FILE = os.environ.get("BACKUP_STATUS_FILE", "").strip()

CONTRACT_LABEL = CONTRACT_ID or "unknown"
METHOD_LABEL = os.environ.get("CONTRACT_METHOD", "invoke")
GAS_INSTRUCTIONS = float(os.environ.get("CONTRACT_GAS_INSTRUCTIONS", "250000"))

# Histogram bucket upper bounds (prometheus-style, cumulative, +Inf implicit).
GAS_BUCKETS = (10000.0, 100000.0, 500000.0, 1000000.0, 5000000.0)
HORIZON_BUCKETS = (0.1, 0.5, 1.0, 3.0, 10.0)

# Scrape-to-scrape state. The exporter is a proxy for on-chain counters; keeping
# monotonically increasing totals here makes rate()/increase() queries in the
# alert rules and dashboards behave correctly.
_STATE: dict = {
    "nebula_contract_invocations_total": 0.0,
    "nebula_contract_errors_total": 0.0,
    "nebula_contract_gas_sum": 0.0,
    "nebula_contract_gas_count": 0.0,
    "nebula_contract_gas_buckets": [0.0] * len(GAS_BUCKETS),
    "horizon_request_duration_sum": 0.0,
    "horizon_request_duration_count": 0.0,
    "horizon_request_duration_buckets": [0.0] * len(HORIZON_BUCKETS),
    "soroban_tx_success_total": 0.0,
    "soroban_tx_failure_total": 0.0,
    "soroban_gas_used_total": 0.0,
    "cache_expirations_total": 0.0,
    "soroban_active_users": 0.0,
}

# Every scalar the backup workflow (or the on-chain exporter) may publish.
_STATUS_DEFAULTS: dict[str, float] = {
    "backup_success": 0.0,
    "backup_last_success_timestamp": 0.0,
    "backup_duration_seconds": 0.0,
    "backup_size_bytes": 0.0,
    "backup_storage_available_bytes": 0.0,
    "backup_storage_total_bytes": 0.0,
    "recovery_success": 0.0,
    "recovery_duration_seconds": 0.0,
    "last_drill_timestamp": 0.0,
    # Extra gauges referenced by alert rules; default to "nothing wrong".
    "yield_forecast_stale_entries": 0.0,
    "migration_started_timestamp": 0.0,
    "migration_status": 0.0,  # 1 = in_progress, 0 = idle/complete
}

_LAST_SUCCESS_TS = 0.0


def horizon_get(path: str) -> tuple[int, dict | list | None]:
    url = f"{HORIZON_URL}{path}"
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=SCRAPE_TIMEOUT) as resp:
            body = resp.read().decode("utf-8")
            return resp.status, json.loads(body) if body else None
    except urllib.error.HTTPError as exc:
        return exc.code, None
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError):
        return 0, None


def load_status_file() -> dict[str, float]:
    """Read BACKUP_STATUS_FILE and return numeric status values.

    Missing, unreadable or malformed files degrade to the safe defaults rather
    than raising - the HTTP handler must always be able to serve /metrics.
    """
    values = dict(_STATUS_DEFAULTS)
    if not BACKUP_STATUS_FILE:
        return values
    try:
        with open(BACKUP_STATUS_FILE, "r", encoding="utf-8") as handle:
            payload = json.load(handle)
    except (OSError, json.JSONDecodeError, ValueError):
        return values
    if not isinstance(payload, dict):
        return values
    for key in _STATUS_DEFAULTS:
        raw = payload.get(key)
        if raw is None:
            continue
        try:
            values[key] = float(raw)
        except (TypeError, ValueError):
            continue
    return values


def _observe(sum_key: str, count_key: str, buckets_key: str, bounds: tuple, value: float) -> None:
    _STATE[sum_key] += value
    _STATE[count_key] += 1
    for index, upper in enumerate(bounds):
        if value <= upper:
            _STATE[buckets_key][index] += 1


def collect() -> dict:
    global _LAST_SUCCESS_TS

    start = time.time()
    status, ledger_page = horizon_get("/ledgers?order=desc&limit=1")
    latency = time.time() - start
    _observe(
        "horizon_request_duration_sum",
        "horizon_request_duration_count",
        "horizon_request_duration_buckets",
        HORIZON_BUCKETS,
        latency,
    )

    values: dict = dict(_STATE)
    values.update(load_status_file())

    values["horizon_request_duration_seconds"] = latency
    values["nebula_horizon_up"] = 0
    values["nebula_horizon_latest_ledger"] = 0
    values["nebula_horizon_closed_at"] = 0
    values["nebula_contract_scrape_success"] = 0

    if status == 200 and isinstance(ledger_page, dict):
        records = (
            ledger_page.get("_embedded", {}).get("records")
            if isinstance(ledger_page.get("_embedded"), dict)
            else None
        )
        if records:
            latest = records[0]
            values["nebula_horizon_up"] = 1
            values["nebula_horizon_latest_ledger"] = float(latest.get("sequence") or 0)
            closed = latest.get("closed_at") or latest.get("closedAt")
            if isinstance(closed, str):
                values["nebula_horizon_closed_at"] = 1
            values["nebula_contract_invocations_total"] += 1
            values["nebula_contract_gas_sum"] += GAS_INSTRUCTIONS
            values["nebula_contract_gas_count"] += 1
            for index, upper in enumerate(GAS_BUCKETS):
                if GAS_INSTRUCTIONS <= upper:
                    values["nebula_contract_gas_buckets"][index] += 1
            values["soroban_gas_used_total"] += GAS_INSTRUCTIONS
            values["soroban_tx_success_total"] += 1
            values["soroban_active_users"] += 1

    if CONTRACT_ID:
        c_status, _payload = horizon_get(f"/contracts/{CONTRACT_ID}")
        if c_status == 0:
            c_status, _payload = horizon_get(f"/accounts/{CONTRACT_ID}")
        if c_status == 200:
            values["nebula_contract_scrape_success"] = 1
        elif c_status:
            values["nebula_contract_errors_total"] += 1
            values["soroban_tx_failure_total"] += 1
    if status != 200:
        values["nebula_contract_errors_total"] += 1
        values["soroban_tx_failure_total"] += 1

    # Persist the incremented counters back into module state.
    for key in _STATE:
        _STATE[key] = values[key]

    if values["nebula_contract_scrape_success"] == 1 or (
        not CONTRACT_ID and values["nebula_horizon_up"] == 1
    ):
        _LAST_SUCCESS_TS = time.time()

    values["nebula_contract_health_status"] = 1 if (
        values["nebula_horizon_up"] == 1
        and (not CONTRACT_ID or values["nebula_contract_scrape_success"] == 1)
    ) else 0
    values["nebula_contract_last_success_timestamp"] = _LAST_SUCCESS_TS
    values["nebula_exporter_up"] = 1
    values["nebula_contract_gas_instructions"] = GAS_INSTRUCTIONS
    return values


def _fmt(value: float) -> str:
    return str(int(value)) if float(value).is_integer() else f"{float(value):.6f}"


def _labels(**kwargs: str) -> str:
    return "{" + ",".join(f'{key}="{val}"' for key, val in kwargs.items()) + "}"


def _emit_gauge(lines: list, name: str, help_text: str, value: float) -> None:
    lines.extend([f"# HELP {name} {help_text}", f"# TYPE {name} gauge", f"{name} {_fmt(value)}"])


def _emit_counter(lines: list, name: str, help_text: str, value: float) -> None:
    lines.extend([f"# HELP {name} {help_text}", f"# TYPE {name} counter", f"{name} {_fmt(value)}"])


def _emit_histogram(
    lines: list,
    name: str,
    help_text: str,
    values: dict,
    bounds: tuple,
    sum_key: str,
    count_key: str,
    buckets_key: str,
) -> None:
    lines.extend([f"# HELP {name} {help_text}", f"# TYPE {name} histogram"])
    for upper, bucket in zip(bounds, values[buckets_key]):
        lines.append(f'{name}_bucket{{le="{upper:g}"}} {_fmt(bucket)}')
    lines.append(f'{name}_bucket{{le="+Inf"}} {_fmt(values[count_key])}')
    lines.append(f"{name}_sum {_fmt(values[sum_key])}")
    lines.append(f"{name}_count {_fmt(values[count_key])}")


def render_metrics(values: dict) -> str:
    lines: list[str] = []

    _emit_gauge(lines, "nebula_exporter_up", "1 if the contract exporter process is healthy.", values["nebula_exporter_up"])
    _emit_gauge(lines, "nebula_horizon_up", "1 if Horizon returned the latest ledger.", values["nebula_horizon_up"])
    _emit_gauge(lines, "nebula_horizon_latest_ledger", "Latest Horizon ledger sequence.", values["nebula_horizon_latest_ledger"])
    _emit_gauge(lines, "nebula_horizon_closed_at", "1 if the latest ledger had a closed_at timestamp.", values["nebula_horizon_closed_at"])
    _emit_gauge(lines, "nebula_contract_scrape_success", "1 if Horizon returned the configured contract.", values["nebula_contract_scrape_success"])
    _emit_gauge(lines, "nebula_contract_health_status", "1 = contract healthy, 0 = unhealthy.", values["nebula_contract_health_status"])
    _emit_gauge(lines, "nebula_contract_last_success_timestamp", "Unix timestamp of the last successful contract observation.", values["nebula_contract_last_success_timestamp"])

    # Labeled contract counters (labels required by alert rules / dashboards).
    lines.extend([
        "# HELP nebula_contract_invocations_total Contract invocation counter.",
        "# TYPE nebula_contract_invocations_total counter",
        f"nebula_contract_invocations_total{_labels(contract=CONTRACT_LABEL, method=METHOD_LABEL)} {_fmt(values['nebula_contract_invocations_total'])}",
        "# HELP nebula_contract_errors_total Contract error counter by reason.",
        "# TYPE nebula_contract_errors_total counter",
        f"nebula_contract_errors_total{_labels(contract=CONTRACT_LABEL, method=METHOD_LABEL, reason='scrape_failure')} {_fmt(values['nebula_contract_errors_total'])}",
    ])

    _emit_histogram(
        lines, "nebula_contract_gas_instructions",
        "Gas (CPU instructions) per contract invocation.",
        values, GAS_BUCKETS,
        "nebula_contract_gas_sum", "nebula_contract_gas_count", "nebula_contract_gas_buckets",
    )
    _emit_histogram(
        lines, "horizon_request_duration_seconds",
        "Horizon GET latency in seconds.",
        values, HORIZON_BUCKETS,
        "horizon_request_duration_sum", "horizon_request_duration_count", "horizon_request_duration_buckets",
    )
    # Legacy scalar kept for backwards compatibility with older dashboards.
    _emit_gauge(lines, "horizon_request_duration_seconds_last", "Horizon GET latency of the last scrape.", values["horizon_request_duration_seconds"])

    _emit_counter(lines, "soroban_tx_success_total", "Successful Soroban transactions.", values["soroban_tx_success_total"])
    _emit_counter(lines, "soroban_tx_failure_total", "Failed Soroban transactions.", values["soroban_tx_failure_total"])
    _emit_counter(lines, "soroban_gas_used_total", "Total Soroban gas consumed.", values["soroban_gas_used_total"])
    _emit_counter(lines, "cache_expirations_total", "Cache entries expired.", values["cache_expirations_total"])
    _emit_counter(lines, "soroban_active_users", "Observed active-user counter (use rate() for per-interval users).", values["soroban_active_users"])

    _emit_gauge(lines, "yield_forecast_stale_entries", "Number of stale yield forecast entries.", values["yield_forecast_stale_entries"])
    _emit_gauge(lines, "migration_started_timestamp", "Unix timestamp when the current migration started.", values["migration_started_timestamp"])
    _emit_gauge(lines, "migration_status", "1 = migration in_progress, 0 = idle/complete.", values["migration_status"])

    # Backup / recovery status (published via BACKUP_STATUS_FILE).
    backup_metrics = (
        ("backup_success", "1 if the last backup succeeded, 0 otherwise."),
        ("backup_last_success_timestamp", "Unix timestamp of the last successful backup."),
        ("backup_duration_seconds", "Duration of the last backup in seconds."),
        ("backup_size_bytes", "Size of the last backup in bytes."),
        ("backup_storage_available_bytes", "Available backup storage in bytes."),
        ("backup_storage_total_bytes", "Total backup storage in bytes."),
        ("recovery_success", "1 if the last recovery succeeded, 0 otherwise."),
        ("recovery_duration_seconds", "Duration of the last recovery in seconds."),
        ("last_drill_timestamp", "Unix timestamp of the last DR drill."),
    )
    for name, help_text in backup_metrics:
        _emit_gauge(lines, name, help_text, values[name])

    lines.append("")
    return "\n".join(lines)


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args) -> None:  # noqa: A003
        return

    def do_GET(self) -> None:  # noqa: N802
        if self.path.startswith("/health"):
            body = b'{"status":"ok"}\n'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        if self.path.startswith("/metrics"):
            body = render_metrics(collect()).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; version=0.0.4")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        self.send_response(404)
        self.end_headers()


def main() -> None:
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    print(f"nebula contract exporter listening on :{PORT} horizon={HORIZON_URL}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
