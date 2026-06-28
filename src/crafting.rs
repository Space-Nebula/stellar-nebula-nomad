use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};
use crate::recipes::{get_recipe, is_rare, is_unlocked, unlock_rare_recipe};

#[soroban_sdk::contracttype]
pub enum CraftingDataKey {
    PlayerLevel(Address),
    PlayerXP(Address),
}

pub fn craft(env: Env, player: Address, recipe_id: u32) {
    player.require_auth();
    let recipe = get_recipe(&env, recipe_id);

    // Unlock gate: rare recipes require prior unlock
    if is_rare(&recipe) && !is_unlocked(&env, &player, recipe_id) {
        panic!("Recipe locked");
    }

    // Skill check
    let level = get_level(&env, player.clone());
    if level < recipe.required_level {
        panic!("Insufficient level");
    }

    require_resources(&env, &player, &recipe.inputs);
    consume_resources(&env, &player, &recipe.inputs);

    mint_resource(&env, &player, recipe.output);

    // Add XP: 10 XP per craft base + rarity bonus
    let xp_gain = 10 + (recipe.rarity * 5);
    add_xp(&env, player.clone(), xp_gain);

    // Rare discovery chance (5%)
    let random: u64 = env.prng().gen();
    if random % 100 < 5 {
        // Unlock recipe id 999 as the discovered rare recipe
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
        let balance = get_resource_balance(env, player, symbol.clone());
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

// Resource balance helpers using the (Symbol("res_bal"), Address, Symbol) key format.
fn get_resource_balance(env: &Env, player: &Address, symbol: Symbol) -> u32 {
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().get(&key).unwrap_or(0)
}

fn set_resource_balance(env: &Env, player: &Address, symbol: Symbol, amount: u32) {
    let key = (symbol_short!("res_bal"), player.clone(), symbol);
    env.storage().instance().set(&key, &amount);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol, Vec};
    use crate::recipes::{self, Recipe};

    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    /// Seed a resource balance directly into instance storage.
    fn seed_resource(env: &Env, player: &Address, sym: Symbol, amount: u32) {
        let key = (symbol_short!("res_bal"), player.clone(), sym);
        env.storage().instance().set(&key, &amount);
    }

    /// Build a minimal Recipe with given id, rarity, and a single input/output pair.
    fn make_recipe(env: &Env, id: u32, rarity: u32, input: Symbol, output: Symbol) -> Recipe {
        let mut inputs = Vec::new(env);
        inputs.push_back((input, 5u32));
        Recipe {
            id,
            inputs,
            output: (output, 1u32),
            rarity,
            required_level: 1,
        }
    }

    // (a) Crafting a common recipe succeeds when the player has resources + level.
    #[test]
    fn test_craft_common_recipe_succeeds() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let iron = Symbol::new(&env, "iron");
        let steel = Symbol::new(&env, "steel");

        env.mock_all_auths();
        env.as_contract(&id, || {
            // Register recipe (rarity 1 = common)
            let recipe = make_recipe(&env, 1, 1, iron.clone(), steel.clone());
            recipes::set_recipe(&env, &recipe);

            // Seed enough resources
            seed_resource(&env, &player, iron.clone(), 10);

            // Craft should succeed
            craft(env.clone(), player.clone(), 1);

            // Output resource should have been minted
            let key = (symbol_short!("res_bal"), player.clone(), steel.clone());
            let out_bal: u32 = env.storage().instance().get(&key).unwrap_or(0);
            assert_eq!(out_bal, 1);
        });
    }

    // (b) Crafting a locked rare recipe (rarity >= 3) panics with "Recipe locked".
    #[test]
    #[should_panic(expected = "Recipe locked")]
    fn test_craft_locked_rare_panics() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let crystal = Symbol::new(&env, "crystal");
        let gem = Symbol::new(&env, "gem");

        env.mock_all_auths();
        env.as_contract(&id, || {
            // Register rare recipe (rarity 3)
            let recipe = make_recipe(&env, 10, 3, crystal.clone(), gem.clone());
            recipes::set_recipe(&env, &recipe);

            // Seed resources
            seed_resource(&env, &player, crystal.clone(), 10);

            // This should panic: recipe is rare and NOT unlocked
            craft(env.clone(), player.clone(), 10);
        });
    }

    // (c) After unlock_rare_recipe, crafting the same rare recipe succeeds.
    #[test]
    fn test_craft_after_unlock_succeeds() {
        let (env, id) = make_env();
        let player = Address::generate(&env);
        let crystal = Symbol::new(&env, "crystal");
        let gem = Symbol::new(&env, "gem");

        env.mock_all_auths();
        env.as_contract(&id, || {
            // Register rare recipe (rarity 3)
            let recipe = make_recipe(&env, 10, 3, crystal.clone(), gem.clone());
            recipes::set_recipe(&env, &recipe);

            // Unlock the recipe for the player
            recipes::unlock_rare_recipe(&env, player.clone(), 10);

            // Seed resources
            seed_resource(&env, &player, crystal.clone(), 10);

            // Craft should now succeed
            craft(env.clone(), player.clone(), 10);

            let key = (symbol_short!("res_bal"), player.clone(), gem.clone());
            let out_bal: u32 = env.storage().instance().get(&key).unwrap_or(0);
            assert_eq!(out_bal, 1);
        });
    }
}
