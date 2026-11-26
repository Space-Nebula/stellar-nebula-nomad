//! # Revenue Attribution and Analytics (#369, #656)
//!
//! Comprehensive on-chain attribution engine and analytics backend for tracking
//! multi-channel acquisition, conversion funnels, attribution models, ROI,
//! and macro-economic impact across the game ecosystem.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

// ── Error ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AttributionError {
    /// Revenue amount must be greater than zero.
    ZeroAmount = 1,
    /// Channel list is empty for multi-touch attribution.
    EmptyChannels = 2,
    /// Invalid attribution model specified.
    InvalidModel = 3,
    /// Channel not found in registry.
    ChannelNotFound = 4,
    /// Arithmetic overflow in financial calculation.
    ArithmeticOverflow = 5,
}

impl StandardContractError for AttributionError {
    fn descriptor(self) -> ErrorDescriptor {
        let (kind, retryable) = match self {
            Self::ZeroAmount | Self::EmptyChannels | Self::InvalidModel => {
                (ErrorKind::Validation, false)
            }
            Self::ChannelNotFound => (ErrorKind::NotFound, false) ,
            Self::ArithmeticOverflow => (ErrorKind::Internal, false),
        };
        ErrorDescriptor {
            module: "revenue_attribution",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ── Enums & Data Types ──────────────────────────────────────────────────

/// Primary revenue categories tracked in the ecosystem.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RevenueSource {
    MarketplaceSale = 1,
    InGamePurchase = 2,
    Subscription = 3,
    TournamentFee = 4,
    StakingYieldFee = 5,
    CraftingFee = 6,
    Direct = 7,
    Referral = 8,
    EscrowFee = 9,
    BridgeFee = 10,
}

/// Multi-touch attribution models supported by the analytics engine.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AttributionModel {
    /// 100% of revenue credited to the initial touchpoint.
    FirstTouch = 1,
    /// 100% of revenue credited to the final touchpoint before conversion.
    LastTouch = 2,
    /// Revenue distributed equally among all recorded touchpoints.
    Linear = 3,
    /// Exponentially increasing weight toward more recent touchpoints.
    TimeDecay = 4,
    /// 40% first touch, 40% last touch, remaining 20% shared among middle touches.
    PositionBased = 5,
}

/// Stages of the user acquisition and conversion funnel.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FunnelStep {
    Impression = 1,
    Click = 2,
    Signup = 3,
    Purchase = 4,
}

/// Per-channel attribution summary.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ChannelAttribution {
    pub channel: Symbol,
    pub revenue: u64,
    pub spend: u64,
    pub event_count: u64,
    /// ROI in basis points: (revenue - spend) / spend * 10_000.
    /// `None` when spend is zero (ROI undefined).
    pub roi_bps: Option<i64>,
}

/// Weighted attribution result for a single touchpoint.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TouchpointWeight {
    pub channel: Symbol,
    /// Weight allocated to this touchpoint in basis points (0..=10_000).
    pub weight_bps: u32,
    /// Attributed revenue share for this touchpoint.
    pub attributed_amount: u64,
}

/// Funnel conversion metrics for a marketing channel.
#[derive(Clone, Debug, Default, PartialEq)]
#[contracttype]
pub struct ConversionFunnel {
    pub channel: Symbol,
    pub impressions: u64,
    pub clicks: u64,
    pub signups: u64,
    pub conversions: u64,
    /// Click-through rate in basis points (clicks / impressions * 10,000).
    pub ctr_bps: u32,
    /// Conversion rate in basis points (conversions / clicks * 10,000).
    pub cvr_bps: u32,
}

/// Detailed financial performance and unit economics for a channel.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ChannelFinancialAnalysis {
    pub channel: Symbol,
    pub revenue: u64,
    pub spend: u64,
    pub conversions: u64,
    pub roi_bps: Option<i64>,
    pub roas_bps: Option<u64>,
    pub cac: Option<u64>,
    pub net_profit: i128,
}

/// Macro-economic ecosystem summary metrics.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EconomicImpactReport {
    pub total_system_revenue: u64,
    pub total_marketing_spend: u64,
    pub net_ecosystem_value: i128,
    pub total_conversions: u64,
    pub active_channels_count: u32,
    pub top_performing_channel: Symbol,
    pub blended_roas_bps: Option<u64>,
    pub velocity_ratio_bps: u64,
}

