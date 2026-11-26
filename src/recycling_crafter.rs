use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};
/// Maximum batch size for recycle/craft operations.
pub const RECYCLE_CRAFT_BATCH_SIZE: u32 = 8;
/// Basis points denominator (100% = 10_000 bps).
pub const RECYCLING_BPS_DENOM: u32 = 10_000;
/// Maximum player recycling efficiency bonus (25%).
pub const MAX_EFFICIENCY_BONUS_BPS: u32 = 2_500;
/// Efficiency bonus gained per recycle action (1%).
pub const EFFICIENCY_STEP_BPS: u32 = 100;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum RecyclingKey {
    /// Recipe data keyed by recipe_id.
    Recipe(u64),
    /// Auto-incrementing recipe ID counter.
    RecipeCounter,
    /// Player-specific crafting stats (for efficiency multipliers & metrics).
    RecyclingPlayerStats(Address),
    /// Global environmental / sustainability metrics.
    GlobalMetrics,
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RecyclingError {
    /// Recipe does not exist.
    InvalidRecipe = 1,
    /// Inputs do not match recipe requirements.
    InvalidInputs = 2,
    /// Batch size exceeds limit.
    BatchTooLarge = 3,
    /// Insufficient resources to craft.
    InsufficientResources = 4,
    /// Crafted item already exists (idempotency guard).
    AlreadyCrafted = 5,
}

impl crate::error_standard::StandardContractError for RecyclingError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::InvalidRecipe | Self::InvalidInputs => (ErrorKind::Validation, false),
            Self::BatchTooLarge | Self::InsufficientResources => (ErrorKind::ResourceLimit, false),
            Self::AlreadyCrafted => (ErrorKind::Conflict, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "recycling_crafter",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Recipe {
    pub id: u64,
    pub inputs: Vec<Symbol>,
    pub outputs: Vec<Symbol>,
    pub input_quantities: Vec<u32>,
    pub output_quantities: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CraftingResult {
    pub recipe_id: u64,
    pub inputs_consumed: Vec<Symbol>,
    pub outputs_produced: Vec<Symbol>,
    pub output_quantities: Vec<u32>,
    pub efficiency_bonus_bps: u32,
    pub crafted_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RecyclingStats {
    pub total_recycled: u64,
    pub items_crafted: u64,
    pub materials_recovered: u64,
    pub carbon_offset_units: u64,
    pub efficiency_bonus_bps: u32,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SustainabilityMetrics {
    pub total_recycled: u64,
    pub total_materials_recovered: u64,
    pub total_carbon_offset: u64,
}

/// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the recipe library at deployment.
pub fn initialize_recycling(env: &Env) {
    if !env.storage().instance().has(&RecyclingKey::RecipeCounter) {
        env.storage()
            .instance()
            .set(&RecyclingKey::RecipeCounter, &0u64);
        // Load default recipes
        load_default_recipes(env);
    }
}

/// Load a small set of default recipes (placeholder).
fn load_default_recipes(env: &Env) {
    // Recipe 1: Recycle 2 ore -> 1 dust
    add_recipe(
        env,
        Vec::from_array(env, [symbol_short!("ore")]),
        Vec::from_array(env, [symbol_short!("dust")]),
        Vec::from_array(env, [2]),
        Vec::from_array(env, [1]),
    );
    // Recipe 2: Recycle 1 dust + 1 gas -> 1 exotic
    add_recipe(
        env,
        Vec::from_array(env, [symbol_short!("dust"), symbol_short!("gas")]),
        Vec::from_array(env, [symbol_short!("exotic")]),
        Vec::from_array(env, [1, 1]),
        Vec::from_array(env, [1]),
    );
}

/// Internal helper to add a recipe.
fn add_recipe(
    env: &Env,
    inputs: Vec<Symbol>,
    outputs: Vec<Symbol>,
    input_qts: Vec<u32>,
    output_qts: Vec<u32>,
) -> u64 {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&RecyclingKey::RecipeCounter)
        .unwrap_or(0);
    let next_id = counter + 1;
    env.storage()
        .instance()
        .set(&RecyclingKey::RecipeCounter, &next_id);

    let recipe = Recipe {
        id: next_id,
        inputs,
        outputs,
        input_quantities: input_qts,
        output_quantities: output_qts,
    };

    env.storage()
        .instance()
        .set(&RecyclingKey::Recipe(next_id), &recipe);

    next_id
}

/// Recycle a resource into base materials with player efficiency bonus and
/// environmental sustainability tracking.
pub fn recycle_resource(
    env: &Env,
    caller: &Address,
    resource: Symbol,
    amount: u32,
) -> Result<Vec<(Symbol, u32)>, RecyclingError> {
    caller.require_auth();

    let base_recovery = amount / 2;
    if base_recovery == 0 {
        return Err(RecyclingError::InsufficientResources);
    }

    let mut stats = get_player_recycling_stats(env, caller);
    let bonus = ((base_recovery as u64).saturating_mul(stats.efficiency_bonus_bps as u64)
        / RECYCLING_BPS_DENOM as u64) as u32;
    let recovered = base_recovery.saturating_add(bonus);
    let carbon_offset = (amount as u64).saturating_mul(2);

    stats.total_recycled = stats.total_recycled.saturating_add(amount as u64);
    stats.materials_recovered = stats.materials_recovered.saturating_add(recovered as u64);
    stats.carbon_offset_units = stats.carbon_offset_units.saturating_add(carbon_offset);
    stats.efficiency_bonus_bps = stats
        .efficiency_bonus_bps
        .saturating_add(EFFICIENCY_STEP_BPS)
        .min(MAX_EFFICIENCY_BONUS_BPS);
    env.storage()
        .persistent()
        .set(&RecyclingKey::RecyclingPlayerStats(caller.clone()), &stats);

    let mut global = get_sustainability_metrics(env);
    global.total_recycled = global.total_recycled.saturating_add(amount as u64);
    global.total_materials_recovered = global
        .total_materials_recovered
        .saturating_add(recovered as u64);
    global.total_carbon_offset = global.total_carbon_offset.saturating_add(carbon_offset);
    env.storage()
        .instance()
        .set(&RecyclingKey::GlobalMetrics, &global);

    // Emit ResourceRecycled event
    env.events().publish(
        (symbol_short!("recycle"), symbol_short!("resource")),
        (
            caller.clone(),
            resource,
            amount,
            symbol_short!("dust"),
            recovered,
            env.ledger().timestamp(),
        ),
    );

    Ok(Vec::from_array(env, [(symbol_short!("dust"), recovered)]))
}

/// Craft a new item using a recipe, applying the player's integer-safe
/// recycling efficiency bonus to output quantities.
pub fn craft_new_item(
    env: &Env,
    caller: &Address,
    recipe_id: u64,
    inputs: Vec<Symbol>,
    quantities: Vec<u32>,
) -> Result<CraftingResult, RecyclingError> {
    caller.require_auth();

    if (inputs.len() as u32) > RECYCLE_CRAFT_BATCH_SIZE {
        return Err(RecyclingError::BatchTooLarge);
    }

    let recipe: Recipe = env
        .storage()
        .instance()
        .get(&RecyclingKey::Recipe(recipe_id))
        .ok_or(RecyclingError::InvalidRecipe)?;

    // Validate inputs match recipe
    if inputs.len() != recipe.inputs.len() || quantities.len() != recipe.input_quantities.len() {
        return Err(RecyclingError::InvalidInputs);
    }
    for i in 0..inputs.len() {
        if inputs.get(i).unwrap() != recipe.inputs.get(i).unwrap()
            || quantities.get(i).unwrap() != recipe.input_quantities.get(i).unwrap()
        {
            return Err(RecyclingError::InvalidInputs);
        }
    }

    let mut stats = get_player_recycling_stats(env, caller);
    let bonus_bps = stats.efficiency_bonus_bps;
    let mut final_outputs = Vec::new(env);
    let mut final_quantities = Vec::new(env);
    for i in 0..recipe.outputs.len() {
        let base_qty = recipe.output_quantities.get(i).unwrap();
        let bonus = ((base_qty as u64).saturating_mul(bonus_bps as u64)
            / RECYCLING_BPS_DENOM as u64) as u32;
        final_outputs.push_back(recipe.outputs.get(i).unwrap().clone());
        final_quantities.push_back(base_qty.saturating_add(bonus));
    }

    stats.items_crafted = stats.items_crafted.saturating_add(1);
    env.storage()
        .persistent()
        .set(&RecyclingKey::RecyclingPlayerStats(caller.clone()), &stats);

    let result = CraftingResult {
        recipe_id,
        inputs_consumed: inputs.clone(),
        outputs_produced: final_outputs.clone(),
        output_quantities: final_quantities,
        efficiency_bonus_bps: bonus_bps,
        crafted_at: env.ledger().timestamp(),
    };

    // Emit ItemCrafted event
    env.events().publish(
        (symbol_short!("craft"), symbol_short!("item")),
        (
            caller.clone(),
            recipe_id,
            inputs,
            final_outputs,
            result.crafted_at,
        ),
    );

    Ok(result)
}

/// Get recipe by ID.
pub fn get_recipe(env: &Env, recipe_id: u64) -> Option<Recipe> {
    env.storage()
        .instance()
        .get(&RecyclingKey::Recipe(recipe_id))
}

/// Get a player's recycling stats and current efficiency bonus.
pub fn get_player_recycling_stats(env: &Env, player: &Address) -> RecyclingStats {
    env.storage()
        .persistent()
        .get(&RecyclingKey::RecyclingPlayerStats(player.clone()))
        .unwrap_or(RecyclingStats {
            total_recycled: 0,
            items_crafted: 0,
            materials_recovered: 0,
            carbon_offset_units: 0,
            efficiency_bonus_bps: 0,
        })
}

/// Get global environmental / sustainability metrics for recycling.
pub fn get_sustainability_metrics(env: &Env) -> SustainabilityMetrics {
    env.storage()
        .instance()
        .get(&RecyclingKey::GlobalMetrics)
        .unwrap_or(SustainabilityMetrics {
            total_recycled: 0,
            total_materials_recovered: 0,
            total_carbon_offset: 0,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    #[test]
    fn test_recycling_efficiency_and_crafting() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Stub, ());
        let player = Address::generate(&env);

        env.as_contract(&contract_id, || {
            initialize_recycling(&env);

            // First recycle: 0% bonus -> 100 ore yields 50 dust, bonus grows to 100 bps
            let out1 = recycle_resource(&env, &player, symbol_short!("ore"), 100).unwrap();
            assert_eq!(out1.get(0).unwrap().1, 50);

            // Second recycle: 100 bps (1%) bonus -> 200 ore yields 100 + 1 = 101 dust
            let out2 = recycle_resource(&env, &player, symbol_short!("ore"), 200).unwrap();
            assert_eq!(out2.get(0).unwrap().1, 101);

            let stats = get_player_recycling_stats(&env, &player);
            assert_eq!(stats.total_recycled, 300);
            assert_eq!(stats.materials_recovered, 151);
            assert_eq!(stats.carbon_offset_units, 600);
            assert_eq!(stats.efficiency_bonus_bps, 200);

            let metrics = get_sustainability_metrics(&env);
            assert_eq!(metrics.total_recycled, 300);
            assert_eq!(metrics.total_carbon_offset, 600);

            // Craft using Recipe 1 (2 ore -> 1 dust)
            let crafted = craft_new_item(
                &env,
                &player,
                1,
                Vec::from_array(&env, [symbol_short!("ore")]),
                Vec::from_array(&env, [2]),
            )
            .unwrap();
            assert_eq!(crafted.recipe_id, 1);
            assert_eq!(crafted.efficiency_bonus_bps, 200);
            assert_eq!(get_player_recycling_stats(&env, &player).items_crafted, 1);
        });
    }
}
