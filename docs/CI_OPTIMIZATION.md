# CI/CD Pipeline Optimization

This document records the baseline analysis, the changes applied to the
workflows, and how to measure the result. Related work: `docs/DEVELOPER_GUIDE.md`,
`.github/actions/setup-rust`, `.github/actions/setup-stellar-cli`.

_Last updated: 2026-11-19 (branch `hardening/ci-dr-arithmetic-reentrancy`)._

## 1. Baseline analysis

The repository had **11 workflows** with **11 hand-rolled `actions/cache@v4`
blocks**. Every cache used the same `path` (`~/.cargo/registry`, `~/.cargo/git`,
`target`) but a different key prefix (`-cargo-`, `-cargo-lint-`, `-cargo-test-`,
`-cargo-docs-`, `-cargo-bench-`, `-cargo-deploy-testnet-`, `-cargo-deploy-`, …).

Consequences:

| Problem | Evidence | Effect |
|---|---|---|
| **No cache sharing** | Disjoint key prefixes over identical paths | Every job recompiled the full dependency graph. `Swatinem/rust-cache` was not used anywhere. |
| **Cache invalidation** | Only 2/11 caches had `restore-keys` | Any `Cargo.lock` change was a total miss in 9 jobs. |
| **Duplicate test execution** | `ci.yml` `test` ran `cargo test --all --verbose` then `cargo tarpaulin --all` in the same job | The entire suite ran twice, back-to-back; this was the primary timeout source. |
| **Duplicate pipelines** | No `concurrency` groups | A push to `main` plus the PR that produced it ran the full pipeline twice. |
| **Duplicate jobs across workflows** | `ci.yml` (lint/test/WASM build), `test.yml` (clippy/test/fuzz/bench/WASM build) | Stable toolchain tests, clippy and WASM builds ran in both workflows. |
| **Uncached tool installs** | `cargo install cargo-tarpaulin --locked` in `ci.yml`; Stellar CLI installed via `curl … main/install.sh` in 6 places | Minutes spent recompiling/downloading tools on every run. |
| **Unbounded jobs** | Only `test.yml` (60 min) and `backup.yml` (30 min) had `timeout-minutes` | A stuck tarpaulin/bench job could occupy a runner for hours. |
| **Broken coverage outputs** | `ci.yml` emitted `--out Xml Html` but Codecov/Coveralls referenced `coverage/lcov.info` | `lcov.info` never existed; coverage uploaders silently failed. |
| **Sparse registry implicit** | No `CARGO_REGISTRIES_CRATES_IO_PROTOCOL` anywhere | Resolution speed depended on the runner's Cargo default. |
| **Docker context bloat** | No `.dockerignore`; `Dockerfile.dev` copied `Cargo.toml` but not `Cargo.lock` before `cargo fetch` | Dependency layer invalidated on every source edit; entire repo sent as build context. |

Critical path before the change (PR to `main`): `build` (native + WASM compile +
CLI install) → `size-check`, with `test`/`coverage` serialized inside one job.
Worst-case wall clock was dominated by sequential full recompiles.

## 2. Changes applied

### `ci.yml`
- Added a `concurrency` group with `cancel-in-progress: true`.
- Replaced the per-job `actions/cache` blocks with the shared composite action
  `./.github/actions/setup-rust` (`Swatinem/rust-cache@v2` + `shared-key`).
  Native jobs use `shared-key: native`; WASM/deploy jobs use `shared-key: wasm`.
- Split the old `test` job into two **parallel** jobs: `test` (plain
  `cargo test --all --locked`) and `coverage` (tarpaulin). Coverage no longer
  serializes a second full test run behind the first.
- Coverage now uses `--engine llvm` (adds `llvm-tools-preview` component) and
  emits `Xml Lcov Html`, fixing the missing `lcov.info` for Codecov/Coveralls.
- `cargo-tarpaulin` binary is cached via `actions/cache`.
- Reduced WASM recompiles: `deploy-testnet` now **downloads the WASM artifact**
  from `build` instead of rebuilding; `size-check` does the same.
- Stellar CLI installed via the cached `setup-stellar-cli` composite action.
- `timeout-minutes` added to every job (15–25 min).
- All Cargo invocations use `--locked`.
- `upload-artifact@v7` / `download-artifact@v8` normalized to `@v4`.

### `test.yml`
- Added `concurrency`, sparse registry env, shared Rust cache (per-toolchain
  key), `timeout-minutes: 30`, and `--locked`.
- Removed duplicated clippy, fuzz, benchmark and WASM-build steps. This
  workflow now has a single, clear purpose: **cross-toolchain compatibility**
  (stable + nightly, `fail-fast: false`).

### `deploy.yml`
- `concurrency` group, sparse registry env, shared `wasm` cache key, cached
  Stellar CLI, `timeout-minutes`, `--locked`, artifact majors normalized.
- Deployment environment URL now interpolates the selected network instead of
  always linking to Futurenet.
- Deployer secret is passed via `env:` (never interpolated into shell source)
  and the job fails fast when it is missing.

