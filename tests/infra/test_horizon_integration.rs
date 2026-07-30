#![cfg(test)]

//! Integration tests for the Stellar Horizon API client (Issue #203).
//!
//! Covers:
//!   - Horizon account-info queries
//!   - Transaction submission and event emission
//!   - Event-stream subscription and cancellation

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{Env, IntoVal, String};
use stellar_nebula_nomad::integrations::horizon_client::{
    emit_tx_for_indexing, query_account_info, stream_events, submit_transaction,
    unsubscribe_stream, EventFilter,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn fresh_env() -> Env {
    Env::default()
}

// ── Account-info queries ──────────────────────────────────────────────────────

#[test]
fn horizon_account_query_returns_correct_address() {
    let env = fresh_env();
    let addr = soroban_sdk::Address::generate(&env);
    let info = query_account_info(&env, &addr);
    assert_eq!(info.address, addr);
}

#[test]
fn horizon_account_query_defaults_to_zero_balance_and_sequence() {
    let env = fresh_env();
    let addr = soroban_sdk::Address::generate(&env);
    let info = query_account_info(&env, &addr);
    assert_eq!(info.balance, 0);
    assert_eq!(info.sequence, 0);
}

#[test]
fn horizon_account_query_emits_horizon_query_event() {
    let env = fresh_env();
    let addr = soroban_sdk::Address::generate(&env);
    query_account_info(&env, &addr);

    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (topics, _) = events.get(0).unwrap();
    assert_eq!(
        topics.get(0).unwrap(),
        soroban_sdk::symbol_short!("horizon").into_val(&env),
        "first topic must be 'horizon'"
    );
    assert_eq!(
        topics.get(1).unwrap(),
        soroban_sdk::symbol_short!("query").into_val(&env),
        "second topic must be 'query'"
    );
}

#[test]
fn horizon_multiple_account_queries_each_emit_one_event() {
    let env = fresh_env();
    let a = soroban_sdk::Address::generate(&env);
    let b = soroban_sdk::Address::generate(&env);
    query_account_info(&env, &a);
    query_account_info(&env, &b);
    assert_eq!(env.events().all().len(), 2);
}

// ── Transaction submission ────────────────────────────────────────────────────

#[test]
fn horizon_submit_transaction_accepted() {
    let env = fresh_env();
    let result = submit_transaction(
        &env,
        String::from_str(&env, "hash_abc"),
        String::from_str(&env, "scan_nebula"),
    );
    assert!(result.accepted, "transaction should be marked accepted");
}

#[test]
fn horizon_submit_transaction_preserves_hash_and_operation() {
    let env = fresh_env();
    let hash = String::from_str(&env, "0xdeadbeef");
    let op = String::from_str(&env, "mint_ship");
    let result = submit_transaction(&env, hash.clone(), op.clone());
    assert_eq!(result.tx_hash, hash);
    assert_eq!(result.operation, op);
}

#[test]
fn horizon_submit_transaction_emits_tx_event() {
    let env = fresh_env();
    submit_transaction(
        &env,
        String::from_str(&env, "cafebabe"),
        String::from_str(&env, "harvest"),
    );
    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (topics, _) = events.get(0).unwrap();
    assert_eq!(
        topics.get(1).unwrap(),
        soroban_sdk::symbol_short!("tx").into_val(&env)
    );
}

#[test]
fn horizon_emit_tx_for_indexing_compat_wrapper() {
    let env = fresh_env();
    emit_tx_for_indexing(
        &env,
        String::from_str(&env, "abc"),
        String::from_str(&env, "op"),
    );
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn horizon_submit_multiple_transactions_emits_multiple_events() {
    let env = fresh_env();
    for i in 0u32..5 {
        let hash = String::from_str(&env, "tx");
        let _ = i;
        submit_transaction(&env, hash, String::from_str(&env, "op"));
    }
    assert_eq!(env.events().all().len(), 5);
}

// ── Event streaming ───────────────────────────────────────────────────────────

#[test]
fn horizon_stream_events_emits_stream_event() {
    let env = fresh_env();
    let sub = soroban_sdk::Address::generate(&env);
    let filter = EventFilter {
        topic: String::from_str(&env, "horizon"),
        sub_topic: String::from_str(&env, "tx"),
    };
    stream_events(&env, &sub, filter);

    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (topics, _) = events.get(0).unwrap();
    assert_eq!(
        topics.get(1).unwrap(),
        soroban_sdk::symbol_short!("stream").into_val(&env)
    );
}

#[test]
fn horizon_stream_with_wildcard_sub_topic_succeeds() {
    let env = fresh_env();
    let sub = soroban_sdk::Address::generate(&env);
    let filter = EventFilter {
        topic: String::from_str(&env, "horizon"),
        sub_topic: String::from_str(&env, ""),
    };
    stream_events(&env, &sub, filter);
    assert_eq!(env.events().all().len(), 1);
}

#[test]
fn horizon_unsubscribe_emits_unsub_event() {
    let env = fresh_env();
    let sub = soroban_sdk::Address::generate(&env);
    unsubscribe_stream(&env, &sub);

    let events = env.events().all();
    assert_eq!(events.len(), 1);
    let (topics, _) = events.get(0).unwrap();
    assert_eq!(
        topics.get(1).unwrap(),
        soroban_sdk::symbol_short!("unsub").into_val(&env)
    );
}

#[test]
fn horizon_subscribe_then_unsubscribe_emits_two_events() {
    let env = fresh_env();
    let sub = soroban_sdk::Address::generate(&env);
    let filter = EventFilter {
        topic: String::from_str(&env, "horizon"),
        sub_topic: String::from_str(&env, "tx"),
    };
    stream_events(&env, &sub, filter);
    unsubscribe_stream(&env, &sub);
    assert_eq!(env.events().all().len(), 2);
}

// ── Full workflow ─────────────────────────────────────────────────────────────

#[test]
fn horizon_full_workflow_query_submit_stream() {
    let env = fresh_env();
    let player = soroban_sdk::Address::generate(&env);

    // 1. Query account info.
    let info = query_account_info(&env, &player);
    assert_eq!(info.address, player);

    // 2. Submit a transaction.
    let result = submit_transaction(
        &env,
        String::from_str(&env, "workflow_tx"),
        String::from_str(&env, "nebula_scan"),
    );
    assert!(result.accepted);

    // 3. Subscribe to event stream.
    let filter = EventFilter {
        topic: String::from_str(&env, "horizon"),
        sub_topic: String::from_str(&env, ""),
    };
    stream_events(&env, &player, filter);

    // One event per operation.
    assert_eq!(env.events().all().len(), 3);
}
