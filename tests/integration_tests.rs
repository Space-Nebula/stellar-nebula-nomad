#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env};
use stellar_nebula_nomad::{
    BondStatus, NebulaNomadContract, NebulaNomadContractClient, NomadBond, YieldDelegation,
};

// ── Test Helpers ─────────────────────────────────────────────────────────────

fn setup_env() -> (Env, NebulaNomadContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 500,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register_contract(None, NebulaNomadContract);
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    (env, client)
}

fn two_players(env: &Env) -> (Address, Address) {
    (Address::generate(env), Address::generate(env))
}

// ═══════════════════════════════════════════════════════════════════════════
// create_bond
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_create_bond_returns_pending() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    let bond = client.create_bond(&alice, &1, &bob);
    assert_eq!(bond.bond_id, 1);
    assert_eq!(bond.initiator, alice);
    assert_eq!(bond.partner, bob);
    assert_eq!(bond.ship_id, 1);
    assert_eq!(bond.status, BondStatus::Pending);
    assert_eq!(bond.created_at, 1_700_000_000);
}

#[test]
fn test_create_bond_increments_ids() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    let charlie = Address::generate(&env);

    let b1 = client.create_bond(&alice, &1, &bob);
    let b2 = client.create_bond(&alice, &2, &charlie);
    assert_eq!(b1.bond_id, 1);
    assert_eq!(b2.bond_id, 2);
}

#[test]
#[should_panic(expected = "cannot bond with yourself")]
fn test_create_bond_self_bond_panics() {
    let (env, client) = setup_env();
    let alice = Address::generate(&env);
    client.create_bond(&alice, &1, &alice);
}

#[test]
fn test_create_bond_emits_event() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    client.create_bond(&alice, &1, &bob);

    let events = env.events().all();
    assert!(!events.is_empty(), "expected bond created event");
}

// ═══════════════════════════════════════════════════════════════════════════
// accept_bond
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_accept_bond_activates() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    let bond = client.accept_bond(&bob, &1);
    assert_eq!(bond.status, BondStatus::Active);
}

#[test]
#[should_panic(expected = "only the designated partner can accept")]
fn test_accept_bond_wrong_partner_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    let charlie = Address::generate(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&charlie, &1);
}

#[test]
#[should_panic(expected = "bond is not pending")]
fn test_accept_bond_already_active_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.accept_bond(&bob, &1); // second accept
}

#[test]
#[should_panic(expected = "bond not found")]
fn test_accept_bond_nonexistent_panics() {
    let (env, client) = setup_env();
    let bob = Address::generate(&env);
    client.accept_bond(&bob, &99);
}

// ═══════════════════════════════════════════════════════════════════════════
// delegate_yield
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_delegate_yield_sets_config() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);

    let del = client.delegate_yield(&alice, &1, &50);
    assert_eq!(del.bond_id, 1);
    assert_eq!(del.delegator, alice);
    assert_eq!(del.beneficiary, bob);
    assert_eq!(del.percentage, 50);
    assert_eq!(del.total_yielded, 0);
}

#[test]
fn test_delegate_yield_partner_to_initiator() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);

    let del = client.delegate_yield(&bob, &1, &30);
    assert_eq!(del.delegator, bob);
    assert_eq!(del.beneficiary, alice);
    assert_eq!(del.percentage, 30);
}

#[test]
#[should_panic(expected = "percentage must be 1-100")]
fn test_delegate_yield_zero_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &0);
}

#[test]
#[should_panic(expected = "percentage must be 1-100")]
fn test_delegate_yield_over_100_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &101);
}

#[test]
#[should_panic(expected = "bond is not active")]
fn test_delegate_yield_pending_bond_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.delegate_yield(&alice, &1, &50); // bond still pending
}

#[test]
#[should_panic(expected = "caller is not part of this bond")]
fn test_delegate_yield_outsider_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    let charlie = Address::generate(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&charlie, &1, &50);
}

// ═══════════════════════════════════════════════════════════════════════════
// accrue_essence + claim_yield (the core yield flow)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_accrue_essence() {
    let (env, client) = setup_env();
    let alice = Address::generate(&env);

    client.accrue_essence(&alice, &1000);
    assert_eq!(client.get_essence_balance(&alice), 1000);
}

#[test]
fn test_accrue_essence_accumulates() {
    let (env, client) = setup_env();
    let alice = Address::generate(&env);

    client.accrue_essence(&alice, &500);
    client.accrue_essence(&alice, &300);
    assert_eq!(client.get_essence_balance(&alice), 800);
}

#[test]
fn test_claim_yield_transfers_correct_amount() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    // Bond + activate + delegate 50%
    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);

    // Alice earns 1000 essence
    client.accrue_essence(&alice, &1000);

    // Bob claims 50% of Alice's balance → 500
    let claimed = client.claim_yield(&bob, &1);
    assert_eq!(claimed, 500);
    assert_eq!(client.get_essence_balance(&alice), 500);
    assert_eq!(client.get_essence_balance(&bob), 500);
}

#[test]
fn test_claim_yield_updates_total_yielded() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &25);
    client.accrue_essence(&alice, &400);

    client.claim_yield(&bob, &1);
    let del = client.get_yield_delegation(&1);
    assert_eq!(del.total_yielded, 100); // 25% of 400
}

