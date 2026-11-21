# Contract Size Monitoring

This repository tracks the size of every optimized WASM contract so that an
accidental size regression is caught in pull requests instead of after a
release. The check is implemented by
[`contract_size.py`](../.github/scripts/contract_size.py) (Python 3 standard
library only) and driven by
[`.github/workflows/size-regression.yml`](../.github/workflows/size-regression.yml).

## How it works

On every pull request to `main` and every push to `main`, the workflow:

1. Builds the contracts with `stellar contract build` for the `wasm32v1-none`
   target and optimizes each artifact with `stellar contract optimize` into
   `wasm/*.optimized.wasm`.
2. Measures each optimized artifact and records `filename -> byte count` as
   JSON (also exposed as the `measure` step output `sizes`).
3. **Fetches the baseline into a temporary file** with `git show`. The source
   is the target branch for pull requests (`origin/main`) and the parent commit
   for pushes to `main` (`HEAD~1`). The committed `.wasm-sizes.json` in the
   working tree is *not* modified at this point, so the report compares real
   "before" and "after" values.
4. Runs `contract_size.py compare`, which emits a Markdown table and three
   GitHub Actions outputs: `report`, `failed`, and `summary`.
5. Runs `contract_size.py history` to append the measurement to the history
   database.
6. Uploads the optimized WASM and a report bundle as workflow artifacts.
7. Posts/updates a single PR comment containing the captured `report` (the
   comment is identified by an HTML marker so repeated pushes update it rather
   than spamming new comments).
8. Fails the job if `failed == true` — i.e. when a regression is detected.
9. On `main` only, and only when no regression was found, writes the new sizes
   to `.wasm-sizes.json`, appends to `.wasm-size-history.json`, and commits both
   with a `[skip ci]` message so the commit does not re-trigger the workflow.

> The baseline is read from git rather than from the working tree. This is the
> key fix over the original workflow: the PR report no longer shows "before"
> sizes equal to "after" sizes, and `steps.compare.outputs.report` is now
> populated (the script writes real GitHub Actions outputs, including a
> heredoc block for the multiline report).

## Thresholds and configuration

All limits live in
[`.github/contract-size-config.json`](../.github/contract-size-config.json):

| Key | Default | Meaning |
|-----|---------|---------|
| `max_increase_pct` | `10` | Maximum relative growth versus the baseline, in percent. A change strictly greater than this is a regression. |
| `max_increase_bytes` | `8192` | Maximum absolute growth versus the baseline, in bytes. A change strictly greater than this is a regression. |
| `max_bytes` | `65536` | Hard per-contract cap (64 KB). Any contract larger than this is a regression regardless of the baseline. |
| `history_file` | `.wasm-size-history.json` | Path to the history database. |
| `max_history_entries` | `200` | Number of most-recent history entries to keep. |

A contract is flagged as a **regression** when it exceeds *any* of the three
limits. New and removed contracts are reported but are not regressions by
themselves; a brand-new contract can still fail the `max_bytes` hard cap.
Decreases are always welcome and never fail the check.

Missing or malformed config fields fall back to the defaults with a warning on
stderr; a missing baseline is treated as an empty object (first measurement),
so the workflow never crashes on a fresh repository. Percentage change is
undefined when the baseline is zero, and is reported as `—` rather than
dividing by zero.

## Interpreting the PR report

The bot comment looks like this:

```markdown
## 📦 Contract Size Report

| Contract | Before | After | Δ Bytes | Δ % | Status |
|:---|---:|---:|---:|---:|:---|
| `example.wasm` | 45,000 B (44.0 KB) | 46,000 B (44.9 KB) | +1000 | +2.2% | ✅ decreased / ⚠️ increased … |
| `new_contract.wasm` | — | 30,000 B (29.3 KB) | — | — | 🆕 new |

**Totals:** before 45000 B · after 76000 B · change +31000 B (+68.9%)
**Summary:** 2 contract(s) — 0 regression, 1 increased, 0 decreased, 0 unchanged, 1 new, 0 removed.
```

Status meanings:

- ❌ **regression** — exceeded a configured limit; the job fails. The reason
  (e.g. `+12.4% exceeds 10% limit`, `+9000 B exceeds 8192 B absolute limit`,
  or `70000 B exceeds 65536 B hard cap`) is shown under the row.
