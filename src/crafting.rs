use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};
use crate::recipes::{get_recipe, unlock_rare_recipe};

#[soroban_sdk::contracttype]
pub enum CraftingDataKey {
    PlayerLevel(Address),
    PlayerXP(Address),
}

pub fn craft(env: Env, player: Address, recipe_id: u32) {
    player.require_auth();
    let recipe = get_recipe(&env, recipe_id);

    // Skill check
    let level = get_level(&env, player.clone());
    if level < recipe.required_level {
        panic!("Insufficient level");
    }

    require_resources(&env, &player, &recipe.inputs);
    consume_resources(&env, &player, &recipe.inputs);

    mint_resource(&env, &player, recipe.output);

    // Add XP: 10 XP per craft base + rarity bonus
    let xp_gain = 10 + (recipe.rarity as u32 * 5);
    add_xp(&env, player.clone(), xp_gain);

    // Rare discovery chance (5%)
    let mut rand_bytes = [0u8; 32];
    env.prng().fill_bytes(&mut rand_bytes);
    let random = rand_bytes[0] as u32;
    if random % 100 < 5 {
        // In a real scenario, we'd pick a random rare recipe ID
        unlock_rare_recipe(&env, player.clone(), 999);
        env.events().publish(
            (symbol_short!("rare_dis"), player.clone()),
            symbol_short!("unlocked"),
        );
    }
}

pub fn add_xp(env: &Env, player: Address, xp: u32) {
    let current_xp = get_xp(env, player.clone());
    let new_xp = current_xp + xp;
    
    let old_level = get_level(env, player.clone());
    let new_level = 1 + (new_xp / 100); // 100 XP per level

    env.storage().persistent().set(&CraftingDataKey::PlayerXP(player.clone()), &new_xp);
    
    if new_level > old_level {
        env.storage().persistent().set(&CraftingDataKey::PlayerLevel(player.clone()), &new_level);
        env.events().publish(
            (symbol_short!("levelup"), player),
            new_level,
        );
    }
}

pub fn get_level(env: &Env, player: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::PlayerLevel(player))
        .unwrap_or(1)
}

pub fn get_xp(env: &Env, player: Address) -> u32 {
    env.storage()
        .persistent()
        .get(&CraftingDataKey::PlayerXP(player))
        .unwrap_or(0)
}

fn require_resources(env: &Env, player: &Address, inputs: &Vec<(Symbol, u32)>) {
    for input in inputs.iter() {
        let (symbol, required) = input;
        let balance = get_resource_balance(env, player, symbol);
        if balance < required {
            panic!("Insufficient resources");
        }
    }
}

fn consume_resources(env: &Env, player: &Address, inputs: &Vec<(Symbol, u32)>) {
    for input in inputs.iter() {
        let (symbol, amount) = input;
        let balance = get_resource_balance(env, player, symbol);
        set_resource_balance(env, player, symbol, balance - amount);
    }
}

fn mint_resource(env: &Env, player: &Address, output: (Symbol, u32)) {
    let (symbol, amount) = output;
    let balance = get_resource_balance(env, player, symbol.clone());
    set_resource_balance(env, player, symbol, balance + amount);
    
    env.events().publish(
        (symbol_short!("crafted"), player.clone()),
        amount,
    );
}

// Helper to interact with the existing resource system or a simplified one
fn get_resource_balance(env: &Env, player: &Address, symbol: Symbol) -> u32 {
    // Using the key format from resource_minter.rs for compatibility
    // ResourceKey::ResourceBalance(Address, AssetId)
    // We'll wrap it in a local enum if we can't import it easily, 
    // but for simplicity we'll just use a raw key or follow the pattern.
    
    // Pattern: (Symbol("res_bal"), Address, Symbol)
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().get(&key).unwrap_or(0)
}

fn set_resource_balance(env: &Env, player: &Address, symbol: Symbol, amount: u32) {
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().set(&key, &amount);
}
