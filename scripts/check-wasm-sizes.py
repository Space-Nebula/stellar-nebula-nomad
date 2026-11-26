#!/usr/bin/env python3
"""Compare optimized WASM sizes against the checked-in baseline.

Called by .github/workflows/size-regression.yml. Reads the `SIZES` JSON from
the environment (produced by the measure step) and writes the new baseline
back to `WASM_BASELINE_FILE` (default `.wasm-sizes.json`).
"""

import json
import os
import sys

MAX_INCREASE_PCT = 10
MAX_BYTES = 65536


def main() -> int:
    baseline_file = os.environ.get("WASM_BASELINE_FILE", ".wasm-sizes.json")
    sizes_raw = os.environ.get("SIZES")
    if not sizes_raw:
        print("No sizes data available")
        return 0

    sizes = json.loads(sizes_raw)
    baseline = {}
    if os.path.exists(baseline_file):
        with open(baseline_file, encoding="utf-8") as f:
            baseline = json.load(f)

    failed = False
    report_lines = []

    for name, current_size in sorted(sizes.items()):
        if name in baseline:
            prev_size = baseline[name]
            if prev_size > 0:
                pct_change = ((current_size - prev_size) / prev_size) * 100
                if pct_change > MAX_INCREASE_PCT:
                    failed = True
                    report_lines.append(
                        f"❌ **{name}**: {prev_size} → {current_size} bytes "
                        f"(+{pct_change:.1f}%) — exceeds {MAX_INCREASE_PCT}% threshold"
                    )
                elif pct_change > 0:
                    report_lines.append(
                        f"⚠️ **{name}**: {prev_size} → {current_size} bytes (+{pct_change:.1f}%)"
                    )
                elif pct_change < 0:
                    report_lines.append(
                        f"✅ **{name}**: {prev_size} → {current_size} bytes "
                        f"({pct_change:.1f}%) — size decreased"
                    )
                else:
                    report_lines.append(f"✅ **{name}**: {current_size} bytes (unchanged)")
            else:
                report_lines.append(
                    f"📄 **{name}**: {current_size} bytes (new, previous was 0)"
                )
        else:
            report_lines.append(f"📄 **{name}**: {current_size} bytes (new — no baseline)")

    for name, current_size in sizes.items():
        if current_size > MAX_BYTES:
            failed = True
            report_lines.append(f"❌ **{name}**: {current_size} bytes exceeds 64 KB limit")

    report = "\n".join(report_lines)
    print(f"report={report}")
    print(f"failed={'true' if failed else 'false'}")

    # Make the report/failed values available to later steps.
    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output:
        with open(github_output, "a", encoding="utf-8") as out:
            out.write(f"report<<EOF\n{report}\nEOF\n")
            out.write(f"failed={'true' if failed else 'false'}\n")

    with open(baseline_file, "w", encoding="utf-8") as f:
        json.dump(sizes, f, indent=2)

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
