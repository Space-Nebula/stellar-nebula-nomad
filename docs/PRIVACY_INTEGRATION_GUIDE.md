# Privacy Stats Integration Guide

## Quick Start

This guide shows how to integrate privacy-preserving stats into your Nebula Nomad application.

## Prerequisites

- Soroban SDK 22.0+
- Player addresses authenticated
- Basic understanding of Soroban contracts

## Step-by-Step Integration

### 1. Enable Privacy for a Player

Before using any privacy features, players must opt in:

```rust
use stellar_nebula_nomad::{opt_in_privacy, is_opted_in_privacy};

// Check if player has opted in
if !is_opted_in_privacy(env.clone(), player.clone()) {
    // Opt in to privacy features
    opt_in_privacy(env.clone(), player.clone())?;
}
```

**When to call:**

- During player onboarding
- When player enables privacy in settings
- Before first private stat commitment

### 2. Commit Private Stats

#### Single Stat Commitment

```rust
use stellar_nebula_nomad::{commit_private_stat};
use soroban_sdk::symbol_short;

// Commit a player's score without revealing it
let score = 1000i128;
let commitment_hash = commit_private_stat(
    env.clone(),
    player.clone(),
    symbol_short!("score"),
    score
)?;

// Store commitment_hash for later verification
// The actual score value is NOT stored on-chain
```

#### Batch Commitment (Recommended)

For multiple stats, use batch operations for better gas efficiency:

```rust
use stellar_nebula_nomad::{batch_commit_stats};
use soroban_sdk::{symbol_short, Vec};

let mut stat_types = Vec::new(&env);
stat_types.push_back(symbol_short!("score"));
stat_types.push_back(symbol_short!("kills"));
stat_types.push_back(symbol_short!("deaths"));

let mut values = Vec::new(&env);
values.push_back(1000i128);  // score
values.push_back(50i128);    // kills
values.push_back(10i128);    // deaths

let commitments = batch_commit_stats(
    env.clone(),
    player.clone(),
    stat_types,
    values
)?;

// commitments is a Vec<BytesN<32>> of commitment hashes
```

### 3. Verify Commitments

To verify a commitment without revealing the underlying value:

```rust
use stellar_nebula_nomad::{verify_private_stat};
use soroban_sdk::BytesN;

// Generate proof off-chain (see Proof Generation section)
let proof: BytesN<64> = generate_proof_offchain(value);

// Verify on-chain
let is_valid = verify_private_stat(
    env.clone(),
    commitment_hash,
    proof
)?;

if is_valid {
    // Commitment is valid, accept it
    process_valid_commitment(player, commitment_hash);
}
```

### 4. Query Commitments

Retrieve stored commitments:

```rust
use stellar_nebula_nomad::{get_commitment, get_commitment_count};

// Get specific commitment
let commitment = get_commitment(
    env.clone(),
    player.clone(),
    symbol_short!("score")
)?;

println!("Player: {:?}", commitment.player);
println!("Stat type: {:?}", commitment.stat_type);
println!("Hash: {:?}", commitment.commitment_hash);
println!("Timestamp: {}", commitment.timestamp);

// Get total commitments in system
let total = get_commitment_count(env.clone());
println!("Total commitments: {}", total);
```

### 5. Anonymous Aggregate Statistics (Differential Privacy)

Commitments hide one player's value. For game-wide analytics (average score,
total resources mined, ...) use anonymous aggregates instead: players add a
bounded value to a per-epoch total, and only a noisy, differentially private
report of that total is ever published.

All functions below live in `stellar_nebula_nomad::privacy_stats` and are
re-exported from the crate root. They are module functions (call them from your
own contract code or tests inside the contract context), not contract entry
points.

#### 5.1 Configure a stat (admin, once per stat)

```rust
use stellar_nebula_nomad::{configure_private_aggregate, initialize_privacy_stats};

initialize_privacy_stats(&env, admin.clone())?; // one-shot
configure_private_aggregate(
    &env,
    admin.clone(),
    symbol_short!("score"),
    1_000, // max_contribution: values are clamped to [0, 1000]
    10,    // noise_multiplier b: each released value is (1/b)-DP
)?;
```

