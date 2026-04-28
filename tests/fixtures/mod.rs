use soroban_sdk::{symbol_short, Env, Vec};
use stellar_nebula_nomad::Recipe;

pub fn get_mock_recipe(env: &Env) -> Recipe {
    Recipe {
        id: 1,
        inputs: Vec::from_array(env, [(symbol_short!("ore"), 10)]),
        output: (symbol_short!("steel"), 1),
        rarity: 1,
        required_level: 1,
    }
}
