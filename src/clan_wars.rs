//! Guild Wars System (Issue #530)
use soroban_sdk::{contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone)]
pub struct Territory {
    pub id: u32,
    pub name: String,
    pub owner: Option<Address>,
    pub resource_bonus: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct WarDeclaration {
    pub attacker: Address,
    pub defender: Address,
    pub stake: i128,
    pub start_time: u64,
    pub end_time: u64,
}

#[contracttype]
#[derive(Clone, Copy)]
pub enum BattleStrategy {
    Aggressive,
    Defensive,
    Balanced,
}

pub fn declare_war(
    env: &Env,
    attacker: &Address,
    defender: &Address,
    stake: i128,
) -> Result<(), String> {
    // Stub: Validate guilds exist, stake amount, create war
    if stake < 1000 {
        return Err(String::from_str(env, "Minimum stake is 1000"));
    }
    Ok(())
}

/// Resolve a battle from the two strategies. Mirrors are settled by a coin
/// flip from `rng`; see [`resolve_battle_with`].
pub fn resolve_battle(
    env: &Env,
    attacker: &Address,
    defender: &Address,
    attacker_strategy: BattleStrategy,
    defender_strategy: BattleStrategy,
) -> Address {
    resolve_battle_with(
        &crate::traits::RealRandomnessProvider::new(env),
        attacker,
        defender,
        attacker_strategy,
        defender_strategy,
    )
}

/// Strategy triangle: Aggressive beats Balanced, Balanced beats Defensive,
/// Defensive beats Aggressive. A mirror match is a fair coin flip taken from
/// the injected [`RandomnessProvider`], so tests can pin either outcome.
pub fn resolve_battle_with<R: crate::traits::RandomnessProvider>(
    rng: &R,
    attacker: &Address,
    defender: &Address,
    attacker_strategy: BattleStrategy,
    defender_strategy: BattleStrategy,
) -> Address {
    let attacker_wins = match (attacker_strategy, defender_strategy) {
        (BattleStrategy::Aggressive, BattleStrategy::Balanced)
        | (BattleStrategy::Balanced, BattleStrategy::Defensive)
        | (BattleStrategy::Defensive, BattleStrategy::Aggressive) => true,
        (BattleStrategy::Balanced, BattleStrategy::Aggressive)
        | (BattleStrategy::Defensive, BattleStrategy::Balanced)
        | (BattleStrategy::Aggressive, BattleStrategy::Defensive) => false,
        _ => rng.coin_flip(),
    };
    if attacker_wins {
        attacker.clone()
    } else {
        defender.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn mirror_matches_follow_the_injected_coin() {
        use crate::traits::MockRandomnessProvider;
        let env = Env::default();
        let a = Address::generate(&env);
        let d = Address::generate(&env);
        let heads = MockRandomnessProvider::fixed(1);
        let tails = MockRandomnessProvider::fixed(2);
        let s = BattleStrategy::Balanced;
        assert_eq!(resolve_battle_with(&heads, &a, &d, s, s), a);
        assert_eq!(resolve_battle_with(&tails, &a, &d, s, s), d);
        // The triangle never consults the coin.
        assert_eq!(
            resolve_battle_with(&tails, &a, &d, BattleStrategy::Aggressive, s),
            a
        );
        assert_eq!(
            resolve_battle_with(&heads, &a, &d, s, BattleStrategy::Aggressive),
            d
        );
    }

    #[test]
    fn test_declare_war_validates_stake() {
        let env = Env::default();
        let a = Address::generate(&env);
        let d = Address::generate(&env);
        assert!(declare_war(&env, &a, &d, 500).is_err());
        assert!(declare_war(&env, &a, &d, 1000).is_ok());
    }
}