| Parameter | Range | Meaning |
| --- | --- | --- |
| `max_contribution` | `1..=1_000_000_000` | Clamp bound for one player's value. It is the sensitivity of the sum. |
| `noise_multiplier` | `1..=1_000` | Noise scale `b`. Bigger means more noise and more privacy. |

Up to 32 stat types can be configured. Re-configuring a stat takes effect
immediately. The report for an epoch always uses the largest clamp bound seen
in that epoch, so lowering the bound mid-epoch never weakens the guarantee.

#### 5.2 Contribute (player)

```rust
use stellar_nebula_nomad::contribute_anonymous_stat;

// Player must be opted in (see step 1).
let epoch = contribute_anonymous_stat(&env, player.clone(), symbol_short!("score"), 1_250)?;
// 1_250 is clamped to 1_000 before it is added to the total.
```

What is stored:

- `PrivacyAggregate(stat, epoch)`: `{ contributors, sum, max_bound }`. Only totals, no addresses.
- `PrivacyNullifier(sha256(player_xdr || stat_xdr || epoch_xdr))`: blocks a
  second contribution from the same player to the same stat in the same epoch.
  The key is a hash, not the address.
- Event `("privacy", "contrib")` with data `(stat, epoch)` only. No player, no value.

Epochs are `ledger_timestamp / PRIVACY_EPOCH_SECS` (86 400 s, one day).

#### 5.3 Publish and read the report (anyone)

```rust
use stellar_nebula_nomad::{get_privacy_report, publish_privacy_report};

// Only for closed epochs (epoch < current_privacy_epoch(&env)), and only once.
let report = publish_privacy_report(&env, symbol_short!("score"), epoch)?;
let same = get_privacy_report(&env, symbol_short!("score"), epoch)?;
```

`PrivacyReport` fields:

| Field | Meaning |
| --- | --- |
| `noisy_contributors` | Contributor count plus discrete Laplace noise, clamped at 0 |
| `noisy_sum` | Sum plus discrete Laplace noise scaled to `max_contribution`, clamped at 0. `0` when suppressed |
| `suppressed` | `true` when `noisy_contributors < MIN_REPORT_COHORT` (5). The sum is withheld for tiny cohorts |
| `max_contribution`, `noise_multiplier` | Parameters actually used |
| `epsilon_milli` | Upper bound on the total privacy loss of this report, in thousandths (`ceil(2000 / b)`) |
| `published_at` | Ledger timestamp of publication |

The noisy report is stored when it is first published. Reading it again returns
the same numbers, so callers cannot average the noise away by querying repeatedly.

#### 5.4 How the noise works

- Noise is the discrete Laplace distribution: `P(x) ∝ q^|x|` with
  `q = s / (s + 1)`. It is sampled as the difference of two geometric draws from
  `env.prng()`. The sampler is an inverse-CDF binary search in 64.64 fixed
  point, because the Soroban VM rejects floating-point code.
- Count: sensitivity 1, scale `s = b`, so `ε ≤ ln(1 + 1/b) ≤ 1/b`.
- Sum: sensitivity `max_contribution = C`, scale `s = b·C`, so `ε ≤ C·ln(1 + 1/(bC)) ≤ 1/b`.
- One report releases both, so the total is `ε ≤ 2/b`. This bound is reported in `epsilon_milli`.
- The geometric draw is truncated at `64·s`. The probability of exceeding that is `< e^-64`.

Example: `b = 10` gives `ε ≤ 0.2` per report. Count noise has standard deviation
`sqrt(2·10·11) ≈ 14.8`, and sum noise for `C = 1000` has standard deviation `≈ 14 142`.
Choose `b` so that the noise is small compared with the expected cohort.

#### 5.5 Transparency report

