use soroban_sdk::{contracterror, contracttype, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecipeError {
    RecipeNotFound = 1,
}

// ── Rare rarity threshold ─────────────────────────────────────────────────────

/// Recipes with rarity >= this value are considered rare and require an unlock.
pub const RARE_RARITY_THRESHOLD: u32 = 3;

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: u32,
    pub inputs: Vec<(Symbol, u32)>,
    pub output: (Symbol, u32),
    /// Rarity tier (1 = common, 2 = uncommon, 3+ = rare). u32 for contracttype compat.
    pub rarity: u32,
    pub required_level: u32,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
pub enum RecipeKey {
    Recipe(u32),
    PlayerRareUnlocked(Address, u32),
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns true if the recipe's rarity meets or exceeds RARE_RARITY_THRESHOLD.
pub fn is_rare(recipe: &Recipe) -> bool {
    recipe.rarity >= RARE_RARITY_THRESHOLD
}

/// Returns true if the player has unlocked the given rare recipe.
pub fn is_unlocked(env: &Env, player: &Address, recipe_id: u32) -> bool {
    env.storage()
        .instance()
        .get(&RecipeKey::PlayerRareUnlocked(player.clone(), recipe_id))
        .unwrap_or(false)
}

// ── CRUD ──────────────────────────────────────────────────────────────────────

pub fn get_recipe(env: &Env, id: u32) -> Result<Recipe, RecipeError> {
    env.storage()
        .instance()
        .get(&RecipeKey::Recipe(id))
        .ok_or(RecipeError::RecipeNotFound)
}

pub fn set_recipe(env: &Env, recipe: &Recipe) {
    env.storage()
        .instance()
        .set(&RecipeKey::Recipe(recipe.id), recipe);
}

pub fn unlock_rare_recipe(env: &Env, player: Address, recipe_id: u32) {
    env.storage()
        .instance()
        .set(&RecipeKey::PlayerRareUnlocked(player, recipe_id), &true);
}