- ⚠️ **increased** — grew, but stayed within all limits.
- ✅ **decreased** — got smaller (good).
- ➖ **unchanged** — byte-for-byte identical.
- 🆕 **new** — no baseline entry existed; informational.
- ⬅️ **removed** — present in the baseline but not in the current build;
  informational (worth a human glance).

The `Totals` line sums only contracts that exist in each column, so it gives a
quick sense of overall direction but is not itself a pass/fail criterion.

## Size history

[`.wasm-size-history.json`](../.wasm-size-history.json) is a small JSON
"database" that starts as:

```json
{ "version": 1, "entries": [] }
```

Each workflow run appends one entry:

```json
{
  "timestamp": "2026-11-19T12:34:56Z",
  "commit": "abc123...",
  "ref": "refs/heads/main",
  "ref_name": "main",
  "event": "push",
  "sizes": { "example.wasm": 46000 }
}
```

Fields come from the `GITHUB_SHA`, `GITHUB_REF`, `GITHUB_REF_NAME`, and
`GITHUB_EVENT_NAME` environment variables and can be overridden with CLI
flags. Only the newest `max_history_entries` entries are retained; the file is
written atomically so an interrupted run cannot leave it corrupted. The entire
file is also uploaded as the `contract-size-report-*` artifact on every run,
so history is recoverable even if the commit step is skipped.

## Tuning the limits

Edit `.github/contract-size-config.json` and commit the change. To allow a
larger contract, raise `max_bytes`; to be stricter about growth, lower
`max_increase_pct` or `max_increase_bytes`. Because the config is read at run
time, no code changes are needed. Individual runs can override the history
settings with `history --history-file` / `history --max-entries`, and the
compare step can be made self-failing with `compare --strict` (the workflow
intentionally does *not* use `--strict` so that the PR comment and history
steps still run after a regression is detected).

## Testing locally

The script has no third-party dependencies. Check the CLI:

```bash
python3 .github/scripts/contract_size.py --help
```

Run the built-in assertions (comparison thresholds, zero-baseline division,
new/removed detection, history bounding):

```bash
python3 .github/scripts/contract_size.py selftest
```

Compare two sample size snapshots. Create the files:

```bash
cat > /tmp/baseline.json <<'EOF'
{
  "example.wasm": 50000,
  "shrinking.wasm": 20000
}
EOF

cat > /tmp/current.json <<'EOF'
{
  "example.wasm": 58000,
  "shrinking.wasm": 19000,
  "brand_new.wasm": 12000
}
EOF
```

Then run the comparison against a temporary config with relaxed limits so you
can exercise each status:

```bash
cat > /tmp/config.json <<'EOF'
{
  "max_increase_pct": 10,
  "max_increase_bytes": 8192,
  "max_bytes": 65536,
  "history_file": ".wasm-size-history.json",
  "max_history_entries": 200
}
EOF

python3 .github/scripts/contract_size.py compare \
  --current /tmp/current.json \
  --baseline /tmp/baseline.json \
  --config /tmp/config.json
```

Expected result: `example.wasm` is +16% / +8000 B, so it is reported as an ❌
regression (the +16% exceeds 10%); `shrinking.wasm` is ✅ decreased; and
`brand_new.wasm` is 🆕 new. Add `--strict` to make the process exit with code
`1` for the regression:

```bash
python3 .github/scripts/contract_size.py compare \
  --current /tmp/current.json \
  --baseline /tmp/baseline.json \
  --config /tmp/config.json --strict; echo "exit=$?"
```

You can also feed sizes through the environment exactly as the workflow does,
and simulate GitHub Actions outputs:

```bash
SIZES='{"example.wasm": 58000}' GITHUB_OUTPUT=/tmp/gh_output \
  python3 .github/scripts/contract_size.py compare \
    --current-env SIZES --baseline /tmp/baseline.json --config /tmp/config.json
cat /tmp/gh_output   # contains report<<EOF ... / failed=false / summary=...
```

To exercise history writing without touching the committed file:

```bash
python3 .github/scripts/contract_size.py history \
  --sizes /tmp/current.json \
  --history-file /tmp/history.json \
  --commit "$(git rev-parse HEAD)" \
  --ref refs/heads/main --ref-name main --event push \
  --max-entries 3
```
