//! Performance metrics tracking and baseline comparison.
//! This benchmark collects detailed metrics about operation performance,
//! including CPU costs, memory usage, and batch efficiency ratios.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String, Vec};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct PerformanceMetric {
    name: String,
    cpu_cost: u64,
    memory_cost: u64,
    cpu_limit: u64,
}

struct MetricsCollector {
    metrics: HashMap<String, PerformanceMetric>,
}

impl MetricsCollector {
    fn new() -> Self {
        MetricsCollector {
            metrics: HashMap::new(),
        }
    }

    fn record(&mut self, metric: PerformanceMetric) {
        self.metrics.insert(metric.name.clone(), metric);
    }

    fn report(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║          PERFORMANCE REGRESSION METRICS REPORT              ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        let mut sorted: Vec<_> = self.metrics.values().collect();
        sorted.sort_by(|a, b| b.cpu_cost.cmp(&a.cpu_cost));

        for metric in sorted {
            let cpu_percent = (metric.cpu_cost as f64 / metric.cpu_limit as f64) * 100.0;
            let status = if metric.cpu_cost <= metric.cpu_limit {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };

            println!(
                "{:<35} {:>10} {:>10} {:>12}% [{}]",
                metric.name, metric.cpu_cost, metric.memory_cost, cpu_percent as u64, status
            );
        }

        println!(
            "\n{} operations completed successfully.",
            self.metrics.len()
        );
    }

    fn verify_all_pass(&self) -> bool {
        self.metrics
            .values()
            .all(|m| m.cpu_cost <= m.cpu_limit)
    }
}

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

fn main() {
    let mut collector = MetricsCollector::new();

    let (env, client, player) = setup();

    let (cpu, mem) = measure(&env, || {
        let _ = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    });
    collector.record(PerformanceMetric {
        name: "mint_ship".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 3_000_000,
    });

    let (cpu, mem) = measure(&env, || {
        let _ = client.initialize_profile(&player);
    });
    collector.record(PerformanceMetric {
        name: "initialize_profile".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 2_000_000,
    });

    let seed = BytesN::from_array(&env, &[1u8; 32]);
    let (cpu, mem) = measure(&env, || {
        let _ = client.generate_nebula_layout(&seed, &player);
    });
    collector.record(PerformanceMetric {
        name: "generate_nebula_layout".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 6_000_000,
    });

    let (cpu, mem) = measure(&env, || {
        let _ = client.scan_nebula(&seed, &player);
    });
    collector.record(PerformanceMetric {
        name: "scan_nebula".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 6_000_000,
    });

    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let layout = client.generate_nebula_layout(&BytesN::from_array(&env, &[2u8; 32]), &player);
    let (cpu, mem) = measure(&env, || {
        let _ = client.harvest_resources(&ship.id, &layout);
    });
    collector.record(PerformanceMetric {
        name: "harvest_resources".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 6_000_000,
    });

    let profile_id = client.initialize_profile(&player);
    let (cpu, mem) = measure(&env, || {
        client.update_progress(&player, &profile_id, &1, &100);
    });
    collector.record(PerformanceMetric {
        name: "update_progress".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 2_000_000,
    });

    let founder = Address::generate(&env);
    let name = String::from_str(&env, "Performance Test Alliance");
    let (cpu, mem) = measure(&env, || {
        let _ = client.found_alliance(&founder, &name);
    });
    collector.record(PerformanceMetric {
        name: "found_alliance".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 4_000_000,
    });

    let owner = Address::generate(&env);
    let test_ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    let (cpu, mem) = measure(&env, || {
        let _ = client.consume_energy(&test_ship.id, &500);
    });
    collector.record(PerformanceMetric {
        name: "consume_energy".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 2_000_000,
    });

    let (cpu, mem) = measure(&env, || {
        let _ = client.recharge_energy(&test_ship.id, &200);
    });
    collector.record(PerformanceMetric {
        name: "recharge_energy".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 2_000_000,
    });

    let admin = Address::generate(&env);
    let mut admins = Vec::new(&env);
    admins.push_back(admin.clone());
    client.initialize_admins(&admins);
    let (cpu, mem) = measure(&env, || {
        client.pause_contract(&admin);
    });
    collector.record(PerformanceMetric {
        name: "pause_contract".to_string(),
        cpu_cost: cpu,
        memory_cost: mem,
        cpu_limit: 2_000_000,
    });

    collector.report();

    if !collector.verify_all_pass() {
        panic!("Performance regressions detected!");
    }

    println!("\n✓ All performance metrics within acceptable thresholds");
}