```rust
use stellar_nebula_nomad::get_privacy_transparency_report;

let t = get_privacy_transparency_report(&env);
// t.opted_in_players, t.total_commitments, t.total_contributions,
// t.reports_published, t.configured_stats, t.epoch_secs, t.min_report_cohort
```

The transparency report holds counters and parameters only, never per-player
data. `opted_in_players` counts players who opt in from this release on.
Calling `opt_in_privacy` again does not double count.

#### 5.6 Threat model and limits (read before relying on it)

- **Ledger data is public.** Every transaction's invoker and arguments,
  including the `value` passed to `contribute_anonymous_stat` or
  `commit_private_stat`, are visible on the Stellar ledger. Contract storage is
  readable through RPC, including the raw `PrivacyAggregate` totals. Differential
  privacy protects the published `PrivacyReport` and anything built on it
  (dashboards, indexers, leaderboards that read reports). It does not protect
  against someone replaying raw ledger history.
- **The PRNG is not secret.** `env.prng()` is seeded from consensus data that a
  malicious validator could bias, as the soroban-sdk documentation warns. The
  noise is fine for analytics. Do not use these reports where an adversarial
  validator gains from steering them.
- **Sybil accounts.** The one-contribution-per-epoch rule is per address. One
  person with many addresses contributes many times. Pair this with your
  existing anti-bot checks if that matters for a stat.
- **Nullifiers are pseudonymous, not anonymous.** Anyone who knows a player's
  address can recompute their nullifier and learn that they contributed in an
  epoch, but not the value.
- **Commitment hashes.** `commit_private_stat` hashes `(stat, value, timestamp)`
  without the player, so equal inputs give equal hashes. This is kept unchanged
  so existing commitments still verify.

## Common Integration Patterns

### Pattern 1: Private Leaderboard

```rust
// Player submits score privately
pub fn submit_private_score(
    env: Env,
    player: Address,
    score: i128,
) -> Result<BytesN<32>, PrivacyError> {
    // Ensure player is opted in
    if !is_opted_in_privacy(env.clone(), player.clone()) {
        opt_in_privacy(env.clone(), player.clone())?;
    }

    // Commit score
    let commitment = commit_private_stat(
        env.clone(),
        player.clone(),
        symbol_short!("score"),
        score
    )?;

    // Add to leaderboard (commitment only, not score)
    add_to_leaderboard(&env, player, commitment.clone());

    Ok(commitment)
}

// Verify leaderboard entry
pub fn verify_leaderboard_entry(
    env: Env,
    commitment: BytesN<32>,
    proof: BytesN<64>,
) -> Result<bool, PrivacyError> {
    verify_private_stat(env, commitment, proof)
}
```

### Pattern 2: Achievement Verification

```rust
// Player claims achievement privately
pub fn claim_private_achievement(
    env: Env,
    player: Address,
    achievement_type: Symbol,
    progress: i128,
) -> Result<BytesN<32>, PrivacyError> {
    // Commit achievement progress
    let commitment = commit_private_stat(
        env.clone(),
        player.clone(),
        achievement_type.clone(),
        progress
    )?;

    // Emit event for achievement system
    env.events().publish(
        (symbol_short!("achieve"), symbol_short!("claim")),
        (player, achievement_type, commitment.clone())
    );

    Ok(commitment)
}
```

### Pattern 3: Tournament Entry

```rust
// Player enters tournament with private stats
pub fn enter_tournament_private(
    env: Env,
    player: Address,
    tournament_id: u64,
    stats: Vec<(Symbol, i128)>,
) -> Result<Vec<BytesN<32>>, PrivacyError> {
    // Ensure opted in
    if !is_opted_in_privacy(env.clone(), player.clone()) {
        opt_in_privacy(env.clone(), player.clone())?;
    }

    // Extract stat types and values
    let mut types = Vec::new(&env);
    let mut values = Vec::new(&env);

    for i in 0..stats.len() {
        let (stat_type, value) = stats.get(i).unwrap();
        types.push_back(stat_type);
        values.push_back(value);
    }

    // Batch commit all stats
    let commitments = batch_commit_stats(
        env.clone(),
        player.clone(),
        types,
        values
    )?;

    // Register for tournament
    register_tournament_entry(&env, tournament_id, player, commitments.clone());

    Ok(commitments)
}
```

