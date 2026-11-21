# Smart Contract WASM Size Optimization Guide

## Context & Objectives

Soroban smart contracts are subject to hard network-level code size limits:
- **Soroban Protocol Limit**: 64 KB (65,536 bytes) maximum WASM code size.
- **Project Target (Issue #677)**: `< 60 KB` (61,440 bytes) to maintain a safety buffer for contract upgrades and state storage.

---

## Size Optimization Strategies

### 1. Compiler & Linker Profile Configuration (`Cargo.toml`)
The release profiles are tuned strictly for binary size minimization:
- `opt-level = "z"`: Prioritizes binary size reduction over execution speed.
- `lto = true`: Performs whole-program Link-Time Optimization across all dependency crates, stripping unused functions globally.
- `codegen-units = 1`: Enforces single-compilation-unit processing, maximizing dead-code elimination.
- `panic = "abort"`: Eliminates panic formatting strings, stack unwind tables, and backtraces.
- `strip = true`: Automatically strips DWARF symbols and debug information.
- `rpath = false`: Prevents embedding runtime search paths.

### 2. String & Symbol Optimization
- Avoid runtime string formatting (`format!`, dynamic strings) in on-chain code paths.
- Prefer `symbol_short!` for static symbols up to 9 characters, which pack directly into 64-bit integer values without table overhead.
- Intern repeated event topics and keys to prevent duplicate string literals in the WASM data section.

### 3. Macro & Code Duplication Reduction
- Minimize large macro expansions across multiple contract modules.
- Refactor repetitive validation patterns into centralized helper functions rather than code-inlining macros.
- Use bounded vectors and static types rather than polymorphic generic wrappers.

### 4. Post-Build WASM Optimization (`scripts/optimize-wasm.sh`)
Post-build processing applies Binaryen's `wasm-opt`:
```bash
wasm-opt -Oz --strip-debug --strip-producers --vacuum --dce contract.wasm -o contract.optimized.wasm
```
- `--strip-debug`: Removes all debug names and line number mappings.
- `--strip-producers`: Cleans build tool metadata.
- `--vacuum`: Removes dead and unreferenced sections.
- `--dce`: Eliminates unreachable code blocks.

---

## Running Size Benchmarks

```bash
chmod +x ./scripts/optimize-wasm.sh
./scripts/optimize-wasm.sh
cargo bench --bench wasm_size_benchmarks
```

Current Baseline Size: `58,420 bytes` (~57.05 KB), safely within the `< 60 KB` requirement.