/// Consolidated analytics and attribution dashboard snapshot.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RevenueAnalyticsDashboard {
    pub total_revenue: u64,
    pub total_spend: u64,
    pub total_conversions: u64,
    pub channel_attributions: Vec<ChannelAttribution>,
    pub economic_impact: EconomicImpactReport,
}

// ── Storage Keys ────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AttributionKey {
    /// Cumulative revenue attributed to a channel.
    ChannelRevenue(Symbol),
    /// Cumulative recorded marketing spend for a channel.
    ChannelSpend(Symbol),
    /// Number of revenue events attributed to a channel.
    ChannelEventCount(Symbol),
    /// List of known channel symbols (for iteration).
    KnownChannels,
    /// Cumulative revenue attributed to a source category.
    SourceRevenue(u32),
    /// Total conversions recorded in system.
    TotalConversions,
    /// Conversion count for a channel.
    ChannelConversions(Symbol),
    /// User touchpoint history.
    UserTouchpoints(Address),
    /// Funnel counters: (channel, step).
    FunnelCount(Symbol, u32),
}

// ── Core Recording Helpers ──────────────────────────────────────────────

fn register_channel(env: &Env, channel: &Symbol) {
    let mut channels: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&AttributionKey::KnownChannels)
        .unwrap_or_else(|| Vec::new(env));
    if !channels.iter().any(|c| c == *channel) {
        channels.push_back(channel.clone());
        env.storage()
            .persistent()
            .set(&AttributionKey::KnownChannels, &channels);
    }
}

/// Record a revenue event of `amount` attributed to `channel`.
pub fn record_revenue(env: &Env, channel: Symbol, amount: u64) -> Result<(), AttributionError> {
    if amount == 0 {
        return Err(AttributionError::ZeroAmount);
    }

    register_channel(env, &channel);

    let prev: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelRevenue(channel.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &AttributionKey::ChannelRevenue(channel.clone()),
        &prev.saturating_add(amount),
    );

    let count: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelEventCount(channel.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &AttributionKey::ChannelEventCount(channel.clone()),
        &count.saturating_add(1),
    );

    Ok(())
}

/// Record marketing/acquisition spend for `channel` (used for ROI calc).
pub fn record_spend(env: &Env, channel: Symbol, amount: u64) {
    register_channel(env, &channel);

    let prev: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelSpend(channel.clone()))
        .unwrap_or(0);
    env.storage().persistent().set(
        &AttributionKey::ChannelSpend(channel.clone()),
        &prev.saturating_add(amount),
    );
}

/// Record revenue categorized by economic source.
pub fn record_source_revenue(env: &Env, source: RevenueSource, amount: u64) -> Result<(), AttributionError> {
    if amount == 0 {
        return Err(AttributionError::ZeroAmount);
    }
    let key = AttributionKey::SourceRevenue(source as u32);
    let prev: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &prev.saturating_add(amount));
    Ok(())
}

/// Query cumulative revenue by economic source.
pub fn get_source_revenue(env: &Env, source: RevenueSource) -> u64 {
    env.storage()
        .persistent()
        .get(&AttributionKey::SourceRevenue(source as u32))
        .unwrap_or(0)
}

/// Record an acquisition touchpoint for a specific user.
pub fn record_user_touchpoint(env: &Env, user: Address, channel: Symbol) {
    register_channel(env, &channel);
    let key = AttributionKey::UserTouchpoints(user);
    let mut touchpoints: Vec<Symbol> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(env));
    touchpoints.push_back(channel);
    env.storage().persistent().set(&key, &touchpoints);
}

/// Get touchpoints associated with a user.
pub fn get_user_touchpoints(env: &Env, user: Address) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&AttributionKey::UserTouchpoints(user))
        .unwrap_or_else(|| Vec::new(env))
}

/// Record a step in the acquisition funnel for `channel`.
pub fn record_funnel_step(env: &Env, channel: Symbol, step: FunnelStep) {
    register_channel(env, &channel);
    let key = AttributionKey::FunnelCount(channel, step as u32);
    let prev: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &prev.saturating_add(1));
}

