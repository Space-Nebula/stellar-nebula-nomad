use soroban_sdk::{contracttype, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: u32,
    pub inputs: Vec<(Symbol, u32)>,
    pub output: (Symbol, u32),
    pub rarity: u8,
    pub required_level: u32,
}

#[contracttype]
pub enum RecipeKey {
    Recipe(u32),
    PlayerRareUnlocked(soroban_sdk::Address, u32),
}

pub fn get_recipe(env: &Env, id: u32) -> Recipe {
    env.storage()
        .instance()
        .get(&RecipeKey::Recipe(id))
        .expect("Recipe not found")
}

pub fn set_recipe(env: &Env, recipe: &Recipe) {
    env.storage()
        .instance()
        .set(&RecipeKey::Recipe(recipe.id), recipe);
}

pub fn unlock_rare_recipe(env: &Env, player: soroban_sdk::Address, recipe_id: u32) {
    env.storage()
        .instance()
        .set(&RecipeKey::PlayerRareUnlocked(player, recipe_id), &true);
}
