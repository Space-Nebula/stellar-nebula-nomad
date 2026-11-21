#!/usr/bin/env python3
"""Contract size regression monitoring for optimized Stellar/Soroban WASM.

This helper is intentionally dependency-free (Python 3 standard library only)
so it can run inside GitHub Actions without an install step and locally for
quick verification.

Subcommands
-----------
compare   Compare current contract sizes against a baseline, produce a
          Markdown table and emit GitHub Actions outputs (report, failed,
          summary) to $GITHUB_OUTPUT when that variable is set.
history   Append the current sizes to a bounded JSON history file.
selftest  Run built-in assertions covering the comparison and history logic.

See docs/CONTRACT_SIZE_MONITORING.md for the full workflow documentation.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import sys
from datetime import datetime, timezone

DEFAULT_CONFIG_PATH = ".github/contract-size-config.json"
DEFAULT_BASELINE_PATH = ".wasm-sizes.json"
DEFAULT_HISTORY_PATH = ".wasm-size-history.json"
DEFAULT_CURRENT_ENV = "SIZES"

DEFAULT_CONFIG = {
    "max_increase_pct": 10.0,
    "max_increase_bytes": 8192,
    "max_bytes": 65536,
    "history_file": DEFAULT_HISTORY_PATH,
    "max_history_entries": 200,
}

STATUS_LABELS = {
    "regression": "❌ regression",
    "increase": "⚠️ increased",
    "decrease": "✅ decreased",
    "unchanged": "➖ unchanged",
    "new": "🆕 new",
    "removed": "⬅️ removed",
}


# --------------------------------------------------------------------------- #
# Generic helpers (robust against missing / malformed input)
# --------------------------------------------------------------------------- #
def warn(message: str) -> None:
    """Emit a warning without ever raising, so a bad input degrades cleanly."""
    print(f"warning: {message}", file=sys.stderr)


def read_json(path, label, default):
    """Read a JSON file, returning *default* instead of crashing on failure."""
    if not path:
        return dict(default) if isinstance(default, dict) else default
    if not os.path.isfile(path):
        warn(f"{label} file '{path}' not found; using defaults")
        return dict(default) if isinstance(default, dict) else default
    try:
        with open(path, "r", encoding="utf-8") as handle:
            text = handle.read()
    except OSError as exc:
        warn(f"could not read {label} file '{path}' ({exc}); using defaults")
        return dict(default) if isinstance(default, dict) else default
    if not text.strip():
        warn(f"{label} file '{path}' is empty; using defaults")
        return dict(default) if isinstance(default, dict) else default
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        warn(f"{label} file '{path}' contains invalid JSON ({exc}); using defaults")
        return dict(default) if isinstance(default, dict) else default


def as_float(value, fallback, label):
    try:
        return float(value)
    except (TypeError, ValueError):
        warn(f"{label}={value!r} is not a number; using {fallback}")
        return fallback


def as_int(value, fallback, label):
    try:
        parsed = int(value)
        if isinstance(value, float) and value != parsed:
            warn(f"{label}={value!r} is not a whole number; using {parsed}")
        return parsed
    except (TypeError, ValueError):
        warn(f"{label}={value!r} is not an integer; using {fallback}")
        return fallback


def load_config(path):
    """Load thresholds with defaults for any missing or invalid field."""
    config = dict(DEFAULT_CONFIG)
    data = read_json(path, "config", {})
    if not isinstance(data, dict):
        warn(f"config '{path}' is not a JSON object; using defaults")
        return config
    for key in DEFAULT_CONFIG:
        if key in data and data[key] is not None:
            config[key] = data[key]

    config["max_increase_pct"] = as_float(
        config["max_increase_pct"], 10.0, "max_increase_pct"
    )
    config["max_increase_bytes"] = as_int(
        config["max_increase_bytes"], 8192, "max_increase_bytes"
    )
    config["max_bytes"] = as_int(config["max_bytes"], 65536, "max_bytes")
    config["max_history_entries"] = as_int(
        config["max_history_entries"], 200, "max_history_entries"
    )

    if config["max_increase_pct"] < 0:
        warn("max_increase_pct must be >= 0; using 0")
        config["max_increase_pct"] = 0.0
    if config["max_increase_bytes"] < 0:
        warn("max_increase_bytes must be >= 0; using 0")
        config["max_increase_bytes"] = 0
    if config["max_bytes"] <= 0:
        warn("max_bytes must be > 0; using 65536")
        config["max_bytes"] = 65536
    if config["max_history_entries"] < 1:
        warn("max_history_entries must be >= 1; using 200")
        config["max_history_entries"] = 200
    if not isinstance(config["history_file"], str) or not config["history_file"]:
        warn("history_file must be a non-empty string; using default")
        config["history_file"] = DEFAULT_HISTORY_PATH
    return config


def normalize_sizes(data):
    """Coerce a name -> bytes JSON object into name -> non-negative int."""
    if not isinstance(data, dict):
        warn("sizes data is not a JSON object; treating as empty")
        return {}
    sizes = {}
    for name, value in data.items():
        if isinstance(value, bool):
            warn(f"ignoring non-numeric size for '{name}' ({value!r})")
            continue
        try:
            size = int(value)
        except (TypeError, ValueError):
            warn(f"ignoring non-numeric size for '{name}' ({value!r})")
            continue
        if size < 0:
            warn(f"ignoring negative size for '{name}'")
            continue
        sizes[str(name)] = size
    return sizes


def load_sizes(path=None, env_name=None, label="sizes"):
    """Return (sizes_dict, source_description).

    A file path takes precedence over an environment variable.  Missing or
    malformed inputs yield an empty dict plus a warning instead of an error.
    """
    if path:
        return normalize_sizes(read_json(path, label, {})), f"file:{path}"
    if env_name:
        raw = os.environ.get(env_name)
        if raw is None or not raw.strip():
            warn(f"environment variable {env_name} is unset or empty; treating as no sizes")
            return {}, f"env:{env_name} (empty)"
        try:
            data = json.loads(raw)
        except json.JSONDecodeError as exc:
            warn(
                f"environment variable {env_name} does not contain valid JSON "
                f"({exc}); treating as no sizes"
            )
            return {}, f"env:{env_name} (invalid)"
        return normalize_sizes(data), f"env:{env_name}"
    return {}, "none"


def atomic_write_json(path, data):
    """Write JSON atomically so an interrupted run cannot corrupt history."""
    directory = os.path.dirname(os.path.abspath(path)) or "."
    os.makedirs(directory, exist_ok=True)
    tmp = f"{path}.tmp.{os.getpid()}"
    try:
        with open(tmp, "w", encoding="utf-8") as handle:
            json.dump(data, handle, indent=2, sort_keys=True)
            handle.write("\n")
        os.replace(tmp, path)
    except OSError as exc:
        try:
            if os.path.exists(tmp):
                os.remove(tmp)
        except OSError:
            pass
        raise SystemExit(f"error: could not write '{path}': {exc}")


def utc_now_iso():
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


# --------------------------------------------------------------------------- #
# Comparison logic
# --------------------------------------------------------------------------- #
def compare(current, baseline, config):
    """Compare two name -> bytes maps.

    A contract is a regression when it exceeds ANY of:
      * the relative growth limit   (max_increase_pct)
      * the absolute growth limit   (max_increase_bytes)
      * the per-contract hard cap   (max_bytes)

    New and removed contracts are reported but are not regressions by
    themselves; a new contract can still breach the hard cap.
    """
    max_pct = config["max_increase_pct"]
    max_bytes_inc = config["max_increase_bytes"]
    max_bytes = config["max_bytes"]

    rows = []
    counts = {
        "regression": 0,
        "increase": 0,
        "decrease": 0,
        "unchanged": 0,
        "new": 0,
        "removed": 0,
    }
    failed = False

    for name in sorted(set(current) | set(baseline)):
        in_current = name in current
        in_baseline = name in baseline
        before = baseline.get(name)
        after = current.get(name)
        delta = None
        pct = None
        reasons = []

        if in_current and after > max_bytes:
            reasons.append(f"{after} B exceeds {max_bytes} B hard cap")
        if in_current and in_baseline:
            delta = after - before
            if before > 0:
                pct = (delta / before) * 100.0
                if pct > max_pct:
                    reasons.append(f"+{pct:.1f}% exceeds {max_pct}% limit")
            # A zero baseline makes the percentage undefined. Never divide by
            # zero: pct stays None and the absolute delta is reported instead.
            if delta > max_bytes_inc:
                reasons.append(f"+{delta} B exceeds {max_bytes_inc} B absolute limit")

        if reasons:
            status = "regression"
            failed = True
        elif in_current and not in_baseline:
            status = "new"
        elif in_baseline and not in_current:
            status = "removed"
        elif delta > 0:
            status = "increase"
        elif delta < 0:
            status = "decrease"
        else:
            status = "unchanged"

        counts[status] += 1
        rows.append(
            {
                "name": name,
                "before": before,
                "after": after,
                "delta": delta,
                "pct": pct,
                "status": status,
                "reason": "; ".join(reasons),
            }
        )

    return {"rows": rows, "counts": counts, "failed": failed}


def fmt_bytes(value):
    if value is None:
        return "—"
    if value >= 1024:
        return f"{value} B ({value / 1024:.1f} KB)"
    return f"{value} B"


def fmt_pct_value(value):
    if value is None:
        return "—"
    return f"+{value:.1f}%" if value > 0 else f"{value:.1f}%"


def fmt_delta(value):
    if value is None:
        return "—"
    return f"+{value}" if value > 0 else str(value)


def summary_line(counts, total):
    return (
        f"**Summary:** {total} contract(s) — "
        f"{counts['regression']} regression, {counts['increase']} increased, "
        f"{counts['decrease']} decreased, {counts['unchanged']} unchanged, "
        f"{counts['new']} new, {counts['removed']} removed."
    )


def render_report(result, config, current_source, baseline_source):
    rows = result["rows"]
    lines = ["## 📦 Contract Size Report", ""]
    if not rows:
        lines.append("_No contract size data was available to compare._")
    else:
        lines.append("| Contract | Before | After | Δ Bytes | Δ % | Status |")
        lines.append("|:---|---:|---:|---:|---:|:---|")
        for row in rows:
            status = STATUS_LABELS[row["status"]]
            if row["reason"]:
                status += f"<br>_{row['reason']}_"
            lines.append(
                f"| `{row['name']}` | {fmt_bytes(row['before'])} | "
                f"{fmt_bytes(row['after'])} | {fmt_delta(row['delta'])} | "
                f"{fmt_pct_value(row['pct'])} | {status} |"
            )
        lines.append("")
        before_total = sum(r["before"] for r in rows if r["before"] is not None)
        after_total = sum(r["after"] for r in rows if r["after"] is not None)
        delta_total = after_total - before_total
        pct_total = (delta_total / before_total * 100.0) if before_total > 0 else None
        lines.append(
            f"**Totals:** before {before_total} B · after {after_total} B · "
            f"change {fmt_delta(delta_total)} B ({fmt_pct_value(pct_total)})"
        )

    lines.append("")
    lines.append(summary_line(result["counts"], len(rows)))
    lines.append("")
    lines.append(
        f"_Limits: +{config['max_increase_pct']}% / +{config['max_increase_bytes']} B "
        f"per contract, {config['max_bytes']} B hard cap. "
        f"Current: {current_source} · baseline: {baseline_source}._"
    )
    lines.append("")
    if result["failed"]:
        lines.append("### ❌ Size regression detected")
        lines.append("One or more contracts exceeded the configured limits; see the ❌ rows above.")
    else:
        lines.append("✅ All contract sizes are within the configured limits.")
    return "\n".join(lines)


def pick_delimiter(value):
    base = "CONTRACT_SIZE_OUTPUT_EOF"
    delimiter = base
    counter = 0
    while any(line.strip() == delimiter for line in value.splitlines()):
        counter += 1
        delimiter = f"{base}_{counter}"
    return delimiter


def write_github_output(outputs):
    """Append key/value pairs to $GITHUB_OUTPUT using heredoc for multiline values."""
    path = os.environ.get("GITHUB_OUTPUT")
    if not path:
        return False
    try:
        with open(path, "a", encoding="utf-8") as handle:
            for key, value in outputs.items():
                value = "" if value is None else str(value)
                if "\n" in value or "\r" in value:
                    delimiter = pick_delimiter(value)
                    handle.write(f"{key}<<{delimiter}\n{value}\n{delimiter}\n")
                else:
                    handle.write(f"{key}={value}\n")
        return True
    except OSError as exc:
        warn(f"could not write to GITHUB_OUTPUT ('{path}'): {exc}")
        return False


# --------------------------------------------------------------------------- #
# Subcommands
# --------------------------------------------------------------------------- #
def cmd_compare(args):
    config = load_config(args.config)

    current_env = args.current_env
    if not args.current and not current_env:
        current_env = DEFAULT_CURRENT_ENV
    current, current_source = load_sizes(
        path=args.current, env_name=current_env, label="current sizes"
    )
    baseline, baseline_source = load_sizes(path=args.baseline, label="baseline sizes")

    result = compare(current, baseline, config)
    report = render_report(result, config, current_source, baseline_source)
    summary = summary_line(result["counts"], len(result["rows"]))
    failed = "true" if result["failed"] else "false"

    if write_github_output({"report": report, "failed": failed, "summary": summary}):
        print(f"Wrote report/failed/summary to GITHUB_OUTPUT (failed={failed}).")
    else:
        # GITHUB_OUTPUT is unset: this is a local run, so print the report.
        print(report)
        print()
        print(f"failed={failed}")

    if args.strict and result["failed"]:
        print("error: size regression detected (strict mode enabled)", file=sys.stderr)
        return 1
    return 0


def cmd_history(args):
    config = load_config(args.config)
    history_path = args.history_file or config["history_file"]

    sizes_env = args.sizes_env
    if not args.sizes and not sizes_env:
        sizes_env = DEFAULT_CURRENT_ENV
    sizes, _ = load_sizes(path=args.sizes, env_name=sizes_env, label="sizes")

    data = read_json(history_path, "history", {"version": 1, "entries": []})
    if not isinstance(data, dict):
        warn("history root is not a JSON object; starting a fresh history")
        data = {"version": 1, "entries": []}
    entries = data.get("entries")
    if not isinstance(entries, list):
        warn("history 'entries' is not a list; starting a fresh history")
        entries = []
    entries = [entry for entry in entries if isinstance(entry, dict)]

    entry = {
        "timestamp": utc_now_iso(),
        "commit": args.commit or os.environ.get("GITHUB_SHA", ""),
        "ref": args.ref or os.environ.get("GITHUB_REF", ""),
        "ref_name": args.ref_name or os.environ.get("GITHUB_REF_NAME", ""),
        "event": args.event or os.environ.get("GITHUB_EVENT_NAME", ""),
        "sizes": sizes,
    }
    entries.append(entry)

    max_entries = args.max_entries if args.max_entries is not None else config["max_history_entries"]
    trimmed = max(0, len(entries) - max_entries)
    if trimmed:
        entries = entries[-max_entries:]

    atomic_write_json(history_path, {"version": 1, "entries": entries})

    short_sha = entry["commit"][:12] if entry["commit"] else "no-sha"
    print(
        f"Appended history entry (event={entry['event'] or 'unknown'}, "
        f"commit={short_sha}) to '{history_path}'."
    )
    if trimmed:
        print(f"Trimmed {trimmed} oldest entr{'y' if trimmed == 1 else 'ies'} "
              f"(keeping the last {max_entries}).")
    return 0


def cmd_selftest(args):
    """Small dependency-free test suite for the comparison/history logic."""
    failures = []

    def check(condition, message):
        if not condition:
            failures.append(message)

    config = dict(DEFAULT_CONFIG)
    config["max_increase_pct"] = 10
    config["max_increase_bytes"] = 100
    config["max_bytes"] = 1000

    result = compare({"a.wasm": 110}, {"a.wasm": 100}, config)
    check(not result["failed"], "a 10% increase should NOT fail at a 10% limit")
    check(result["rows"][0]["status"] == "increase", "10% increase should be 'increase'")

    result = compare({"a.wasm": 120}, {"a.wasm": 100}, config)
    check(result["failed"], "a 20% increase should fail")
    check(result["rows"][0]["status"] == "regression", "20% increase should be 'regression'")

    result = compare({"a.wasm": 150}, {"a.wasm": 100}, config)
    check(result["failed"], "an absolute byte breach should fail")

    result = compare({"a.wasm": 2000}, {"a.wasm": 50}, config)
    check(result["failed"], "a hard-cap breach should fail")

    result = compare({"a.wasm": 90}, {"a.wasm": 100}, config)
    check(result["rows"][0]["status"] == "decrease", "a smaller contract should be 'decrease'")
    check(not result["failed"], "a decrease should never fail")

    result = compare({"a.wasm": 100}, {}, config)
    check(result["rows"][0]["status"] == "new", "a contract without a baseline should be 'new'")

    result = compare({"a.wasm": 100}, {"a.wasm": 0, "b.wasm": 50}, config)
    check(not result["failed"], "a zero baseline must not divide by zero")
    check(result["counts"]["removed"] == 1, "a removed contract should be counted")

    check(
        normalize_sizes({"x": "42", "y": "bad", "z": -1}) == {"x": 42},
        "size normalization should coerce numeric strings and drop invalid values",
    )

    check(
        pick_delimiter("a\nCONTRACT_SIZE_OUTPUT_EOF\nb") == "CONTRACT_SIZE_OUTPUT_EOF_1",
        "output delimiter should avoid collisions with the value",
    )

    import tempfile

    with tempfile.TemporaryDirectory() as directory:
        history_path = os.path.join(directory, "history.json")
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            for index in range(5):
                cmd_history(
                    argparse.Namespace(
                        config=None,
                        history_file=history_path,
                        sizes=None,
                        sizes_env=None,
                        commit=f"sha-{index}",
                        ref="refs/heads/main",
                        ref_name="main",
                        event="push",
                        max_entries=3,
                    )
                )
            history_data = read_json(history_path, "history", {})
        check(len(history_data["entries"]) == 3, "history should be bounded to 3 entries")
        check(
            history_data["entries"][-1]["commit"] == "sha-4",
            "history should keep the newest entries",
        )
        check(
            "timestamp" in history_data["entries"][-1],
            "history entries should include a timestamp",
        )

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        print(f"{len(failures)} selftest check(s) failed.", file=sys.stderr)
        return 1
    print("All selftest checks passed.")
    return 0


# --------------------------------------------------------------------------- #
# Entrypoint
# --------------------------------------------------------------------------- #
def build_parser():
    parser = argparse.ArgumentParser(
        prog="contract_size.py",
        description="Track and compare optimized WASM contract sizes.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  contract_size.py compare --current sizes.json --baseline .wasm-sizes.json\n"
            "  contract_size.py compare --current-env SIZES --strict\n"
            "  contract_size.py history --sizes-env SIZES\n"
            "  contract_size.py selftest\n"
        ),
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    compare_parser = subparsers.add_parser(
        "compare", help="compare current sizes with a baseline and emit a report"
    )
    compare_parser.add_argument("--current", help="path to current sizes JSON file")
    compare_parser.add_argument(
        "--current-env",
        help=(
            f"env var holding current sizes JSON (default {DEFAULT_CURRENT_ENV} "
            "when --current is omitted)"
        ),
    )
    compare_parser.add_argument(
        "--baseline",
        default=DEFAULT_BASELINE_PATH,
        help=f"path to baseline sizes JSON (default: {DEFAULT_BASELINE_PATH})",
    )
    compare_parser.add_argument(
        "--config",
        default=DEFAULT_CONFIG_PATH,
        help=f"path to thresholds config JSON (default: {DEFAULT_CONFIG_PATH})",
    )
    compare_parser.add_argument(
        "--strict",
        action="store_true",
        help="exit with code 1 when a regression is detected",
    )
    compare_parser.set_defaults(func=cmd_compare)

    history_parser = subparsers.add_parser(
        "history", help="append current sizes to the bounded history file"
    )
    history_parser.add_argument("--sizes", help="path to sizes JSON file")
    history_parser.add_argument(
        "--sizes-env",
        help=f"env var holding sizes JSON (default {DEFAULT_CURRENT_ENV})",
    )
    history_parser.add_argument(
        "--config",
        default=DEFAULT_CONFIG_PATH,
        help=f"path to thresholds config JSON (default: {DEFAULT_CONFIG_PATH})",
    )
    history_parser.add_argument("--history-file", help="override history_file from config")
    history_parser.add_argument("--commit", help="commit SHA (default: $GITHUB_SHA)")
    history_parser.add_argument("--ref", help="git ref (default: $GITHUB_REF)")
    history_parser.add_argument("--ref-name", help="branch name (default: $GITHUB_REF_NAME)")
    history_parser.add_argument("--event", help="event name (default: $GITHUB_EVENT_NAME)")
    history_parser.add_argument(
        "--max-entries", type=int, help="override max_history_entries from config"
    )
    history_parser.set_defaults(func=cmd_history)

    selftest_parser = subparsers.add_parser(
        "selftest", help="run built-in assertions without external dependencies"
    )
    selftest_parser.set_defaults(func=cmd_selftest)

    return parser


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