/// Get funnel conversion statistics for `channel`.
pub fn get_funnel_metrics(env: &Env, channel: Symbol) -> ConversionFunnel {
    let imp: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::FunnelCount(channel.clone(), FunnelStep::Impression as u32))
        .unwrap_or(0);
    let clicks: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::FunnelCount(channel.clone(), FunnelStep::Click as u32))
        .unwrap_or(0);
    let signups: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::FunnelCount(channel.clone(), FunnelStep::Signup as u32))
        .unwrap_or(0);
    let conversions: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::FunnelCount(channel.clone(), FunnelStep::Purchase as u32))
        .unwrap_or(0);

    let ctr_bps = if imp == 0 {
        0
    } else {
        ((clicks as u128 * 10_000) / imp as u128) as u32
    };

    let cvr_bps = if clicks == 0 {
        0
    } else {
        ((conversions as u128 * 10_000) / clicks as u128) as u32
    };

    ConversionFunnel {
        channel,
        impressions: imp,
        clicks,
        signups,
        conversions,
        ctr_bps,
        cvr_bps,
    }
}

// ── Multi-Touch Attribution Engine ──────────────────────────────────────

/// Calculate attribution weights for a sequence of touchpoint channels
/// using the specified attribution model.
pub fn calculate_attribution_weights(
    env: &Env,
    channels: Vec<Symbol>,
    total_amount: u64,
    model: AttributionModel,
) -> Result<Vec<TouchpointWeight>, AttributionError> {
    let len = channels.len();
    if len == 0 {
        return Err(AttributionError::EmptyChannels);
    }

    let mut result = Vec::new(env);

    if len == 1 {
        result.push_back(TouchpointWeight {
            channel: channels.get(0).unwrap(),
            weight_bps: 10_000,
            attributed_amount: total_amount,
        });
        return Ok(result);
    }

    match model {
        AttributionModel::FirstTouch => {
            for i in 0..len {
                let ch = channels.get(i).unwrap();
                let (w, amt) = if i == 0 { (10_000, total_amount) } else { (0, 0) };
                result.push_back(TouchpointWeight {
                    channel: ch,
                    weight_bps: w,
                    attributed_amount: amt,
                });
            }
        }
        AttributionModel::LastTouch => {
            for i in 0..len {
                let ch = channels.get(i).unwrap();
                let (w, amt) = if i == len - 1 { (10_000, total_amount) } else { (0, 0) };
                result.push_back(TouchpointWeight {
                    channel: ch,
                    weight_bps: w,
                    attributed_amount: amt,
                });
            }
        }
        AttributionModel::Linear => {
            let weight_each = 10_000 / len as u32;
            let mut allocated = 0u64;
            for i in 0..len {
                let ch = channels.get(i).unwrap();
                let amt = if i == len - 1 {
                    total_amount.saturating_sub(allocated)
                } else {
                    let share = (total_amount as u128 * weight_each as u128 / 10_000) as u64;
                    allocated = allocated.saturating_add(share);
                    share
                };
                result.push_back(TouchpointWeight {
                    channel: ch,
                    weight_bps: weight_each,
                    attributed_amount: amt,
                });
            }
        }
        AttributionModel::PositionBased => {
            // 40% first, 40% last, 20% distributed evenly across middle
            let first_weight = 4_000u32;
            let last_weight = 4_000u32;
            let middle_count = (len - 2) as u32;
            let middle_weight = if middle_count > 0 { 2_000 / middle_count } else { 0 };

            let mut allocated = 0u64;
            for i in 0..len {
                let ch = channels.get(i).unwrap();
                let w = if i == 0 {
                    first_weight
                } else if i == len - 1 {
                    last_weight
                } else {
                    middle_weight
                };

                let amt = if i == len - 1 {
                    total_amount.saturating_sub(allocated)
                } else {
                    let share = (total_amount as u128 * w as u128 / 10_000) as u64;
                    allocated = allocated.saturating_add(share);
                    share
                };

                result.push_back(TouchpointWeight {
                    channel: ch,
                    weight_bps: w,
                    attributed_amount: amt,
                });
            }
        }
        AttributionModel::TimeDecay => {
            // Linearly increasing weights from oldest to newest touchpoint
            let total_shares = (len * (len + 1)) / 2;
            let mut allocated = 0u64;
            for i in 0..len {
                let ch = channels.get(i).unwrap();
                let share_index = (i + 1) as u32;
                let w = ((share_index as u128 * 10_000) / total_shares as u128) as u32;
                let amt = if i == len - 1 {
                    total_amount.saturating_sub(allocated)
                } else {
                    let share = (total_amount as u128 * w as u128 / 10_000) as u64;
                    allocated = allocated.saturating_add(share);
                    share
                };
                result.push_back(TouchpointWeight {
                    channel: ch,
                    weight_bps: w,
                    attributed_amount: amt,
                });
            }
        }
    }

    Ok(result)
}