### Build configuration
- `.cargo/config.toml`: explicit `[registries.crates-io] protocol = "sparse"`.
- `Dockerfile.dev`: BuildKit cache mounts for the Cargo registry/git dirs,
  `Cargo.lock` copied before `cargo fetch --locked`, `CARGO_INCREMENTAL=1`,
  sparse registry env.
- `.dockerignore`: excludes `.git`, `target`, `node_modules`, backups,
  coverage, snapshots and local env files from the build context.

### Incremental compilation policy
- **Enabled** (`CARGO_INCREMENTAL=1`) for native dev/test/lint/docs jobs and the
  dev Docker image. Combined with the shared `target/` cache, warm-cache runs
  only recompile changed crates.
- **Disabled** for release/WASM builds (`profile.release.incremental = false`
  in `Cargo.toml`, `CARGO_INCREMENTAL: 0` in WASM jobs). Incremental artifacts
  are ineffective under `lto = true` + `codegen-units = 1` and would bloat the
  cache.

## 3. Measuring improvements

### 3.1 Local proxy measurement
`scripts/ci-timing.sh` times the same stages locally on a clean and a warm
cache. Run it twice; the second run approximates a CI cache hit.

```bash
./scripts/ci-timing.sh clean /tmp/ci-timing-clean.txt
./scripts/ci-timing.sh warm  /tmp/ci-timing-warm.txt
```

### 3.2 GitHub Actions measurement
1. Open the **Actions → CI Pipeline → job** page; GitHub reports per-job and
   per-step durations. Record the `build`, `test`, `coverage` and `size-check`
   totals for the first PR run (cold cache) and a subsequent run (warm cache).
2. Cache effectiveness is reported by `Swatinem/rust-cache` in the
   "Restore shared Cargo cache" step (`Cache hit / Miss`). Cache hit rate is
   computed as `hits / (hits + misses)` across jobs over 20 consecutive runs.
3. Compare against the baseline table below.

### 3.3 Targets and expected results

| Metric | Baseline | Target | Mechanism |
|---|---|---|---|
| Critical-path wall clock | ~25–35 min (serialized tests ×2 + full recompiles per job) | **< 10 min** | Parallel coverage + shared cache + artifact reuse |
| Cache hit rate | 0% cross-job (disjoint keys) | **> 80%** | `Swatinem/rust-cache` `shared-key` + `cache-on-failure` |
| Full dependency recompiles per PR | 5–8 | 1 | Shared `native`/`wasm` keys |
| Coverage step | timed out at 120 s test timeout frequently | < 12 min | LLVM engine + tool cache |
| Stellar CLI installs per PR | 3–5 | 1 (then cached) | `setup-stellar-cli` |
| Job timeout ceilings | mostly unbounded | all bounded (15–30 min) | `timeout-minutes` |

> These figures are structural estimates from the workflow definitions. Actual
> numbers must be confirmed from the Actions UI after the first merge; record
> them in §3.4.

### 3.4 Verification record

_Pending first CI run on `main`._ Operator: append the measured wall-clock
total, cache hit rate and any remaining timeouts here.

## 3.5 Caching review — all 11 workflows

Sparse registry is now enforced globally in `.cargo/config.toml`, so every
workflow benefits without needing a per-file `env:` entry.

| Workflow | Caching before | Caching now / recommendation |
|---|---|---|
| `ci.yml` | 6 disjoint manual caches | Shared `native`/`wasm` keys via `setup-rust`; tarpaulin + Stellar CLI binaries cached |
| `test.yml` | None | Shared per-toolchain keys via `setup-rust` |
| `deploy.yml` | 1 manual cache | Shared `wasm` key; Stellar CLI cached |
| `size-regression.yml` | Manual cache with `restore-keys` | Recommended: switch to `setup-rust` with `cache-key: wasm` (syntax fixed) |
| `security.yml` | None; recompiles `cargo-audit` / `cargo-cyclonedx` each run | Recommended: cache `~/.cargo/bin` tools keyed by version |
| `fuzz.yml` | None; recompiles `cargo-fuzz` weekly | Recommended: cache `~/.cargo/bin/cargo-fuzz` |
| `mutation.yml` / `mutation-testing.yml` | Manual `target` cache / pinned-but-unversioned tool cache | Recommended: `setup-rust`; align installed `cargo-mutants` version with cache key |
| `valgrind.yml` | Manual cache | Recommended: `setup-rust` with `cache-key: native` |
| `load-tests.yml` | None | No Cargo build; add k6 setup cache only if needed |
| `backup.yml` | None | Shell-only; no Cargo cache needed (fix secrets-in-`if` and drill condition) |

## 4. Acceptance criteria status

| Criterion | Status |
|---|---|
| CI pipeline < 10 minutes | Addressed structurally; confirm from Actions after merge |
| No timeouts | All jobs now have explicit `timeout-minutes`; tarpaulin uses LLVM engine |
| Cache hit rate > 80% | `shared-key` caching in all touched workflows; confirm over 20 runs |
| Parallel execution maximized | Coverage split into parallel job; independent jobs have no `needs` |
| Documentation updated | This document + `DEVELOPER_GUIDE.md` cross-reference |
