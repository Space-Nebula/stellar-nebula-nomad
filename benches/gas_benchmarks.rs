//! Gas/CPU budgets for the most exercised public entrypoints, measured
//! through the contract client so the numbers include host invocation cost.
//!
//! `harness = false`, so `main` runs every benchmark; `cargo test --benches`
//! and `cargo bench --all` both execute it. Each benchmark prints its cost
//! and asserts a ceiling. Ceilings are set at roughly 1.5x the value measured
//! on soroban-sdk 28 so a genuine regression fails while host-version noise
//! does not.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, Symbol};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

const MAX_CPU_NEBULA_GEN: u64 = 4_600_000;
const MAX_CPU_SCAN: u64 = 5_600_000;
const MAX_CPU_HARVEST: u64 = 5_800_000;
const MAX_CPU_BATCH_MINT: u64 = 900_000;
const MAX_CPU_PER_PROFILE_UPDATE: u64 = 45_000;
const MAX_MEM_BYTES: u64 = 1_800_000;

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

fn seed(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

/// Run `op` with an unlimited budget and return `(cpu, memory)`.
fn measure<F: FnOnce()>(env: &Env, op: F) -> (u64, u64) {
    let mut budget = env.cost_estimate().budget();
    budget.reset_unlimited();
    op();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

fn report(name: &str, cpu: u64, mem: u64, cpu_limit: u64, mem_limit: u64) {
    println!("{name}: cpu={cpu} memory={mem} (limits cpu={cpu_limit} memory={mem_limit})");
    assert!(
        cpu <= cpu_limit,
        "{name} exceeded CPU budget: {cpu} > {cpu_limit}"
    );
    assert!(
        mem <= mem_limit,
        "{name} exceeded memory budget: {mem} > {mem_limit}"
    );
}

fn bench_nebula_generation() {
    let (env, client, player) = setup();
    let s = seed(&env, 1);
    let (cpu, mem) = measure(&env, || {
        let _ = client.generate_nebula_layout(&s, &player);
    });
    report(
        "generate_nebula_layout",
        cpu,
        mem,
        MAX_CPU_NEBULA_GEN,
        MAX_MEM_BYTES,
    );
}

fn bench_scan_nebula() {
    let (env, client, player) = setup();
    let s = seed(&env, 2);
    let (cpu, mem) = measure(&env, || {
        let _ = client.scan_nebula(&s, &player);
    });
    report("scan_nebula", cpu, mem, MAX_CPU_SCAN, MAX_MEM_BYTES);
}

fn bench_harvest_resources() {
    let (env, client, player) = setup();
    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let layout = client.generate_nebula_layout(&seed(&env, 3), &player);
    let (cpu, mem) = measure(&env, || {
        let _ = client.harvest_resources(&ship.id, &layout);
    });
    report(
        "harvest_resources",
        cpu,
        mem,
        MAX_CPU_HARVEST,
        MAX_MEM_BYTES,
    );
}

fn bench_batch_mint_ships() {
    let (env, client, player) = setup();
    let ship_types = soroban_sdk::vec![
        &env,
        symbol_short!("fighter"),
        symbol_short!("explorer"),
        symbol_short!("hauler"),
    ];
    let (cpu, mem) = measure(&env, || {
        let _ = client.batch_mint_ships(&player, &ship_types, &Bytes::new(&env));
    });
    println!("  per ship: {} CPU", cpu / 3);
    report(
        "batch_mint_ships_3",
        cpu,
        mem,
        MAX_CPU_BATCH_MINT,
        MAX_MEM_BYTES,
    );
}

fn bench_storage_operations() {
    let (env, client, player) = setup();
    let profile_id = client.initialize_profile(&player);
    let (cpu, mem) = measure(&env, || {
        for i in 0..5u32 {
            client.update_progress(&player, &profile_id, &(i + 1), &(i128::from(i + 1) * 100));
        }
    });
    println!("  per update: {} CPU", cpu / 5);
    report(
        "profile_update_x5",
        cpu,
        mem,
        MAX_CPU_PER_PROFILE_UPDATE * 5,
        MAX_MEM_BYTES,
    );
}

fn bench_difficulty() {
    let (env, client, _) = setup();
    let (cpu, mem) = measure(&env, || {
        let _ = client.calculate_difficulty(&50);
    });
    report("calculate_difficulty", cpu, mem, 30_000, MAX_MEM_BYTES);
}

fn bench_level_quote() {
    let (env, client, player) = setup();
    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let asset: Symbol = symbol_short!("ore");
    let (cpu, mem) = measure(&env, || {
        let _ = client.level_upgrade_cost(&ship.id);
        let _ = client.get_resource_balance(&player, &asset);
    });
    report("level_upgrade_cost", cpu, mem, 60_000, MAX_MEM_BYTES);
}

fn main() {
    bench_nebula_generation();
    bench_scan_nebula();
    bench_harvest_resources();
    bench_batch_mint_ships();
    bench_storage_operations();
    bench_difficulty();
    bench_level_quote();
    println!("gas_benchmarks: all within budget");
}