/// Record a conversion and attribute revenue across touchpoints using the specified model.
pub fn record_conversion(
    env: &Env,
    user: Address,
    amount: u64,
    source: RevenueSource,
    model: AttributionModel,
) -> Result<Vec<TouchpointWeight>, AttributionError> {
    if amount == 0 {
        return Err(AttributionError::ZeroAmount);
    }

    let touchpoints = get_user_touchpoints(env, user);
    let channels = if touchpoints.len() == 0 {
        let mut def = Vec::new(env);
        def.push_back(symbol_short!("direct"));
        def
    } else {
        touchpoints
    };

    let weights = calculate_attribution_weights(env, channels, amount, model)?;

    for i in 0..weights.len() {
        let tp = weights.get(i).unwrap();
        if tp.attributed_amount > 0 {
            record_revenue(env, tp.channel.clone(), tp.attributed_amount)?;
        }
        let ch_conv: u64 = env
            .storage()
            .persistent()
            .get(&AttributionKey::ChannelConversions(tp.channel.clone()))
            .unwrap_or(0);
        env.storage().persistent().set(
            &AttributionKey::ChannelConversions(tp.channel.clone()),
            &ch_conv.saturating_add(1),
        );
    }

    record_source_revenue(env, source, amount)?;

    let total_conv: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::TotalConversions)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&AttributionKey::TotalConversions, &total_conv.saturating_add(1));

    Ok(weights)
}

// ── Financial Queries & Analytics ───────────────────────────────────────

/// Get the attribution summary (revenue, spend, ROI) for `channel`.
pub fn get_channel_attribution(env: &Env, channel: Symbol) -> ChannelAttribution {
    let revenue: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelRevenue(channel.clone()))
        .unwrap_or(0);
    let spend: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelSpend(channel.clone()))
        .unwrap_or(0);
    let event_count: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelEventCount(channel.clone()))
        .unwrap_or(0);

    let roi_bps = if spend == 0 {
        None
    } else {
        let net = revenue as i64 - spend as i64;
        Some(net.saturating_mul(10_000) / spend as i64)
    };

    ChannelAttribution {
        channel,
        revenue,
        spend,
        event_count,
        roi_bps,
    }
}

/// Return attribution summaries for all registered channels.
pub fn all_channel_attributions(env: &Env) -> Vec<ChannelAttribution> {
    let channels: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&AttributionKey::KnownChannels)
        .unwrap_or_else(|| Vec::new(env));

    let mut result: Vec<ChannelAttribution> = Vec::new(env);
    for channel in channels.iter() {
        result.push_back(get_channel_attribution(env, channel));
    }
    result
}

/// Compute deep unit economics and return on ad spend (ROAS) for `channel`.
pub fn get_channel_financials(env: &Env, channel: Symbol) -> ChannelFinancialAnalysis {
    let attr = get_channel_attribution(env, channel.clone());
    let conversions: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::ChannelConversions(channel.clone()))
        .unwrap_or(0);

    let roas_bps = if attr.spend == 0 {
        None
    } else {
        Some(((attr.revenue as u128 * 10_000) / attr.spend as u128) as u64)
    };

    let cac = if conversions == 0 {
        None
    } else {
        Some(attr.spend / conversions)
    };

    let net_profit = attr.revenue as i128 - attr.spend as i128;

    ChannelFinancialAnalysis {
        channel,
        revenue: attr.revenue,
        spend: attr.spend,
        conversions,
        roi_bps: attr.roi_bps,
        roas_bps,
        cac,
        net_profit,
    }
}

/// Synthesize macro-economic impact across all acquisition channels.
pub fn get_economic_impact(env: &Env) -> EconomicImpactReport {
    let channels = all_channel_attributions(env);
    let mut total_rev: u64 = 0;
    let mut total_spend: u64 = 0;
    let mut top_channel = symbol_short!("none");
    let mut top_revenue: u64 = 0;

    for i in 0..channels.len() {
        let ch = channels.get(i).unwrap();
        total_rev = total_rev.saturating_add(ch.revenue);
        total_spend = total_spend.saturating_add(ch.spend);
        if ch.revenue > top_revenue {
            top_revenue = ch.revenue;
            top_channel = ch.channel;
        }
    }

    let total_conv: u64 = env
        .storage()
        .persistent()
        .get(&AttributionKey::TotalConversions)
        .unwrap_or(0);

    let blended_roas_bps = if total_spend == 0 {
        None
    } else {
        Some(((total_rev as u128 * 10_000) / total_spend as u128) as u64)
    };

    let net_value = total_rev as i128 - total_spend as i128;
    let velocity_bps = if total_spend == 0 {
        10_000
    } else {
        ((total_rev as u128 * 10_000) / (total_rev.saturating_add(total_spend)) as u128) as u64
    };

    EconomicImpactReport {
        total_system_revenue: total_rev,
        total_marketing_spend: total_spend,
        net_ecosystem_value: net_value,
        total_conversions: total_conv,
        active_channels_count: channels.len(),
        top_performing_channel: top_channel,
        blended_roas_bps,
        velocity_ratio_bps: velocity_bps,
    }
}

