//! Guild tax/tribute (#650), bounty refunds (#648), yield risk (#652) and
//! fractional micro-trading (#653).
#![allow(clippy::pedantic)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Symbol,
};
use stellar_nebula_nomad::*;

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(NebulaNomadContract, ());
    (env, id)
}

/// Runs one call in its own contract frame (each frame allows one auth per address).
macro_rules! c {
    ($env:expr, $id:expr, $e:expr) => {
        $env.as_contract(&$id, || $e)
    };
}

// ─── #650 guild tax / tribute ────────────────────────────────────────────────

#[test]
fn split_guild_tribute_rounds_tax_down_and_conserves_the_total() {
    assert_eq!(split_guild_tribute(1_000, 250), Some((25, 975)));
    assert_eq!(split_guild_tribute(999, 1), Some((0, 999)));
    assert_eq!(split_guild_tribute(100, 10_001), None);
    assert_eq!(split_guild_tribute(-1, 100), None);
}

#[test]
fn tribute_tax_goes_to_the_treasury() {
    let (env, id) = setup();
    let founder = Address::generate(&env);
    let member = Address::generate(&env);
    {
        let alliance = c!(
            env,
            id,
            found_alliance(&env, founder.clone(), String::from_str(&env, "Orion"))
        )
        .unwrap();
        c!(env, id, join_alliance(&env, alliance, member.clone())).unwrap();
        let before = c!(env, id, get_alliance_treasury(&env, alliance));

        // no tax set yet: everything stays with the member
        let r = c!(
            env,
            id,
            pay_guild_tribute(&env, member.clone(), alliance, 1_000)
        )
        .unwrap();
        assert_eq!((r.tax, r.net), (0, 1_000));

        assert_eq!(
            c!(
                env,
                id,
                set_guild_tax_rate(&env, member.clone(), alliance, 500)
            ),
            Err(GuildEconomyError::NotFounder)
        );
        assert_eq!(
            c!(
                env,
                id,
                set_guild_tax_rate(&env, founder.clone(), alliance, MAX_GUILD_TAX_BPS + 1)
            ),
            Err(GuildEconomyError::InvalidTax)
        );
        c!(
            env,
            id,
            set_guild_tax_rate(&env, founder.clone(), alliance, 1_000)
        )
        .unwrap();
        assert_eq!(c!(env, id, get_guild_tax_rate(&env, alliance)), 1_000);

        let r = c!(
            env,
            id,
            pay_guild_tribute(&env, member.clone(), alliance, 2_000)
        )
        .unwrap();
        assert_eq!((r.tax, r.net, r.tax_bps), (200, 1_800, 1_000));
        assert_eq!(
            c!(env, id, get_alliance_treasury(&env, alliance)),
            before + 200
        );
        assert_eq!(r.treasury_after, before + 200);

        let outsider = Address::generate(&env);
        assert_eq!(
            c!(env, id, pay_guild_tribute(&env, outsider, alliance, 100)),
            Err(GuildEconomyError::NotAllianceMember)
        );
        assert_eq!(
            c!(env, id, pay_guild_tribute(&env, member, alliance, 0)),
            Err(GuildEconomyError::InvalidTax)
        );
    }
}

// ─── #648 bounty escrow refunds ──────────────────────────────────────────────

#[test]
fn an_unclaimed_bounty_can_be_cancelled_for_a_refund() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let poster = Address::generate(&env);
    {
        c!(env, id, initialize_bounty_board(&env, &admin));
        let b = c!(
            env,
            id,
            post_bounty(&env, &poster, String::from_str(&env, "Map sector 7"), 500)
        )
        .unwrap();
        assert_eq!(
            c!(env, id, cancel_bounty(&env, &admin, b.id)),
            Err(BountyError::NotPoster)
        );
        assert_eq!(c!(env, id, cancel_bounty(&env, &poster, b.id)), Ok(500));
        assert!(c!(env, id, get_bounty(&env, b.id)).is_none());
        assert_eq!(
            c!(env, id, cancel_bounty(&env, &poster, b.id)),
            Err(BountyError::BountyNotFound)
        );
    }
}

#[test]
fn a_bounty_with_pending_proof_is_refundable_only_after_expiry() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let poster = Address::generate(&env);
    let hunter = Address::generate(&env);
    {
        c!(env, id, initialize_bounty_board(&env, &admin));
        let b = c!(
            env,
            id,
            post_bounty(&env, &poster, String::from_str(&env, "Escort"), 300)
        )
        .unwrap();
        c!(
            env,
            id,
            claim_bounty(&env, &hunter, b.id, BytesN::from_array(&env, &[7; 32]))
        )
        .unwrap();
        assert_eq!(
            c!(env, id, cancel_bounty(&env, &poster, b.id)),
            Err(BountyError::AlreadyClaimed)
        );

        env.ledger().with_mut(|l| l.timestamp = b.expires_at + 1);
        assert_eq!(c!(env, id, cancel_bounty(&env, &poster, b.id)), Ok(300));
    }
}

