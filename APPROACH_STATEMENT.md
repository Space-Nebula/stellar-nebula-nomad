# Approach Statement: Issue #41 - Role-Based Access Control (RBAC)

## Reconnaissance Summary

After full codebase analysis, the following architectural decisions have been established:

### Source Control & Branch
- **Branch Point**: Latest main (confirmed via `git log --oneline -5`)
- **Branch Name**: `feature/role-based-access-control`
- **Target**: PR against main

### Soroban SDK Version & Constraints
- **SDK**: 22.0 (soroban-sdk = "22.0", exact version pinned in Cargo.toml)
- **Symbol Length**: Soroban SDK 22 supports Symbols up to 32 bytes. All role names, action names, and event topics must fit within this limit. 
  - Chosen names: `admin` (5), `nomad` (5), `indexer` (7), `grant_role` (10), `revoke_role` (11), `transfer_admin` (14) — all well within limit.
  - Event topics: `RoleGranted` (11), `RoleRevoked` (11), `AdminTransferred` (15), `PermissionGranted` (17), `PermissionRevoked` (17) — all within limit via symbol_short!().

### Storage Tier & DataKey Pattern
- **Tier**: Persistent storage for all RBAC state (roles, permissions) using `env.storage().persistent()`.
- **Existing Pattern**: Codebase uses #[contracttype] enums as storage keys with structured variants. Examples: `DataKey::Ship(u64)`, `DataKey::OwnerShips(Address)` in ship_nft.rs. Variants store typed tuples in the enum definition itself.
- **DataKey Extension**: A new `AccessControlKey` enum will be added to access_control.rs (not integrated into every module's key space — follows the pattern where each functional module has its own key type, e.g., `ShipDataKey`, `EmergencyKey`, `ResourceKey`).
- **Key Variants**:
  ```rust
  pub enum AccessControlKey {
      Admin,                           // Stores Address
      RoleMember(Symbol, Address),    // (role, address) -> RoleRecord
      RolePermission(Symbol, Symbol), // (role, action) -> bool
      RoleName(Symbol),               // Mark that a role has been defined (optional, optimization)
  }
  ```
- **TTL Policy**: Persistent entries receive default TTL bumps at storage write time. Following existing pattern in resource_minter.rs and other modules, no explicit TTL bump is performed on writes; the SDK defaults apply. Rationale: RBAC state is administrative/static and does not require aggressive TTL management like often-mutated player data.

### Time-Bound Expiry Mechanism
- **Mechanism**: Ledger sequence number (`env.ledger().sequence()`) used for expiry comparison.
- **Rationale**: 
  - Existing codebase uses both ledger sequence and timestamps.
  - Timestamp examples: `env.ledger().timestamp()` in governance.rs, VOTING_PERIOD = 86400 * 3.
  - Ledger sequence examples: staking.rs uses `last_claim_ledger`, `min_lock_ledger`.
  - **Decision**: Ledger sequence is chosen because:
    1. It is manipulation-resistant by design in Soroban (monolithic ledger state).
    2. It aligns with resource_minter.rs patterns (LEDGERS_PER_DAY = 17_280).
    3. Role expiry is administrative and not user-facing, so the finer granularity of ledger-based expiry is appropriate.
- **Implementation**: `RoleRecord` contains `Option<u32>` for expiry_ledger. Comparison: `if let Some(exp) = record.expiry_ledger { if env.ledger().sequence() >= exp { return false; } }`.

### Error Enum Extension Strategy
- **Approach**: A new `AccessControlError` enum is added to access_control.rs and re-exported from lib.rs.
- **Rationale**: 
  - Existing modules (ship_nft.rs, emergency_controls.rs, resource_minter.rs) each define their own error enums with #[contracterror].
  - No shared error enum exists across modules.
  - AccessControlError is specific to RBAC operations and will not be confused with existing errors.
  - Error codes will use error code range 1-100 (accessing existing codes: EmergencyError uses 1-6, ShipError uses 1-7, ResourceError uses 1-8, etc.; no conflict will occur with range 1-100).
- **Variants** (with codes):
  ```rust
  #[contracterror]
  #[derive(Clone, Copy, Debug, Eq, PartialEq)]
  #[repr(u32)]
  pub enum AccessControlError {
      AdminRequired = 1,           // Caller is not the admin
      RoleNotFound = 2,           // Role was never defined or does not exist
      UnauthorizedRole = 3,       // Caller does not hold the required role (detail: 0=not_held, 1=expired, 2=revoked)
      RoleAlreadyGranted = 4,     // Role is already held by address (idempotency policy)
      BatchLimitExceeded = 5,     // Batch grant exceeded 5 addresses
      InvalidExpiry = 6,          // Expiry is in the past or invalid
      PermissionNotFound = 7,     // Permission (role, action) was never defined
      InitializationFailed = 8,  // init_roles already called or other init error
  }
  ```

### Event Emission Pattern
- **Existing Pattern** (e.g., staking.rs, governance.rs):
  ```rust
  env.events().publish(
      (symbol_short!("scope"), symbol_short!("action")),
      (data_tuple,),
  );
  ```
- **Topic Structure**: Tuple of 1-2 Symbols (scope/action pair).
- **Data Payload**: Tuple of event data (Address, Amount, Boolean, etc.).
- **AccessControl Events** (same pattern):
  ```rust
  env.events().publish(
      (symbol_short!("rbac"), symbol_short!("grant")),
      (role.clone(), grantee.clone(), expiry),
  );
  env.events().publish(
      (symbol_short!("rbac"), symbol_short!("revoke")),
      (role.clone(), revokee.clone()),
  );
  env.events().publish(
      (symbol_short!("rbac"), symbol_short!("xfer_adm")),
      (old_admin.clone(), new_admin.clone()),
  );
  env.events().publish(
      (symbol_short!("rbac"), symbol_short!("perm_g")),
      (role.clone(), action.clone()),
  );
  env.events().publish(
      (symbol_short!("rbac"), symbol_short!("perm_r")),
      (role.clone(), action.clone()),
  );
  ```
- **PermissionChecked Event**: The issue specifies emitting PermissionChecked events on read-only checks. **Decision**: No PermissionChecked events will be emitted. Rationale: The existing codebase never emits events from read-only view functions (see analytics.rs::snapshot_leaderboard, all view functions in governance.rs). Emitting on every has_permission or check_permission call would create excessive event noise on every operation and is inconsistent with the codebase's event emission philosophy (events track state mutations, not reads). The `check_permission` function will still emit a `PermissionGranted`-like event only if configured by maintainers; for now, it will emit **PermissionChecked** event on each call to create an audit trail (decision subject to maintainer feedback in PR).

### check_permission Role Enumeration Strategy
- **Challenge**: Soroban's storage model does not provide a built-in way to iterate all keys matching a prefix or pattern. To determine all roles held by a caller, we cannot enumerate arbitrary keys.
- **Strategy Chosen**: **Known Role Enumeration** - Maintain a list of all defined role names in storage (or check explicitly against known roles: `admin`, `nomad`, `indexer`, and any other roles explicitly added via future `define_role` API).
  - For each known role, call `has_role(env, role, caller)` to check if caller holds it.
  - If any role permits the action, return success.
  - Rationale: RBAC is administrative. The set of roles is expected to be small and rarely changed. Explicit checking is clearer and avoids storage iteration issues.
  - **Limitation**: New roles must be explicitly registered in a `known_roles` list in storage. Future DAO governance that adds roles dynamically must update this list.
  - This is conservative and safe; scalability can be improved later if needed.

### Initialization Integration Strategy
- **Existing Pattern**: Codebase has no single "master initialize" function. Each module has independent init functions (initialize_admins, initialize_version, initialize_sponsorship, etc.) callable separately on the main NebulaNomadContract.
- **Decision**: `init_roles(env: &Env, admin: Address)` will be a public function exposed from lib.rs and callable as a separate contract method.
  - Admin must authorize the call.
  - First call: Creates admin role, grants admin role to admin address, creates nomad and indexer roles with no members.
  - Second call+: Reverts with `InitializationFailed`.
  - **Default Permissions**: At init, no default permissions are granted to any role. This is a minimal-privilege default; maintainers must explicitly call `grant_permission` to define what each role can do. (This can be changed during review if a different default permission matrix is desired.)

### Default Roles & Permission Matrix
- **Default Roles** (created at init):
  - `admin`: Full privilege (no permissions are hardcoded; admin can perform any action via explicit permissioning).
  - `nomad`: Regular player role (initially no permissions; must be explicitly granted).
  - `indexer`: Data indexer role (initially no permissions; must be explicitly granted).
- **Default Permission Matrix**:
  - At initialization, no permissions are granted to any role.
  - Admin can grant permissions to roles via `grant_permission(admin, role, action)`.
  - Actions are identified by Symbol constants defined in access_control.rs or in the functions that check them.
  - **Example actions** (derived from scanning mutating functions): grant_role, revoke_role, scan_nebula, harvest_resource, mint_ship, transfer_admin, pause_contract, etc.
  - The exact permission matrix will be documented in the PR and implemented as a reference table in tests.

### Mutating Contract Functions Requiring RBAC Checks
After scanning all functions in src/**/*.rs, the following are identified as mutating operations that should have RBAC guards (sample; full list to be integrated):

**Priority 1 (Core Governance)**:
- `grant_role(admin, role, grantee, expiry)` — action: `grant_role` — requires: `admin` role
- `revoke_role(admin, role, revokee)` — action: `revoke_role` — requires: `admin` role
- `grant_permission(admin, role, action)` — action: `grant_permission` — requires: `admin` role
- `transfer_admin(admin, new_admin)` — action: `transfer_admin` — requires: `admin` role
- `pause_contract(admin)` — action: `pause_contract` — requires: `admin` role (already exists in emergency_controls)

**Priority 2 (Admin/Config)**:
- `initialize_refund(admin)` — action: `init_refund` — requires: `admin` role
- `initialize_bounty_board(admin)` — action: `init_bounty` — requires: `admin` role
- `initialize_fractional(admin)` — action: `init_fractional` — requires: `admin` role
- Market oracle functions (initialize_oracle, update_resource_price, add_oracle_source) — action: `admin_oracle` — requires: `admin` role

**Priority 3 (if expanded)**:
- Any config update, pause/unpause, or governance action would be checked against the specified role.

**Integration Point**: Each function's first statement (after parameter validation if any) will include an early-exit call:
```rust
check_permission(&env, &caller, symbol_short!("action_name"))?;
```

### DAO Extensibility Hook
- **Stub Function**:
  ```rust
  pub fn propose_role_change(
      env: &Env,
      proposer: Address,
      role: Symbol,
      grantee: Address,
      action_type: Symbol, // "grant", "revoke", "expire_update"
  ) -> Result<u64, AccessControlError> {
      // Immediately revert with NotImplemented (to be added to error enum)
      Err(AccessControlError::NotImplemented)
  }
  ```
- **Documentation**: A detailed comment explaining the intended interface:
  ```
  /// Future: DAO Proposal System for Role Changes
  /// This function is a placeholder for integration with a DAO governance system.
  /// Expected future behavior (Issue #41 follow-up):
  /// 1. Proposer creates a role change proposal (grant/revoke role, modify permissions).
  /// 2. Proposal is stored with a unique ID and a timelock (e.g., 7-day voting period).
  /// 3. DAO members vote via a quorum-based system.
  /// 4. After voting period closes, if quorum reached and proposal passed, execution functions in this module automatically apply the change.
  /// 5. Signature: `execute_dao_proposal(env, proposal_id) -> Result<(), AccessControlError>` — executes approved proposal.
  /// This stub allows the interface to be defined now without requiring a full DAO implementation in this PR.
  ```
- **Alternative Considered**: Defining a trait for DAO governance and allowing external DAO contracts to be registered. Decision: Stub is simpler for now; can be extended in a follow-up issue.

### Files to Create/Modify

**Create**:
- `src/access_control.rs` (new RBAC module, ~600-800 lines including tests embedded or separate)
- Potentially `docs/rbac_pattern.md` (reusable pattern guide) if docs/ structure exists

**Modify**:
- `src/lib.rs` — Add module declaration `mod access_control; pub use access_control::*;` and integrate `init_roles` call into contract or expose as public function.
- Target functions in src/ — Add RBAC checks to every mutating function (sample: src/emergency_controls.rs, src/resource_minter.rs, src/governance.rs, etc.)
- Potentially Cargo.toml if dependencies must be added (likely none — SDK 22 already includes everything)

**Deliberately Not Touched**:
- Any function that is read-only or view-only (no storage mutation).
- Core nebula exploration mechanics (scan_nebula is called by players; RBAC would gate administrative functions, not player gameplay).
- Existing error paths or unwrap sites (no refactoring beyond RBAC checks).

### Unresolved Questions Requiring Maintainer Confirmation

1. **Default Permission Matrix**: Should admin be able to perform ALL actions via hardcoded permission at init, or should all permissions (including admin) be explicitly granted after init? Recommend: initially no perms, admin must grant; provides least-privilege safety.

2. **Batch Grant Limit Semantics**: Is the 5-address limit per-call (multiple batch calls can grant 10 total) or cumulative (max 5 total role holders)? Recommend: per-call, as stated in issue.

3. **Event Emission on check_permission**: Should PermissionChecked events be emitted (high cardinality, every operation emits), or should permission checks be silent? Recommend: silent for now (matches codebase convention), can enable via flag if audit trail needed.

4. **Admin Transfer Constraints**: Can a new admin be the caller themselves (no-op transfer), or must it be a different address? Recommend: can be same address (no-op), but emit event anyway (explicit intent).

5. **Role Member Enumeration for check_permission**: Is the "known roles" approach acceptable, or should a reverse index (address -> roles set) be maintained? Recommend: known roles for MVP, can optimize later.

---

## Implementation Order

1. Define AccessControlError enum and RoleRecord struct.
2. Extend storage via AccessControlKey and helper functions.
3. Implement core role functions: has_role, has_permission, grant_role, revoke_role.
4. Implement admin transfer: transfer_admin.
5. Implement batch grant: grant_role_batch.
6. Implement check_permission (the main guard function).
7. Add permission functions: grant_permission, revoke_permission.
8. Add init_roles and idempotency guard.
9. Integrate RBAC checks into all mutating contract functions from sample list above.
10. Write comprehensive test suite (95% coverage).
11. Run CI checks locally.

---

## Next Steps

Upon approval of this approach:
1. Create src/access_control.rs with full module.
2. Integrate into lib.rs.
3. Add RBAC checks to identified functions.
4. Write and run tests.
5. Validate against soroban contract analyze.
6. Submit PR with design notes and security checklist.

