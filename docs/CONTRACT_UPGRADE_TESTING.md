# Contract Upgrade Testing Guide

## Overview

This document describes contract upgrade testing for Stellar Nebula Nomad. Upgrade testing ensures that contract code updates maintain data integrity, preserve state, support rollback, and maintain backward compatibility.

## Why Upgrade Testing Matters

Smart contracts are immutable once deployed, but Soroban allows code upgrades through authorized upgrades. Upgrade testing verifies:

1. **Data Integrity** - Player data doesn't corrupt during migration
2. **State Preservation** - Economic invariants are maintained
3. **Backward Compatibility** - Old clients continue working
4. **Rollback Safety** - Emergency rollback doesn't lose data
5. **Migration Paths** - Complex state transformations work correctly

## Running Upgrade Tests

### Quick Test

```bash
cargo test --test upgrade_
```

### Individual Test Suites

```bash
# Data migration scenarios
cargo test --test upgrade_migration

# State preservation verification
cargo test --test upgrade_state_preservation

# Backward compatibility checks
cargo test --test upgrade_backward_compatibility

# Rollback procedures
cargo test --test upgrade_rollback
```

### Full Upgrade Audit

```bash
# Run all upgrade-related tests
cargo test --test upgrade_ -- --nocapture --test-threads=1
```

## Test Suites

### 1. Migration Tests (`tests/upgrades/upgrade_migration.rs`)

**Purpose**: Verify data moves correctly during contract update

**Test Coverage**:

#### `test_player_profile_migration`
- Creates player profile before upgrade
- Updates progress
- Verifies profile exists after upgrade
- Checks progress data integrity

#### `test_ship_nft_state_migration`
- Mints multiple ships pre-upgrade
- Verifies ships accessible post-upgrade
- Confirms NFT ownership preserved

#### `test_resource_balance_migration`
- Establishes player resources
- Verifies balances after upgrade
- Checks resource types available

#### `test_alliance_membership_migration`
- Creates alliance
- Adds members
- Verifies membership preserved

#### `test_stored_upgrade_metadata`
- Tracks upgrade history
- Records version changes
- Maintains upgrade timeline

**Key Assertions**:
- ✓ Player profiles exist post-upgrade
- ✓ NFT registries intact
- ✓ Resource balances match pre-upgrade
- ✓ Alliance memberships valid
- ✓ No data loss

### 2. State Preservation Tests (`tests/upgrades/upgrade_state_preservation.rs`)

**Purpose**: Verify critical invariants survive upgrade

**Test Coverage**:

#### `test_total_player_count_preserved`
- Counts active players pre-upgrade
- Verifies same count post-upgrade
- Checks no players lost or duplicated

#### `test_resource_supply_invariant`
- Tracks total resource supply before upgrade
- Verifies total supply unchanged after upgrade
- Prevents money creation/destruction

#### `test_alliance_treasury_preserved`
- Records alliance treasuries pre-upgrade
- Verifies same totals post-upgrade
- Checks treasury consistency

#### `test_ship_registry_consistency`
- Enumerates all ships pre-upgrade
- Verifies all ships retrievable post-upgrade
- Confirms ship data consistency

#### `test_seasonal_data_preservation`
- Tracks seasonal progress pre-upgrade
- Verifies season data intact post-upgrade
- Checks all seasonal fields

**Critical Invariants Protected**:

```
Total Resources = Σ(player_balances) + treasury + sinks
Players = count(player_profiles) = count(ships) + unequipped_players
Alliances = count(alliance_records) = count(members)
```

### 3. Backward Compatibility Tests (`tests/upgrades/upgrade_backward_compatibility.rs`)

**Purpose**: Ensure old contract calls work with new code

**Test Coverage**:

#### `test_old_ship_mint_path_compatibility`
- Uses pre-upgrade mint_ship signature
- Verifies mint succeeds with new code
- Confirms ship state valid

#### `test_old_resource_query_compatibility`
- Queries resources using old API
- Verifies get_resource_balance works
- Confirms balance types supported

#### `test_old_alliance_creation_path_compatibility`
- Creates alliance using old API
- Verifies found_alliance compatible
- Confirms alliance accessible

