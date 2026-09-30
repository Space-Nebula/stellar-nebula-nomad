//! Performance regression gate: the hottest entrypoints must stay under
//! fixed CPU ceilings, and a batch mint must not cost more than the singles
//! it replaces.
//!
//! `harness = false`: `main` runs every check under `cargo test --benches`
//! and `cargo bench --all`. Ceilings are roughly 1.5x the values measured on
//! soroban-sdk 28 through the contract client.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String, Vec};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

const MAX_CPU_NEBULA_GEN: u64 = 6_000_000;
const MAX_CPU_SCAN: u64 = 6_000_000;
const MAX_CPU_HARVEST: u64 = 6_000_000;
const MAX_CPU_MINT: u64 = 3_000_000;
const MAX_CPU_PROFILE_UPDATE: u64 = 2_000_000;
const MAX_CPU_FOUND_ALLIANCE: u64 = 4_000_000;
const MAX_CPU_ENERGY_OP: u64 = 2_000_000;
const MAX_CPU_EMERGENCY_PAUSE: u64 = 2_000_000;
const MAX_MEM_BYTES: u64 = 6_000_000;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });
    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    let player = Address::generate(&env);
    (env, client, player)
}

fn measure<F: FnOnce()>(env: &Env, op: F) -> (u64, u64) {
    let mut budget = env.cost_estimate().budget();
    budget.reset_unlimited();
    op();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

fn gate(name: &str, cpu: u64, mem: u64, cpu_limit: u64) {
    println!("{name}: cpu={cpu} memory={mem}");
    assert!(
        cpu <= cpu_limit,
        "REGRESSION: {name} CPU {cpu} exceeds baseline {cpu_limit}"
    );
    assert!(
        mem <= MAX_MEM_BYTES,
        "REGRESSION: {name} memory {mem} exceeds baseline {MAX_MEM_BYTES}"
    );
}

fn regression_nebula_generation() {
    let (env, client, player) = setup();
    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let (cpu, mem) = measure(&env, || {
        let _ = client.generate_nebula_layout(&seed, &player);
    });
    gate("generate_nebula_layout", cpu, mem, MAX_CPU_NEBULA_GEN);
}

fn regression_scan_operation() {
    let (env, client, player) = setup();
    let seed = BytesN::from_array(&env, &[2u8; 32]);
    let (cpu, mem) = measure(&env, || {
        let _ = client.scan_nebula(&seed, &player);
    });
    gate("scan_nebula", cpu, mem, MAX_CPU_SCAN);
}

fn regression_harvest() {
    let (env, client, player) = setup();
    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let layout = client.generate_nebula_layout(&BytesN::from_array(&env, &[3u8; 32]), &player);
    let (cpu, mem) = measure(&env, || {
        let _ = client.harvest_resources(&ship.id, &layout);
    });
    gate("harvest_resources", cpu, mem, MAX_CPU_HARVEST);
}

fn regression_mint_ship() {
    let (env, client, player) = setup();
    let (cpu, mem) = measure(&env, || {
        let _ = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    });
    gate("mint_ship", cpu, mem, MAX_CPU_MINT);
}

/// A batch of three ships must not cost more than three single mints plus a
/// 10 % allowance; otherwise the batch path has lost its point.
fn regression_batch_efficiency() {
    let (env, client, player) = setup();
    let (single_cpu, _) = measure(&env, || {
        let _ = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    });
    let ship_types = soroban_sdk::vec![
        &env,
        symbol_short!("fighter"),
        symbol_short!("explorer"),
        symbol_short!("hauler"),
    ];
    let (batch_cpu, _) = measure(&env, || {
        let _ = client.batch_mint_ships(&player, &ship_types, &Bytes::new(&env));
    });
    println!(
        "batch_mint_ships_3: cpu={batch_cpu} vs 3 singles={}",
        single_cpu * 3
    );
    assert!(
        batch_cpu * 10 <= single_cpu * 3 * 11,
        "REGRESSION: batch mint ({batch_cpu}) costs more than 3 singles + 10% ({})",
        single_cpu * 3 * 11 / 10
    );
}

fn regression_storage_bump_cost() {
    let (env, client, player) = setup();
    let profile_id = client.initialize_profile(&player);
    let (cpu, mem) = measure(&env, || {
        client.update_progress(&player, &profile_id, &1, &100);
    });
    gate("update_progress", cpu, mem, MAX_CPU_PROFILE_UPDATE);
}

fn regression_found_alliance() {
    let (env, client, founder) = setup();
    let name = String::from_str(&env, "Regression Alliance");
    let (cpu, mem) = measure(&env, || {
        let _ = client.found_alliance(&founder, &name);
    });
    gate("found_alliance", cpu, mem, MAX_CPU_FOUND_ALLIANCE);
}

fn regression_energy_consume_recharge() {
    let (env, client, owner) = setup();
    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    let (consume_cpu, consume_mem) = measure(&env, || {
        let _ = client.consume_energy(&ship.id, &500);
    });
    gate(
        "consume_energy",
        consume_cpu,
        consume_mem,
        MAX_CPU_ENERGY_OP,
    );
    let (recharge_cpu, recharge_mem) = measure(&env, || {
        let _ = client.recharge_energy(&ship.id, &200);
    });
    gate(
        "recharge_energy",
        recharge_cpu,
        recharge_mem,
        MAX_CPU_ENERGY_OP,
    );
}

fn regression_emergency_pause() {
    let (env, client, admin) = setup();
    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.initialize_admins(&admins);
    let (cpu, mem) = measure(&env, || {
        client.pause_contract(&admin);
    });
    gate("pause_contract", cpu, mem, MAX_CPU_EMERGENCY_PAUSE);
}

fn main() {
    regression_nebula_generation();
    regression_scan_operation();
    regression_harvest();
    regression_mint_ship();
    regression_batch_efficiency();
    regression_storage_bump_cost();
    regression_found_alliance();
    regression_energy_consume_recharge();
    regression_emergency_pause();
    println!("performance_regression: no regressions");
}
