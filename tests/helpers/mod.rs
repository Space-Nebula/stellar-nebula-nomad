use soroban_sdk::{Address, Env};

pub fn mock_player(env: &Env) -> Address {
    Address::generate(env)
}

pub fn setup_test_env() -> (Env, Address) {
    let env = Env::default();
    let player = mock_player(&env);
    (env, player)
}
