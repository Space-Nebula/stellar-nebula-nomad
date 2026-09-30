//! Gas/CPU benchmarks for critical paths not covered by `gas_benchmarks.rs`:
//! alliance founding, ship energy consume/recharge, crafting, the emergency
//! pause path, and the new staking and level-ladder entrypoints.
//!
//! `harness = false`: `main` runs every benchmark under both
//! `cargo test --benches` and `cargo bench --all`. Ceilings are roughly 1.5x
//! the values measured on soroban-sdk 28 through the contract client.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, Env, String, Vec};
use stellar_nebula_nomad::recipes::Recipe;
use stellar_nebula_nomad::resource_minter::credit_resource_balance;
use stellar_nebula_nomad::{
    craft, set_recipe, LockTier, NebulaNomadContract, NebulaNomadContractClient,
};

const MAX_CPU_FOUND_ALLIANCE: u64 = 350_000;
const MAX_CPU_ENERGY_OP: u64 = 175_000;
const MAX_CPU_CRAFT: u64 = 370_000;
const MAX_CPU_EMERGENCY_PAUSE: u64 = 100_000;
const MAX_CPU_STAKE: u64 = 300_000;
const MAX_CPU_LEVEL_UPGRADE: u64 = 360_000;
const MAX_MEM_BYTES: u64 = 200_000;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address, Address) {
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
    (env, client, id, player)
}

fn measure<F: FnOnce()>(env: &Env, op: F) -> (u64, u64) {
    let mut budget = env.cost_estimate().budget();
    budget.reset_unlimited();
    op();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

fn report(name: &str, cpu: u64, mem: u64, cpu_limit: u64) {
    println!("{name}: cpu={cpu} memory={mem} (limit cpu={cpu_limit})");
    assert!(
        cpu <= cpu_limit,
        "{name} exceeded CPU budget: {cpu} > {cpu_limit}"
    );
    assert!(
        mem <= MAX_MEM_BYTES,
        "{name} exceeded memory budget: {mem} > {MAX_MEM_BYTES}"
    );
}

/// Alliance founding: touches five storage writes (record, count,
/// membership, treasury, contribution).
fn bench_found_alliance() {
    let (env, client, _, founder) = setup();
    let name = String::from_str(&env, "Star Reavers");
    let (cpu, mem) = measure(&env, || {
        let _ = client.found_alliance(&founder, &name);
    });
    report("found_alliance", cpu, mem, MAX_CPU_FOUND_ALLIANCE);
}

/// Energy consume/recharge runs on nearly every scan, harvest and combat
/// action, so it has to stay cheap.
fn bench_energy_consume_and_recharge() {
    let (env, client, _, owner) = setup();
    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    let (consume_cpu, consume_mem) = measure(&env, || {
        let _ = client.consume_energy(&ship.id, &500);
    });
    report(
        "consume_energy",
        consume_cpu,
        consume_mem,
        MAX_CPU_ENERGY_OP,
    );
    let (recharge_cpu, recharge_mem) = measure(&env, || {
        let _ = client.recharge_energy(&ship.id, &200);
    });
    report(
        "recharge_energy",
        recharge_cpu,
        recharge_mem,
        MAX_CPU_ENERGY_OP,
    );
}

/// Crafting: recipe lookup, resource check/consume, mastery bookkeeping and
/// output mint. A zero-input recipe isolates the path overhead from funding.
fn bench_craft() {
    let (env, _, id, player) = setup();
    env.as_contract(&id, || {
        set_recipe(
            &env,
            &Recipe {
                id: 1,
                inputs: Vec::new(&env),
                output: (symbol_short!("essence"), 10),
                rarity: 1,
                required_level: 0,
            },
        );
    });
    let (cpu, mem) = measure(&env, || {
        env.as_contract(&id, || {
            craft(env.clone(), player.clone(), 1).unwrap();
        });
    });
    report("craft", cpu, mem, MAX_CPU_CRAFT);
}

/// Emergency pause must stay cheap enough to land under congestion.
fn bench_emergency_pause() {
    let (env, client, _, admin) = setup();
    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.initialize_admins(&admins);
    let (cpu, mem) = measure(&env, || {
        client.pause_contract(&admin);
    });
    report("pause_contract", cpu, mem, MAX_CPU_EMERGENCY_PAUSE);
}

/// Resource staking deposit: balance debit, stake record, TVL counters.
fn bench_stake_resource() {
    let (env, client, id, owner) = setup();
    let asset = symbol_short!("ore");
    env.as_contract(&id, || {
        credit_resource_balance(&env, &owner, &asset, 100_000).unwrap();
    });
    let (cpu, mem) = measure(&env, || {
        let _ = client.stake_resource(&owner, &asset, &100_000, &LockTier::Days30);
    });
    report("stake_resource", cpu, mem, MAX_CPU_STAKE);
}

/// Level upgrade: ownership check, curve pricing, balance burn, level write.
fn bench_level_upgrade() {
    let (env, client, id, owner) = setup();
    let asset = symbol_short!("ore");
    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    env.as_contract(&id, || {
        credit_resource_balance(&env, &owner, &asset, 10_000).unwrap();
    });
    let (cpu, mem) = measure(&env, || {
        let _ = client.upgrade_ship_level(&owner, &ship.id, &asset);
    });
    report("upgrade_ship_level", cpu, mem, MAX_CPU_LEVEL_UPGRADE);
}

fn main() {
    bench_found_alliance();
    bench_energy_consume_and_recharge();
    bench_craft();
    bench_emergency_pause();
    bench_stake_resource();
    bench_level_upgrade();
    println!("critical_path_benchmarks: all within budget");
}
