// Example: Nomad Bonding System — Multi-Sig Co-op Sharing
//
// This example shows how two players form a bond, delegate
// passive yields, and claim shared cosmic essence.

fn main() {
    println!("=== Stellar Nebula Nomad: Nomad Bonding Example ===");
    println!();
    println!("Flow:");
    println!("  1. Alice calls create_bond(ship_id=42, partner=Bob)");
    println!("     → Bond #1 created in Pending state");
    println!("  2. Bob calls accept_bond(bond_id=1)");
    println!("     → Bond transitions to Active");
    println!("  3. Alice calls delegate_yield(bond_id=1, percentage=40)");
    println!("     → 40% of Alice's future cosmic essence goes to Bob");
    println!("  4. Alice explores nebulas and earns 2000 cosmic essence");
    println!("  5. Bob calls claim_yield(bond_id=1)");
    println!("     → Bob receives 800 (40% of 2000), Alice keeps 1200");
    println!("  6. Either party can dissolve_bond(bond_id=1) at any time");
    println!("     → Future claims are blocked; existing balances remain");
    println!();
    println!("Security guarantees:");
    println!("  - Only the designated partner can accept a bond");
    println!("  - Only bonded addresses can set up yield delegation");
    println!("  - Only the beneficiary can claim delegated yields");
    println!("  - Outsiders cannot dissolve or interact with a bond");
    println!();
    println!("CLI invocation:");
    println!("  soroban contract invoke --id CONTRACT_ID \\");
    println!("    --fn create_bond \\");
    println!("    --arg '\"GALICE...\"' --arg 42 --arg '\"GBOB...\"'");
}
