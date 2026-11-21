#!/usr/bin/env python3
"""Validate the Prometheus / Grafana monitoring setup (stdlib only).

Checks performed:
  1. Alert rule files define required alerts and no alert name is duplicated
     across the files Prometheus loads.
  2. prometheus.yml rule_files all point at existing files and include the
     backup rule file.
  3. contract_exporter.py source contains every metric required by the rules.
  4. Grafana dashboards are valid JSON with a title and at least one panel.

Exit code is 0 on success, 1 otherwise. Diagnostics are printed to stdout/stderr.
PyYAML is used opportunistically when installed; a regex fallback is used
otherwise so the script has no third-party dependencies.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

MONITORING_DIR = Path(__file__).resolve().parent
PROMETHEUS_YML = MONITORING_DIR / "prometheus" / "prometheus.yml"
RULE_FILES = [
    MONITORING_DIR / "prometheus" / "alert-rules.yml",
    MONITORING_DIR / "backup-monitoring.yml",
]
EXPORTER = MONITORING_DIR / "exporters" / "contract_exporter.py"
DASHBOARDS_DIR = MONITORING_DIR / "grafana" / "dashboards"

REQUIRED_ALERTS = {
    "BackupFailed",
    "NoRecentBackup",
    "RecoveryFailed",
    "HighContractErrorRate",
    "ContractUnhealthy",
}
# Backup alerts must live in exactly this file (and not in alert-rules.yml).
BACKUP_RULE_FILE = MONITORING_DIR / "backup-monitoring.yml"

REQUIRED_METRICS = [
    "nebula_contract_invocations_total",
    "nebula_contract_errors_total",
    "nebula_contract_health_status",
    "nebula_contract_last_success_timestamp",
    "nebula_contract_gas_instructions",
    "horizon_request_duration_seconds",
    "soroban_tx_success_total",
    "soroban_tx_failure_total",
    "soroban_gas_used_total",
    "cache_expirations_total",
    "soroban_active_users",
    "backup_success",
    "backup_last_success_timestamp",
    "backup_duration_seconds",
    "backup_size_bytes",
    "backup_storage_available_bytes",
    "backup_storage_total_bytes",
    "recovery_success",
    "recovery_duration_seconds",
    "last_drill_timestamp",
    "yield_forecast_stale_entries",
    "migration_started_timestamp",
    "migration_status",
]
# Provided by node-exporter / Prometheus itself, not by our exporter.
EXTERNAL_METRICS = {
    "node_memory_MemAvailable_bytes",
    "node_memory_MemTotal_bytes",
    "node_filesystem_avail_bytes",
    "node_filesystem_size_bytes",
    "up",
}
REQUIRED_DASHBOARDS = ["contract-health.json", "backup-recovery.json"]

errors: list[str] = []
notes: list[str] = []


def fail(message: str) -> None:
    errors.append(message)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        fail(f"cannot read {path}: {exc}")
        return ""


def load_alert_names(path: Path) -> list[str]:
    """Extract alert names, using PyYAML when available, regex otherwise."""
    text = read_text(path)
    if not text:
        return []
    try:
        import yaml  # type: ignore

        doc = yaml.safe_load(text) or {}
        names = []
        for group in doc.get("groups", []) or []:
            for rule in group.get("rules", []) or []:
                if isinstance(rule, dict) and rule.get("alert"):
                    names.append(str(rule["alert"]))
        notes.append(f"parsed {path.name} with PyYAML")
        return names
    except ImportError:
        pass
    except Exception as exc:  # malformed YAML
        fail(f"{path}: YAML parse error: {exc}")
        return []
    # Lightweight fallback: top-level alert declarations inside rule groups.
    names = re.findall(r"(?m)^\s*-\s*alert:\s*([A-Za-z_][A-Za-z0-9_]*)", text)
    notes.append(f"parsed {path.name} with regex fallback (PyYAML unavailable)")
    return names


def check_alerts() -> None:
    before = len(errors)
    all_names: dict[str, list[str]] = {}
    per_file: dict[str, set[str]] = {}
    for path in RULE_FILES:
        names = load_alert_names(path)
        per_file[path.name] = set(names)
        for name in names:
            all_names.setdefault(name, []).append(path.name)

    for name, files in sorted(all_names.items()):
        if len(files) > 1:
            fail(f"duplicate alert '{name}' defined in: {', '.join(files)}")

    found = set(all_names)
    for required in sorted(REQUIRED_ALERTS):
        if required not in found:
            fail(f"required alert '{required}' is not defined in any rule file")

    if "BackupFailed" in per_file.get("alert-rules.yml", set()):
        fail("BackupFailed must be defined only in backup-monitoring.yml, not alert-rules.yml")
    if "NoRecentBackup" in per_file.get("alert-rules.yml", set()):
        fail("NoRecentBackup must be defined only in backup-monitoring.yml, not alert-rules.yml")
    for name in ("BackupFailed", "NoRecentBackup"):
        if name not in per_file.get("backup-monitoring.yml", set()):
            fail(f"{name} missing from backup-monitoring.yml")

    if len(errors) == before:
        print(f"[OK] alert rules: {len(found)} unique alerts across {len(RULE_FILES)} files (required alerts present)")


def parse_rule_files(text: str) -> list[str]:
    refs: list[str] = []
    in_block = False
    for line in text.splitlines():
        if re.match(r"^rule_files\s*:", line):
            in_block = True
            continue
        if not in_block:
            continue
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if not line.startswith((" ", "\t")):
            break  # next top-level key
        match = re.match(r"^\s+-\s+(.+)$", line)
        if not match:
            continue
        value = match.group(1).split("#", 1)[0].strip().strip("\"'")
        if value:
            refs.append(value)
    return refs


def check_prometheus_config() -> None:
    before = len(errors)
    text = read_text(PROMETHEUS_YML)
    if not text:
        return
    refs = parse_rule_files(text)
    if not refs:
        fail("prometheus.yml has no rule_files entries")
        return
    for ref in refs:
        resolved = (PROMETHEUS_YML.parent / ref).resolve()
        if not resolved.is_file():
            fail(f"prometheus.yml rule_files entry '{ref}' does not exist (expected {resolved})")
    if not any(Path(ref).name == "backup-monitoring.yml" for ref in refs):
        fail("prometheus.yml rule_files does not reference backup-monitoring.yml")
    if len(errors) == before:
        print(f"[OK] prometheus.yml rule_files: {len(refs)} entries, all resolve to existing files")


def check_exporter() -> None:
    before = len(errors)
    text = read_text(EXPORTER)
    if not text:
        return
    missing = [name for name in REQUIRED_METRICS if name not in text]
    if missing:
        fail(f"contract_exporter.py missing required metric names: {', '.join(missing)}")
    if "BACKUP_STATUS_FILE" not in text:
        fail("contract_exporter.py does not read BACKUP_STATUS_FILE for backup status ingest")
    # Every metric referenced by a rule must be exported or explicitly external.
    # Rule files also contain YAML group names (e.g. "backup_alerts") and
    # histogram suffixes (_bucket/_sum/_count); normalise those away.
    expr_sources = "\n".join(read_text(path) for path in RULE_FILES)
    referenced = set(re.findall(r"\b(?:node_|nebula_|soroban_|backup_|recovery_|horizon_|cache_|yield_|migration_|last_drill)[A-Za-z0-9_]*", expr_sources))
    non_metrics = {"backup_alerts", "recovery_alerts"}
    normalised = set()
    for name in referenced:
        if name in non_metrics:
            continue
        for suffix in ("_bucket", "_sum", "_count", "_last"):
            if name.endswith(suffix):
                name = name[: -len(suffix)]
        normalised.add(name)
    unreachable = sorted(m for m in normalised if m not in EXTERNAL_METRICS and m not in text)
    if unreachable:
        fail(f"metrics referenced by rules but not exported/documented: {', '.join(unreachable)}")
    if len(errors) == before:
        print(f"[OK] exporter: {len(REQUIRED_METRICS)} required metrics present; all rule metrics resolvable")


def check_dashboards() -> None:
    before = len(errors)
    files = sorted(DASHBOARDS_DIR.glob("*.json")) if DASHBOARDS_DIR.is_dir() else []
    if not files:
        fail(f"no Grafana dashboards found in {DASHBOARDS_DIR}")
        return
    for name in REQUIRED_DASHBOARDS:
        if not (DASHBOARDS_DIR / name).is_file():
            fail(f"required dashboard missing: {DASHBOARDS_DIR / name}")
    for path in files:
        try:
            doc = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            fail(f"invalid JSON in {path}: {exc}")
            continue
        if not isinstance(doc, dict):
            fail(f"{path}: dashboard JSON is not an object")
            continue
        if not doc.get("title"):
            fail(f"{path}: dashboard has no title")
        panels = doc.get("panels")
        if not isinstance(panels, list) or not panels:
            fail(f"{path}: dashboard has no panels")
        if not doc.get("schemaVersion"):
            fail(f"{path}: dashboard has no schemaVersion")
        if not doc.get("uid"):
            fail(f"{path}: dashboard has no uid")
    if len(errors) == before:
        print(f"[OK] Grafana: {len(files)} dashboards valid (title, uid, schemaVersion, panels)")


def main() -> int:
    check_alerts()
    check_prometheus_config()
    check_exporter()
    check_dashboards()

    for note in notes:
        print(f"[info] {note}")
    if errors:
        print("\nMonitoring validation FAILED:", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        return 1
    print("\nMonitoring validation PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
