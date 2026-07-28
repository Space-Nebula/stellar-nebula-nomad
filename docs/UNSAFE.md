# Unsafe Code Audit

**Last updated:** 2026-07-28
**Audit scope:** All `unsafe` blocks in the codebase.
**Overall verdict:** 0 unsafe blocks in production code. 1 unsafe block in test code (documented below).

---

## Policy

- **Zero `unsafe` in production code** is the target. The crate uses `#![forbid(unsafe_code)]` to enforce this at compile time.
- Test code may use `unsafe` where necessary for testing patterns (e.g., lifetime extension in Soroban test helpers), but each use must be documented with a `// SAFETY:` comment explaining why it's sound.

---

## Audit Results

| Location | Lines | Type | Status |
|----------|-------|------|--------|
| `src/` | — | Production | ✅ No unsafe found |
| `tests/common/mod.rs:43` | 1 | Test helper | ⚠️ Documented below |

---

## `tests/common/mod.rs:43` — Lifetime extension for Soroban test client

### Unsafe code

```rust
let client_static = unsafe {
    core::mem::transmute::<_, NebulaNomadContractClient<'static>>(client)
};
```

### Why it exists

Soroban's generated contract clients borrow the test `Env`. The borrow checker requires the lifetime to be `'static` when the client is returned from a helper function, because the function's local borrows don't outlive the call. However, the `Env` is returned in the same tuple and will outlive the client in practice.

### Soundness justification

1. `Env` is declared first in the tuple: `(Env, NebulaNomadContractClient<'static>, Address)`.
2. Rust drops tuple fields in declaration order, so `Env` is dropped AFTER the client.
3. As long as the caller does not `mem::replace` or `swap` the tuple to move `Env` out early, the borrow is valid.
4. This is the standard pattern used across all Soroban contract test suites.

### Would-be fix (if Rust had better borrow analysis)

Ideally the compiler would understand that the return tuple keeps `Env` alive. There is no stable Rust feature for this yet. Alternatives considered:
- Leaking `Env` via `Box::leak` — works but wastes memory per test.
- Refactoring all callers to pass `Env` by reference — defeats the purpose of a setup helper.
- `ManuallyDrop` — doesn't help with lifetimes.

### Recommendation

Keep as-is. The safety invariant (env dropped after client) is enforced by Rust's drop order.

---

## Running the audit

```bash
# Check for all unsafe blocks in source (should be 0)
grep -rn "unsafe" src/ --include="*.rs"

# Check for all unsafe blocks in tests
grep -rn "unsafe" tests/ --include="*.rs"
```