#[test]
fn test_claim_yield_zero_balance_returns_zero() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);

    // No essence accrued — claim returns 0
    let claimed = client.claim_yield(&bob, &1);
    assert_eq!(claimed, 0);
}

#[test]
fn test_claim_yield_multiple_claims() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);

    client.accrue_essence(&alice, &1000);
    let c1 = client.claim_yield(&bob, &1); // 50% of 1000 = 500
    assert_eq!(c1, 500);

    let c2 = client.claim_yield(&bob, &1); // 50% of 500 = 250
    assert_eq!(c2, 250);

    assert_eq!(client.get_essence_balance(&alice), 250);
    assert_eq!(client.get_essence_balance(&bob), 750);
}

#[test]
#[should_panic(expected = "only the beneficiary can claim yield")]
fn test_claim_yield_non_beneficiary_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    let charlie = Address::generate(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);
    client.accrue_essence(&alice, &1000);

    client.claim_yield(&charlie, &1); // charlie is not the beneficiary
}

#[test]
#[should_panic(expected = "only the beneficiary can claim yield")]
fn test_claim_yield_delegator_cannot_claim_own_delegation() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);
    client.accrue_essence(&alice, &1000);

    client.claim_yield(&alice, &1); // Alice delegated, she can't claim her own
}

#[test]
fn test_claim_yield_emits_event() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);
    client.accrue_essence(&alice, &1000);
    client.claim_yield(&bob, &1);

    let events = env.events().all();
    assert!(events.len() >= 1, "expected yield claimed event");
}

// ═══════════════════════════════════════════════════════════════════════════
// dissolve_bond
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_dissolve_bond_by_initiator() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    let bond = client.dissolve_bond(&alice, &1);
    assert_eq!(bond.status, BondStatus::Dissolved);
}

#[test]
fn test_dissolve_bond_by_partner() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    let bond = client.dissolve_bond(&bob, &1);
    assert_eq!(bond.status, BondStatus::Dissolved);
}

#[test]
#[should_panic(expected = "only bonded parties can dissolve")]
fn test_dissolve_bond_outsider_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);
    let charlie = Address::generate(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.dissolve_bond(&charlie, &1);
}

#[test]
#[should_panic(expected = "bond is already dissolved")]
fn test_dissolve_bond_twice_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.dissolve_bond(&alice, &1);
    client.dissolve_bond(&alice, &1); // second dissolve
}

#[test]
#[should_panic(expected = "bond is not active")]
fn test_claim_yield_after_dissolve_panics() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &50);
    client.accrue_essence(&alice, &1000);
    client.dissolve_bond(&alice, &1);

    client.claim_yield(&bob, &1); // bond dissolved — claim blocked
}

// ═══════════════════════════════════════════════════════════════════════════
// Read-only views
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_get_bond() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    let bond = client.get_bond(&1);
    assert_eq!(bond.initiator, alice);
    assert_eq!(bond.partner, bob);
}

#[test]
fn test_get_yield_delegation() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    client.accept_bond(&bob, &1);
    client.delegate_yield(&alice, &1, &75);

    let del = client.get_yield_delegation(&1);
    assert_eq!(del.percentage, 75);
}

#[test]
fn test_get_essence_balance_default_zero() {
    let (env, client) = setup_env();
    let unknown = Address::generate(&env);
    assert_eq!(client.get_essence_balance(&unknown), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Full end-to-end flow
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_bonding_flow() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    // 1. Alice proposes bond with Bob on ship #42
    let bond = client.create_bond(&alice, &42, &bob);
    assert_eq!(bond.status, BondStatus::Pending);

    // 2. Bob accepts
    let bond = client.accept_bond(&bob, &bond.bond_id);
    assert_eq!(bond.status, BondStatus::Active);

    // 3. Alice delegates 40% of yields to Bob
    let del = client.delegate_yield(&alice, &bond.bond_id, &40);
    assert_eq!(del.percentage, 40);

    // 4. Alice earns 2000 cosmic essence from exploration
    client.accrue_essence(&alice, &2000);
    assert_eq!(client.get_essence_balance(&alice), 2000);

    // 5. Bob claims his 40% → 800
    let claimed = client.claim_yield(&bob, &bond.bond_id);
    assert_eq!(claimed, 800);
    assert_eq!(client.get_essence_balance(&alice), 1200);
    assert_eq!(client.get_essence_balance(&bob), 800);

    // 6. Alice dissolves the bond
    let bond = client.dissolve_bond(&alice, &bond.bond_id);
    assert_eq!(bond.status, BondStatus::Dissolved);

    // 7. Balances remain — only future claims are blocked
    assert_eq!(client.get_essence_balance(&alice), 1200);
    assert_eq!(client.get_essence_balance(&bob), 800);
}

#[test]
fn test_dissolve_pending_bond() {
    let (env, client) = setup_env();
    let (alice, bob) = two_players(&env);

    client.create_bond(&alice, &1, &bob);
    // Initiator dissolves before partner accepts
    let bond = client.dissolve_bond(&alice, &1);
    assert_eq!(bond.status, BondStatus::Dissolved);
}