#### `test_old_ship_info_query_compatibility`
- Retrieves ship info using old query
- Verifies get_ship_info works
- Confirms ownership valid

#### `test_new_query_additions_dont_break_old`
- New query functions coexist
- Old queries still work
- No field renames or removals

#### `test_parameter_signature_preservation`
- Old function signatures still valid
- Parameter types unchanged
- Return types compatible

**Breaking Changes Prevention**:
- ❌ Function signature changes
- ❌ Parameter reordering
- ❌ Return type changes
- ❌ Field removal from records
- ✓ New optional fields (with defaults)
- ✓ New functions (don't remove old ones)
- ✓ New error types (additive)

### 4. Rollback Tests (`tests/upgrades/upgrade_rollback.rs`)

**Purpose**: Emergency downgrade safety

**Test Coverage**:

#### `test_rollback_preserves_player_data`
- Sets up player state at version N
- Simulates upgrade to N+1
- Simulates rollback to N
- Verifies player data intact

#### `test_rollback_preserves_ship_ownership`
- Mints ships at version N
- Tracks ownership through upgrade
- Verifies ownership after rollback
- Confirms no ownership corruption

#### `test_rollback_idempotency`
- Performs operation
- Checks state
- Rolls back and re-does
- Verifies same final state

#### `test_rollback_alliance_state_consistency`
- Creates alliance structure
- Adds members
- Tests rollback
- Verifies structure intact

#### `test_rollback_resource_balances`
- Sets resource balances
- Performs upgrade
- Rolls back
- Verifies exact balances restored

#### `test_rollback_no_data_corruption`
- Creates complex state
- Tests rollback
- Checks for corruption
- Verifies data integrity

**Rollback Guarantees**:
- ✓ All player data recoverable
- ✓ No orphaned references
- ✓ No data corruption
- ✓ Idempotent rollback
- ✓ Fast recovery path

## Upgrade Process

### Pre-Upgrade

1. **Run all tests**:
```bash
cargo test --test upgrade_
```

2. **Document changes**:
```markdown
## Upgrade v2.1.0 → v2.2.0
- New: quest_system module
- Changed: alliance_treasury calculation
- Removed: deprecated_field (migration in code)
- Migration: quest_data => quests table
```

3. **Plan data migration**:
```rust
fn migrate_v21_to_v22() {
    // 1. Create new structures
    // 2. Migrate existing data
    // 3. Update indexes
    // 4. Verify invariants
}
```

### During Upgrade

```bash
# 1. Deploy new code
stellar contract deploy --source-account $ACCOUNT \
    --rpc-url $RPC_URL \
    target/wasm32-unknown-unknown/release/stellar_nebula_nomad.wasm

# 2. Verify on testnet (wait 1 week minimum)
# - Run full test suite
# - Monitor for issues
# - Check economic data

# 3. Run migration tests
cargo test --test upgrade_migration

# 4. Verify state preservation
cargo test --test upgrade_state_preservation

# 5. Deploy to mainnet after testnet validation
```

### Post-Upgrade

1. **Monitor**:
   - Track player activity
   - Monitor economic metrics
   - Check for data inconsistencies

2. **Verify**:
```bash
# Query production contract
stellar contract invoke --source-account $ACCOUNT \
    --contract-id $CONTRACT_ID \
    -- player_count
```

3. **Rollback Ready**:
```bash
# Keep previous version available
# Have rollback plan documented
# Test rollback procedure
```

## Migration Patterns

### Pattern 1: Simple Field Addition

```rust
// Old version
struct Player {
    id: Address,
    level: u32,
}

// New version (backward compatible)
struct Player {
    id: Address,
    level: u32,
    new_field: String,  // Safe to add with default
}
```

**Migration Test**:
```rust
#[test]
fn test_add_field_migration() {
    // Create player with old version
    let player = client.initialize_profile(&player_addr);
    
    // After upgrade, new field should have default
    let progress = client.get_player_progress(&player_addr, &player);
    assert_eq!(progress, 0);  // Default value
}
```

### Pattern 2: Breaking Data Structure

```rust
// Old version
struct Ship {
    id: u64,
    energy: u32,
}

// New version (must migrate data!)
struct Ship {
    id: u64,
    energy_pool: u64,  // Changed type and name
}

// Migration code
fn migrate_ship_energy(old: OldShip) -> Ship {
    Ship {
        id: old.id,
        energy_pool: (old.energy as u64) * 1000,
    }
}
```

**Migration Test**:
```rust
#[test]
fn test_ship_energy_migration() {
    // Create old-format ship
    let old_ship = client.mint_ship(&player, &"fighter", &Bytes::new(&env));
    let old_energy = client.get_ship_energy(&old_ship.id);
    
    // After upgrade with migration
    let new_energy = client.get_ship_energy(&old_ship.id);
    assert_eq!(new_energy, old_energy * 1000);  // Verify conversion
}
```

### Pattern 3: State Consolidation

```rust
// Old version - multiple tables
// resources[account] = HashMap<ResourceType, Balance>
// ships[account] = HashMap<ShipId, ShipData>

// New version - consolidated
// player_data[account] = {
//   resources: HashMap<ResourceType, Balance>,
//   ships: HashMap<ShipId, ShipData>,
//   metadata: PlayerMetadata,
// }

fn migrate_consolidate() {
    for account in all_accounts {
        let resources = old_resources.get(account);
        let ships = old_ships.get(account);
        new_player_data[account] = {
            resources,
            ships,
            metadata: PlayerMetadata::default(),
        }
    }
}
```

## Common Pitfalls

### ❌ Pitfall 1: Removing Fields

```rust
// WRONG - causes data loss
struct Player {
    id: Address,
    // level: u32,  ← REMOVED (players lose levels!)
}
```

**Solution**: Mark deprecated, migrate to new location:
```rust
struct Player {
    id: Address,
    level: u32,  // Keep for compatibility
    new_level: u32,  // New location
}

fn migrate_level() {
    for player in all_players {
        player.new_level = player.level;  // Transfer data
    }
}
```

### ❌ Pitfall 2: Changing Serialization

```rust
// WRONG - breaks deserialization
#[derive(Serialize, Deserialize)]
struct Alliance {
    name: String,
    treasury: u128,  // Changed from u64
}
```

**Solution**: Support both formats temporarily:
```rust
fn deserialize_alliance_safe(data: &[u8]) -> Alliance {
    match deserialize_v2(data) {
        Ok(alliance) => alliance,
        Err(_) => {
            // Try old format
            let old: AllianceV1 = deserialize_v1(data)?;
            Alliance {
                name: old.name,
                treasury: old.treasury as u128,
            }
        }
    }
}
```

### ❌ Pitfall 3: Breaking API

```rust
// WRONG - old clients can't call
pub fn mint_ship(owner: &Address, ship_type: &Symbol, metadata: &Bytes) -> NewReturn {
    // Changed return type
}
```

**Solution**: Support both APIs:
```rust
pub fn mint_ship(owner: &Address, ship_type: &Symbol, metadata: &Bytes) -> OldReturn {
    // Keep original signature
    let new_ship = mint_ship_internal(owner, ship_type, metadata);
    OldReturn {
        id: new_ship.id,
        owner: new_ship.owner,
    }
}

pub fn mint_ship_v2(owner: &Address, ship_type: &Symbol, metadata: &Bytes) -> NewReturn {
    // New version with more fields
    mint_ship_internal(owner, ship_type, metadata)
}
```

## Checklist

Before deploying an upgrade:

- [ ] All upgrade tests pass
- [ ] Migration code written and tested
- [ ] Backward compatibility verified
- [ ] Rollback procedure documented
- [ ] Data migration plan reviewed
- [ ] State preservation verified
- [ ] Testnet deployment successful
- [ ] Monitored testnet for 1 week
- [ ] No reported issues
- [ ] Upgrade advisory prepared
- [ ] Player communication prepared
- [ ] Emergency contacts ready

## Related Documentation

- [Smart Contract Security](./SMART_CONTRACT_SECURITY.md)
- [State Management](./STATE_MANAGEMENT.md)
- [Disaster Recovery](./DISASTER_RECOVERY.md)
- [Deployment Procedures](./DEPLOYMENT.md)