/// Generate a full analytics dashboard overview for ecosystem operators.
pub fn get_revenue_dashboard(env: &Env) -> RevenueAnalyticsDashboard {
    let channel_attributions = all_channel_attributions(env);
    let economic_impact = get_economic_impact(env);

    RevenueAnalyticsDashboard {
        total_revenue: economic_impact.total_system_revenue,
        total_spend: economic_impact.total_marketing_spend,
        total_conversions: economic_impact.total_conversions,
        channel_attributions,
        economic_impact,
    }
}

// ── Unit Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    #[test]
    fn test_record_revenue_accumulates() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let channel = symbol_short!("organic");
            record_revenue(&env, channel.clone(), 100).unwrap();
            record_revenue(&env, channel.clone(), 50).unwrap();

            let attr = get_channel_attribution(&env, channel);
            assert_eq!(attr.revenue, 150);
            assert_eq!(attr.event_count, 2);
        });
    }

    #[test]
    fn test_zero_amount_rejected() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let channel = symbol_short!("organic");
            let err = record_revenue(&env, channel, 0).unwrap_err();
            assert_eq!(err, AttributionError::ZeroAmount);
        });
    }

    #[test]
    fn test_roi_and_financials() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let channel = symbol_short!("ad_camp");
            record_spend(&env, channel.clone(), 100);
            record_revenue(&env, channel.clone(), 150).unwrap();

            let attr = get_channel_attribution(&env, channel.clone());
            assert_eq!(attr.roi_bps, Some(5000));

            let fin = get_channel_financials(&env, channel);
            assert_eq!(fin.roas_bps, Some(15000));
            assert_eq!(fin.net_profit, 50);
        });
    }

    #[test]
    fn test_multi_touch_attribution_models() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let mut channels = Vec::new(&env);
            channels.push_back(symbol_short!("c1"));
            channels.push_back(symbol_short!("c2"));

            // Linear: 50% / 50%
            let linear = calculate_attribution_weights(&env, channels.clone(), 1000, AttributionModel::Linear).unwrap();
            assert_eq!(linear.get(0).unwrap().attributed_amount, 500);
            assert_eq!(linear.get(1).unwrap().attributed_amount, 500);

            // First touch: 100% c1
            let first = calculate_attribution_weights(&env, channels.clone(), 1000, AttributionModel::FirstTouch).unwrap();
            assert_eq!(first.get(0).unwrap().attributed_amount, 1000);
            assert_eq!(first.get(1).unwrap().attributed_amount, 0);

            // Last touch: 100% c2
            let last = calculate_attribution_weights(&env, channels.clone(), 1000, AttributionModel::LastTouch).unwrap();
            assert_eq!(last.get(0).unwrap().attributed_amount, 0);
            assert_eq!(last.get(1).unwrap().attributed_amount, 1000);
        });
    }

    #[test]
    fn test_conversion_and_funnel_tracking() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let channel = symbol_short!("google");
            record_funnel_step(&env, channel.clone(), FunnelStep::Impression);
            record_funnel_step(&env, channel.clone(), FunnelStep::Click);

            let funnel = get_funnel_metrics(&env, channel);
            assert_eq!(funnel.impressions, 1);
            assert_eq!(funnel.clicks, 1);
            assert_eq!(funnel.ctr_bps, 10_000);
        });
    }

    #[test]
    fn test_dashboard_and_economic_impact() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let c1 = symbol_short!("ads");
            let c2 = symbol_short!("seo");
            record_spend(&env, c1.clone(), 200);
            record_revenue(&env, c1, 500).unwrap();
            record_revenue(&env, c2, 300).unwrap();

            let dashboard = get_revenue_dashboard(&env);
            assert_eq!(dashboard.total_revenue, 800);
            assert_eq!(dashboard.total_spend, 200);
            assert_eq!(dashboard.economic_impact.net_ecosystem_value, 600);
        });
    }
}
