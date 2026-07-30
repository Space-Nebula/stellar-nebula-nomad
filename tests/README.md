# Test Organization

Unit tests live inline next to the code (`src/*.rs`, `#[cfg(test)] mod tests`).
Everything under `tests/` is an **integration test** — it goes through the
deployed contract client, not bare module functions.

Integration tests are grouped by module/domain into subdirectories. Each file
is still registered as its own `[[test]]` target in `Cargo.toml` with an
explicit `path`, so existing invocations like
`cargo test --test test_disaster_recovery` or `cargo test test_event_scheduler`
keep working unchanged.

| Directory      | Contents                                                        |
|----------------|------------------------------------------------------------------|
| `fuzz/`        | Fuzz / property-based tests (`fuzz*.rs`)                          |
| `economy/`     | Staking, yield, DEX, gas sponsorship, batching, fractional resources |
| `governance/`  | DAO, RBAC, emergency controls                                    |
| `gameplay/`    | Ships, seasons, gifting, exploration, difficulty scaling          |
| `infra/`       | Storage, snapshots, disaster recovery, oracles, event scheduling, memory leaks |
| `misc/`        | Cross-cutting / multi-issue regression suites                    |
| `fixtures/`    | Shared test data builders                                         |
| `helpers/`     | Shared test setup helpers                                         |
| `chaos/`       | Chaos-engineering scenario configs (not Rust tests)               |
| `load/`        | k6 load-test scripts (not Rust tests)                             |

To run one group: `cargo test --test 'test_*' -- --test-threads=1` won't
filter by directory (Cargo test names are flat), so use the file's `name` or
run everything with `cargo test --all`.