## Proof Generation

### Current Implementation (Simple)

The current implementation uses a simple proof format for demonstration:

```rust
// Off-chain proof generation (simplified)
fn generate_simple_proof(
    env: &Env,
    commitment: BytesN<32>,
) -> BytesN<64> {
    let mut proof_bytes = [0u8; 64];

    // First 32 bytes: commitment hash
    for i in 0..32 {
        proof_bytes[i] = commitment.get(i).unwrap();
    }

    // Last 32 bytes: additional proof data
    // In production, this would be a proper ZK proof
    for i in 32..64 {
        proof_bytes[i] = (i % 256) as u8;
    }

    BytesN::from_array(env, &proof_bytes)
}
```

### Production Implementation (Future)

For production, integrate proper ZK-SNARK libraries:

```rust
// Example with hypothetical ZK library
use zk_snarks::{Groth16, Proof, ProvingKey};

fn generate_zk_proof(
    value: i128,
    commitment: BytesN<32>,
    proving_key: &ProvingKey,
) -> BytesN<64> {
    // Generate ZK proof that value matches commitment
    let proof = Groth16::prove(
        proving_key,
        &[value],
        &commitment,
    );

    // Serialize proof to 64 bytes
    proof.to_bytes()
}
```

## Error Handling

Always handle errors appropriately:

```rust
use stellar_nebula_nomad::PrivacyError;

match commit_private_stat(env.clone(), player.clone(), stat_type, value) {
    Ok(commitment) => {
        // Success - process commitment
        handle_commitment(commitment);
    }
    Err(PrivacyError::NotOptedIn) => {
        // Player needs to opt in first
        prompt_opt_in(player);
    }
    Err(PrivacyError::CommitmentExists) => {
        // Stat already committed
        handle_duplicate();
    }
    Err(PrivacyError::BurstLimitExceeded) => {
        // Too many operations in one tx
        retry_later();
    }
    Err(e) => {
        // Other errors
        log_error(e);
    }
}
```

## Best Practices

### 1. Batch Operations

✅ **Do:** Use batch operations for multiple stats

```rust
batch_commit_stats(env, player, types, values)?;
```

❌ **Don't:** Make individual commits in a loop

```rust
for (stat_type, value) in stats {
    commit_private_stat(env.clone(), player.clone(), stat_type, value)?;
}
```

### 2. Opt-In Checks

✅ **Do:** Check opt-in status before operations

```rust
if !is_opted_in_privacy(env.clone(), player.clone()) {
    return Err(PrivacyError::NotOptedIn);
}
```

❌ **Don't:** Assume player is opted in

```rust
commit_private_stat(env, player, stat_type, value)?; // May fail
```

### 3. Commitment Storage

✅ **Do:** Store commitment hashes off-chain for quick access

```rust
// Store in your database
db.store_commitment(player_id, stat_type, commitment_hash);
```

❌ **Don't:** Query on-chain for every verification

```rust
// Expensive on-chain query
let commitment = get_commitment(env, player, stat_type)?;
```

### 4. Burst Management

✅ **Do:** Reset burst counter at transaction start

```rust
reset_privacy_burst_counter(env.clone());
batch_commit_stats(env, player, types, values)?;
```

❌ **Don't:** Exceed 10 commitments per transaction

```rust
// Will fail if types.len() > 10
batch_commit_stats(env, player, types, values)?;
```

## Testing Your Integration

### Unit Tests

```rust
#[test]
fn test_my_integration() {
    let env = Env::default();
    env.mock_all_auths();
    let player = Address::generate(&env);

    // Test opt-in
    opt_in_privacy(env.clone(), player.clone()).unwrap();
    assert!(is_opted_in_privacy(env.clone(), player.clone()));

    // Test commitment
    let commitment = commit_private_stat(
        env.clone(),
        player.clone(),
        symbol_short!("score"),
        1000i128
    ).unwrap();

    assert_eq!(commitment.len(), 32);
}
```

