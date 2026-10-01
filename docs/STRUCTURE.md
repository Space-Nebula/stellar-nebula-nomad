# Nebula Nomad Project Structure

## Directory Tree

```
stellar-nebula-nomad/
├── README.md                          # Project overview & getting started
├── LICENSE                            # MIT License
├── Cargo.toml                         # Rust package manifest & dependencies
├── .gitignore                         # Git ignore rules
│
├── src/                               # Smart contract source code
│   ├── lib.rs                         # Main contract entry point & initialization
│   ├── nebula_explorer.rs             # Nebula scanning & procedural generation
│   ├── resource_minter.rs             # Resource NFT minting logic
│   └── ship_registry.rs               # Ship NFT registration & upgrades
│
├── tests/                             # Integration & unit tests
│   ├── integration_tests.rs           # Contract integration tests
│   └── (future) sim_tests.rs          # Simulation tests for game logic
│
├── scripts/                           # Deployment & utility scripts
│   ├── deploy.sh                      # Soroban CLI deployment script
│   ├── test.sh                        # Test runner script
│   └── (future) migrate.sh            # Contract migration utilities
│
├── docs/                              # Documentation & specifications
│   ├── ABI.md                         # Contract ABI specification
│   ├── ARCHITECTURE.md                # System design & data flow (optional)
│   ├── DEPLOYMENT.md                  # Production deployment guide (optional)
│   └── (future) CONTRACT_SPECS.md     # Detailed contract specifications
│
├── examples/                          # Sample code & invocations
│   ├── scan_example.rs                # Example: Nebula scanning
│   └── (future) game_flow.rs          # Example: Complete game flow
│
├── .github/                           # GitHub configuration
│   └── workflows/
│       └── test.yml                   # CI/CD pipeline (test & build)
│
└── (future) benches/                  # Performance benchmarks
    └── contract_bench.rs              # Contract performance tests
```

## File Descriptions

### Root Level

| File                 | Purpose                                                         |
| -------------------- | --------------------------------------------------------------- |
| `README.md`          | Primary documentation with getting started & development guides |
| `LICENSE`            | MIT license text                                                |
| `Cargo.toml`         | Project manifest, Soroban SDK dependencies, build config        |
| `.gitignore`         | Git ignore patterns for Rust/WASM builds                        |
| `.github/workflows/` | Automated CI/CD on push & pull requests                         |

### src/ - Smart Contracts

| Module               | Responsibility                                         |
| -------------------- | ------------------------------------------------------ |
| `lib.rs`             | Contract initialization & main public interface        |
| `nebula_explorer.rs` | Ledger-seeded procedural generation for nebula regions |
| `resource_minter.rs` | Fungible resource token creation & tracking            |
| `ship_registry.rs`   | NFT registration, upgrades, and metadata               |

### tests/ - Test Suite

Includes unit tests and integration tests that verify:

- Nebula scan generation consistency
- Resource minting validation
- Ship registration & upgrade logic
- Cross-contract interactions

### scripts/ - DevOps & Automation

- `deploy.sh`: Automates building, optimizing, and deploying to Soroban networks
- `test.sh`: Runs full test suite with proper error handling

### docs/ - Specifications

- `ABI.md`: Complete contract interface, function signatures, and data types
- Additional docs for architecture, deployment procedures, and contract specs (future)

### examples/ - Reference Code

Demonstrates calling contracts from client applications and game flows.

---

## Build Artifacts (Generated)

These are automatically created and not committed to git:

```
target/
├── debug/                             # Debug builds
├── release/                           # Release builds
└── wasm32v1-none/            # WASM builds for Soroban
    └── release/
        └── stellar_nebula_nomad.wasm  # Compiled contract binary

Cargo.lock                             # Dependency lock file (committed)
```

---

## Future Expansion

As Nebula Nomad grows, consider adding:

```
├── benches/                           # Performance benchmarks
├── migrations/                        # Contract upgrade migrations
├── sdk/                               # TypeScript/JavaScript SDK for dApps
├── ui/                                # Web frontend (React/Vue)
├── cli/                               # Custom CLI tool
├── audit/                             # Security audit reports
└── whitepaper/                        # Technical design document
```

---

## Development Workflow

```
1. Clone repo
2. Create feature branch (src/ or tests/)
3. Run: cargo test
4. Format: cargo fmt
5. Lint: cargo clippy
6. Commit to branch
7. Push & open PR
8. CI/CD runs tests automatically (.github/workflows/)
9. Maintainers review
10. Merge to main
```

---

## Total Lines of Code by Component

| Component            | Approx LOC | Status               |
| -------------------- | ---------- | -------------------- |
| lib.rs               | 50         | Core interface       |
| nebula_explorer.rs   | 40         | Procedural gen       |
| resource_minter.rs   | 35         | Resource minting     |
| ship_registry.rs     | 45         | Ship management      |
| integration_tests.rs | 40         | Testing              |
| ABI.md               | 100        | Documentation        |
| README.md            | 600+       | Primary docs         |
| **Total**            | **~900**   | **Production-ready** |