#[test]
fn an_approved_bounty_cannot_be_cancelled() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let poster = Address::generate(&env);
    let hunter = Address::generate(&env);
    {
        c!(env, id, initialize_bounty_board(&env, &admin));
        let b = c!(
            env,
            id,
            post_bounty(&env, &poster, String::from_str(&env, "Salvage"), 100)
        )
        .unwrap();
        c!(
            env,
            id,
            claim_bounty(&env, &hunter, b.id, BytesN::from_array(&env, &[1; 32]))
        )
        .unwrap();
        c!(env, id, approve_bounty(&env, &poster, b.id)).unwrap();
        env.ledger().with_mut(|l| l.timestamp = b.expires_at + 1);
        assert_eq!(
            c!(env, id, cancel_bounty(&env, &poster, b.id)),
            Err(BountyError::AlreadyClaimed)
        );
    }
}

// ─── #652 yield risk analysis ────────────────────────────────────────────────

fn record(env: &Env, player: &Address, day: u64, amount: i128) {
    update_forecast_model(
        env,
        player,
        YieldDataPoint {
            timestamp: day * 86_400,
            yield_amount: amount,
            source: symbol_short!("harvest"),
        },
    )
    .unwrap();
}

#[test]
fn yield_risk_needs_history_and_classifies_volatility() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let steady = Address::generate(&env);
    let wild = Address::generate(&env);
    {
        c!(env, id, initialize_forecast(&env, &admin)).unwrap();

        let empty = c!(env, id, analyze_yield_risk(&env, &steady));
        assert_eq!(empty.level, YieldRiskLevel::Unknown);
        assert_eq!(empty.recommendation, Symbol::new(&env, "observe"));

        for (day, amt) in [(1, 100), (2, 102), (3, 104), (4, 106)] {
            c!(env, id, record(&env, &steady, day, amt));
        }
        let r = c!(env, id, analyze_yield_risk(&env, &steady));
        assert_eq!((r.data_points, r.level), (4, YieldRiskLevel::Low));
        assert_eq!(r.trend_per_day, 2);
        assert_eq!(r.recommendation, Symbol::new(&env, "hold"));

        for (day, amt) in [(1, 400), (2, 50), (3, 300), (4, 20)] {
            c!(env, id, record(&env, &wild, day, amt));
        }
        let r = c!(env, id, analyze_yield_risk(&env, &wild));
        assert_eq!(r.level, YieldRiskLevel::High);
        assert_eq!(
            r.recommendation,
            Symbol::new(&env, "reduce"),
            "high risk and falling"
        );
    }
}

// ─── #653 fractional micro-trading ───────────────────────────────────────────

#[test]
fn listed_shares_trade_and_set_the_last_unit_price() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);
    let ore = symbol_short!("ore");
    {
        c!(env, id, initialize_fractional(&env, &admin)).unwrap();
        let shares = c!(
            env,
            id,
            fractionalize_resource(&env, &seller, ore.clone(), 100, 10)
        )
        .unwrap();
        let share = shares.get(0).unwrap();

        assert_eq!(
            c!(env, id, list_share_for_sale(&env, &seller, share, 0)),
            Err(FractionalError::InvalidPrice)
        );
        assert_eq!(
            c!(env, id, list_share_for_sale(&env, &buyer, share, 50)),
            Err(FractionalError::NotOwner)
        );
        c!(env, id, list_share_for_sale(&env, &seller, share, 50)).unwrap();
        assert_eq!(
            c!(env, id, get_share_listing(&env, share)).unwrap().price,
            50
        );
        assert_eq!(
            c!(env, id, buy_listed_share(&env, &seller, share)),
            Err(FractionalError::NotOwner)
        );

        assert_eq!(c!(env, id, buy_listed_share(&env, &buyer, share)), Ok(50));
        assert!(c!(env, id, is_share_owner(&env, &buyer, share)));
        assert!(!c!(env, id, is_share_owner(&env, &seller, share)));
        assert!(c!(env, id, get_share_listing(&env, share)).is_none());
        // 50 for a 10-unit share
        assert_eq!(
            c!(env, id, get_last_share_price(&env, ore.clone())),
            Some(5)
        );
        assert_eq!(
            c!(env, id, buy_listed_share(&env, &buyer, share)),
            Err(FractionalError::NotListed)
        );
    }
}

#[test]
fn transferring_or_cancelling_closes_a_listing() {
    let (env, id) = setup();
    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let friend = Address::generate(&env);
    {
        c!(env, id, initialize_fractional(&env, &admin)).unwrap();
        let shares = c!(
            env,
            id,
            fractionalize_resource(&env, &seller, symbol_short!("gas"), 20, 2)
        )
        .unwrap();
        let (a, b) = (shares.get(0).unwrap(), shares.get(1).unwrap());

        c!(env, id, list_share_for_sale(&env, &seller, a, 10)).unwrap();
        assert_eq!(
            c!(env, id, cancel_share_listing(&env, &friend, a)),
            Err(FractionalError::NotOwner)
        );
        c!(env, id, cancel_share_listing(&env, &seller, a)).unwrap();
        assert!(c!(env, id, get_share_listing(&env, a)).is_none());

        c!(env, id, list_share_for_sale(&env, &seller, b, 10)).unwrap();
        c!(env, id, transfer_share(&env, &seller, &friend, b)).unwrap();
        assert!(c!(env, id, get_share_listing(&env, b)).is_none());
    }
}
