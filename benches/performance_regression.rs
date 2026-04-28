/// Performance regression test suite
/// Ensures optimizations don't regress over time
use soroban_sdk::{testutils::Address as _, Address, Env};
use stellar_nebula_nomad::*;

const MAX_CPU_NEBULA_GEN: u64 = 1_000_000;
const MAX_CPU_SCAN: u64 = 2_000_000;
const MAX_CPU_HARVEST: u64 = 1_500_000;
const MAX_CPU_MINT: u64 = 800_000;
const MAX_MEM_BYTES: u64 = 100_000;

#[test]
fn regression_nebula_generation() {
    let env = Env::default();
    let player = Address::generate(&env);
    let seed = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(&env, &[1u8; 32]));
    
    env.budget().reset_unlimited();
    let _layout = generate_nebula_layout(env.clone(), seed, player);
    
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();
    
    assert!(cpu <= MAX_CPU_NEBULA_GEN, 
        "REGRESSION: Nebula gen CPU {} exceeds baseline {}", cpu, MAX_CPU_NEBULA_GEN);
    assert!(mem <= MAX_MEM_BYTES,
        "REGRESSION: Memory {} exceeds baseline {}", mem, MAX_MEM_BYTES);
}

#[test]
fn regression_scan_operation() {
    let env = Env::default();
    let player = Address::generate(&env);
    let seed = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(&env, &[1u8; 32]));
    
    env.budget().reset_unlimited();
    let _result = scan_nebula(env.clone(), seed, player);
    
    let cpu = env.budget().cpu_instruction_cost();
    
    assert!(cpu <= MAX_CPU_SCAN,
        "REGRESSION: Scan CPU {} exceeds baseline {}", cpu, MAX_CPU_SCAN);
}

#[test]
fn regression_mint_ship() {
    let env = Env::default();
    let player = Address::generate(&env);
    
    env.budget().reset_unlimited();
    let _ship = mint_ship(
        env.clone(),
        player,
        soroban_sdk::symbol_short!("fighter"),
        soroban_sdk::Bytes::new(&env),
    );
    
    let cpu = env.budget().cpu_instruction_cost();
    
    assert!(cpu <= MAX_CPU_MINT,
        "REGRESSION: Mint CPU {} exceeds baseline {}", cpu, MAX_CPU_MINT);
}

#[test]
fn regression_batch_efficiency() {
    let env = Env::default();
    let player = Address::generate(&env);
    
    // Single operation
    env.budget().reset_unlimited();
    let _ship1 = mint_ship(
        env.clone(),
        player.clone(),
        soroban_sdk::symbol_short!("fighter"),
        soroban_sdk::Bytes::new(&env),
    );
    let single_cpu = env.budget().cpu_instruction_cost();
    
    // Batch operation (3 ships)
    let ship_types = soroban_sdk::vec![
        &env,
        soroban_sdk::symbol_short!("fighter"),
        soroban_sdk::symbol_short!("miner"),
        soroban_sdk::symbol_short!("scout"),
    ];
    
    env.budget().reset_unlimited();
    let _ships = batch_mint_ships(
        env.clone(),
        player,
        ship_types,
        soroban_sdk::Bytes::new(&env),
    );
    let batch_cpu = env.budget().cpu_instruction_cost();
    
    // Batch should be more efficient than 3x single
    let efficiency_ratio = (batch_cpu as f64) / (single_cpu as f64 * 3.0);
    
    assert!(efficiency_ratio < 0.85,
        "REGRESSION: Batch efficiency {} should be < 0.85", efficiency_ratio);
}

#[test]
fn regression_storage_bump_cost() {
    let env = Env::default();
    let player = Address::generate(&env);
    
    let profile_id = initialize_profile(env.clone(), player.clone()).unwrap();
    
    env.budget().reset_unlimited();
    update_progress(env.clone(), player, profile_id, 1, 100).unwrap();
    
    let cpu = env.budget().cpu_instruction_cost();
    
    // Storage operations should be optimized
    assert!(cpu <= 500_000,
        "REGRESSION: Storage update CPU {} exceeds 500K", cpu);
}