### Integration Tests

```rust
#[test]
fn test_leaderboard_integration() {
    let env = Env::default();
    env.mock_all_auths();

    // Create multiple players
    let players = vec![
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];

    // All opt in and commit scores
    for player in players.iter() {
        opt_in_privacy(env.clone(), player.clone()).unwrap();
        commit_private_stat(
            env.clone(),
            player.clone(),
            symbol_short!("score"),
            1000i128
        ).unwrap();
    }

    // Verify all commitments
    assert_eq!(get_commitment_count(env.clone()), 3);
}
```

## Performance Considerations

### Gas Costs

| Operation         | Estimated Gas | Notes                          |
| ----------------- | ------------- | ------------------------------ |
| Opt-in            | Low           | One-time per player            |
| Single commit     | Medium        | 2 storage writes + hash        |
| Batch commit (10) | High          | But cheaper than 10 individual |
| Verify            | Very Low      | Pure function                  |
| Query             | Low           | 1 storage read                 |

### Optimization Tips

1. **Batch when possible**: Commit multiple stats together
2. **Cache commitments**: Store hashes off-chain
3. **Lazy opt-in**: Only opt in when needed
4. **Proof generation**: Do off-chain to save gas

## Migration Guide

### Adding Privacy to Existing Stats

If you have existing stat tracking:

```rust
// Old way (public stats)
pub fn update_score(env: Env, player: Address, score: i128) {
    env.storage().persistent().set(&player, &score);
}

// New way (with privacy option)
pub fn update_score_private(
    env: Env,
    player: Address,
    score: i128,
    use_privacy: bool,
) -> Result<(), PrivacyError> {
    if use_privacy {
        // Use privacy commitment
        if !is_opted_in_privacy(env.clone(), player.clone()) {
            opt_in_privacy(env.clone(), player.clone())?;
        }
        commit_private_stat(env, player, symbol_short!("score"), score)?;
    } else {
        // Traditional storage
        env.storage().persistent().set(&player, &score);
    }
    Ok(())
}
```

## Troubleshooting

### Common Issues

**Issue:** `NotOptedIn` error

- **Solution:** Call `opt_in_privacy()` before committing stats

**Issue:** `CommitmentExists` error

- **Solution:** Each stat type can only be committed once per player

**Issue:** `BurstLimitExceeded` error

- **Solution:** Reduce batch size to 10 or fewer, or reset burst counter

**Issue:** `StatNotConfigured` error

- **Solution:** The privacy admin must call `configure_private_aggregate()` for that stat type

**Issue:** `AlreadyContributed` error

- **Solution:** One anonymous contribution per player, stat and epoch. Retry in the next epoch

**Issue:** `EpochNotClosed` / `ReportAlreadyPublished` error

- **Solution:** Reports can be published only for past epochs, and only once. Read existing ones with `get_privacy_report()`

**Issue:** `InvalidProof` error

- **Solution:** Ensure proof generation matches commitment format

## Support

For more information:

- See `docs/PRIVACY_STATS.md` for detailed documentation
- Check `examples/privacy_stats_example.rs` for working examples
- Review `tests/test_privacy_stats.rs` for test patterns

## Next Steps

1. Integrate opt-in flow in your UI
2. Add privacy option to stat submission
3. Implement proof generation (off-chain)
4. Test with multiple players
5. Monitor gas costs and optimize
6. Plan for ZK-SNARK upgrade

## Future Features

Available now:

- Aggregation with differential privacy (`contribute_anonymous_stat`, `publish_privacy_report`)
- Transparency report (`get_privacy_transparency_report`)
- Selective disclosure (`grant_disclosure`, `revoke_disclosure`, `has_disclosure`)

Coming soon:

- Range proofs (prove value is within range)
- Cross-contract privacy (share commitments between contracts)
- Contract entry points for the aggregate API
