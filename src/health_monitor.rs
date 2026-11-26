use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol, Vec};

#[derive(Clone)]
#[contracttype]
pub enum HealthKey {
    Metric(Symbol),
    HealthRegistry,
    /// Ledger time of the first recorded sample (uptime origin).
    HealthStartedAt,
    /// Call / error / cost counters per operation.
    HealthOperation(Symbol),
    HealthOpRegistry,
    /// Latest usage per resource.
    HealthResource(Symbol),
    HealthResourceRegistry,
    /// Alert thresholds used by `check_health`.
    HealthThresholdConfig,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HealthError {
    MetricBurstExceeded = 1,
    EmptyMetricBatch = 2,
    /// Too many distinct operations or resources are tracked.
    RegistryFull = 3,
    /// Resource capacity must be non-zero.
    InvalidCapacity = 4,
    /// Thresholds must satisfy degraded <= unhealthy <= 10_000 bps.
    InvalidThresholds = 5,
}

impl crate::error_standard::StandardContractError for HealthError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::MetricBurstExceeded => (ErrorKind::ResourceLimit, true),
            Self::EmptyMetricBatch => (ErrorKind::Validation, false),
            Self::RegistryFull => (ErrorKind::ResourceLimit, false),
            Self::InvalidCapacity | Self::InvalidThresholds => (ErrorKind::Validation, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "health_monitor",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HealthMetricInput {
    pub metric: Symbol,
    pub value: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HealthMetricSummary {
    pub metric: Symbol,
    pub samples: u64,
    pub total: u64,
    pub minimum: u64,
    pub maximum: u64,
    pub last_value: u64,
    pub last_updated: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HealthSummary {
    pub metric_count: u32,
    pub total_samples: u64,
    pub total_value: u64,
    pub last_metric: Symbol,
    pub last_value: u64,
    pub metrics: Vec<HealthMetricSummary>,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
struct HealthAggregate {
    samples: u64,
    total: u64,
    minimum: u64,
    maximum: u64,
    last_value: u64,
    last_updated: u64,
}

fn default_aggregate(value: u64, timestamp: u64) -> HealthAggregate {
    HealthAggregate {
        samples: 1,
        total: value,
        minimum: value,
        maximum: value,
        last_value: value,
        last_updated: timestamp,
    }
}

fn load_registry(env: &Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&HealthKey::HealthRegistry)
        .unwrap_or_else(|| Vec::new(env))
}

fn store_registry(env: &Env, registry: &Vec<Symbol>) {
    env.storage()
        .persistent()
        .set(&HealthKey::HealthRegistry, registry);
}

fn record_metric(env: &Env, metric: Symbol, value: u64) -> HealthMetricSummary {
    mark_started(env);
    let mut registry = load_registry(env);
    let mut seen = false;

    let mut i = 0u32;
    while i < registry.len() {
        if let Some(existing) = registry.get(i) {
            if existing == metric {
                seen = true;
                break;
            }
        }
        i += 1;
    }

    if !seen {
        registry.push_back(metric.clone());
        store_registry(env, &registry);
    }

    let exists = env
        .storage()
        .persistent()
        .has(&HealthKey::Metric(metric.clone()));

    let mut aggregate = if exists {
        env.storage()
            .persistent()
            .get(&HealthKey::Metric(metric.clone()))
            .unwrap_or(default_aggregate(value, env.ledger().timestamp()))
    } else {
        default_aggregate(value, env.ledger().timestamp())
    };

    if exists {
        aggregate.samples = aggregate.samples.saturating_add(1);
        aggregate.total = aggregate.total.saturating_add(value);
        aggregate.minimum = aggregate.minimum.min(value);
        aggregate.maximum = aggregate.maximum.max(value);
        aggregate.last_value = value;
        aggregate.last_updated = env.ledger().timestamp();
    }

    env.storage()
        .persistent()
        .set(&HealthKey::Metric(metric.clone()), &aggregate);

    let summary = HealthMetricSummary {
        metric: metric.clone(),
        samples: aggregate.samples,
        total: aggregate.total,
        minimum: aggregate.minimum,
        maximum: aggregate.maximum,
        last_value: aggregate.last_value,
        last_updated: aggregate.last_updated,
    };

    env.events().publish(
        (symbol_short!("health"), symbol_short!("record")),
        (metric, value, summary.samples, summary.total),
    );

    summary
}

pub fn record_contract_health(env: &Env, metric: Symbol, value: u64) -> HealthMetricSummary {
    record_metric(env, metric, value)
}

pub fn record_contract_health_batch(
    env: &Env,
    metrics: Vec<HealthMetricInput>,
) -> Result<Vec<HealthMetricSummary>, HealthError> {
    if metrics.len() == 0 {
        return Err(HealthError::EmptyMetricBatch);
    }
    if metrics.len() > 50 {
        return Err(HealthError::MetricBurstExceeded);
    }

    let mut summaries = Vec::new(env);
    let mut i = 0u32;
    while i < metrics.len() {
        if let Some(input) = metrics.get(i) {
            summaries.push_back(record_metric(env, input.metric, input.value));
        }
        i += 1;
    }

    Ok(summaries)
}

pub fn get_health_summary(env: &Env) -> HealthSummary {
    let registry = load_registry(env);
    let mut metrics = Vec::new(env);
    let mut total_samples = 0u64;
    let mut total_value = 0u64;
    let mut last_metric = symbol_short!("none");
    let mut last_value = 0u64;

    let mut i = 0u32;
    while i < registry.len() {
        if let Some(metric) = registry.get(i) {
            if let Some(aggregate) = env
                .storage()
                .persistent()
                .get::<HealthKey, HealthAggregate>(&HealthKey::Metric(metric.clone()))
            {
                total_samples = total_samples.saturating_add(aggregate.samples);
                total_value = total_value.saturating_add(aggregate.total);
                last_metric = metric.clone();
                last_value = aggregate.last_value;
                metrics.push_back(HealthMetricSummary {
                    metric,
                    samples: aggregate.samples,
                    total: aggregate.total,
                    minimum: aggregate.minimum,
                    maximum: aggregate.maximum,
                    last_value: aggregate.last_value,
                    last_updated: aggregate.last_updated,
                });
            }
        }
        i += 1;
    }

    HealthSummary {
        metric_count: metrics.len(),
        total_samples,
        total_value,
        last_metric,
        last_value,
        metrics,
    }
}

// ─── Health checks, uptime, error rates, performance, resources (#117) ───

/// Maximum distinct operations and resources tracked (bounds `check_health`).
pub const MAX_TRACKED_ITEMS: u32 = 32;
const BPS: u64 = 10_000;

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct OperationStats {
    pub operation: Symbol,
    pub calls: u64,
    pub errors: u64,
    /// Sum of caller-reported cost (e.g. CPU instructions or fee) per call.
    pub total_cost: u64,
    pub max_cost: u64,
    /// Ledger time of the last failed call, 0 if none.
    pub last_error_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ResourceUsage {
    pub resource: Symbol,
    pub used: u64,
    pub capacity: u64,
    pub peak_used: u64,
    pub updated_at: u64,
}

/// Alert thresholds. Error and resource limits are in basis points (1/100 %).
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HealthThresholds {
    pub degraded_error_bps: u32,
    pub unhealthy_error_bps: u32,
    pub degraded_resource_bps: u32,
    pub unhealthy_resource_bps: u32,
    /// Average cost per call above which performance is degraded; 0 disables the check.
    pub max_avg_cost: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[contracttype]
pub enum HealthStatus {
    Healthy = 0,
    Degraded = 1,
    Unhealthy = 2,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HealthReport {
    pub status: HealthStatus,
    pub uptime_secs: u64,
    pub total_calls: u64,
    pub total_errors: u64,
    pub error_rate_bps: u32,
    pub avg_cost: u64,
    pub max_cost: u64,
    /// Most utilised resource (`none` when no resource is tracked).
    pub worst_resource: Symbol,
    pub worst_resource_bps: u32,
    /// Names of failing checks: `err_rate`, `resource`, `perf`.
    pub failed_checks: Vec<Symbol>,
    pub checked_at: u64,
    pub operations: Vec<OperationStats>,
    pub resources: Vec<ResourceUsage>,
}

pub fn default_thresholds() -> HealthThresholds {
    HealthThresholds {
        degraded_error_bps: 500,
        unhealthy_error_bps: 2_000,
        degraded_resource_bps: 8_000,
        unhealthy_resource_bps: 9_500,
        max_avg_cost: 0,
    }
}

fn mark_started(env: &Env) {
    if !env.storage().persistent().has(&HealthKey::HealthStartedAt) {
        env.storage()
            .persistent()
            .set(&HealthKey::HealthStartedAt, &env.ledger().timestamp());
    }
}

/// Add `name` to the registry at `key` if missing.
fn track(env: &Env, key: &HealthKey, name: &Symbol) -> Result<Vec<Symbol>, HealthError> {
    let mut names: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| Vec::new(env));
    if !names.contains(name) {
        if names.len() >= MAX_TRACKED_ITEMS {
            return Err(HealthError::RegistryFull);
        }
        names.push_back(name.clone());
        env.storage().persistent().set(key, &names);
    }
    Ok(names)
}

fn ratio_bps(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return 0;
    }
    // u128 avoids overflow; the result is <= BPS when part <= whole.
    let bps = u128::from(part) * u128::from(BPS) / u128::from(whole);
    u32::try_from(bps).unwrap_or(u32::MAX)
}

/// Record one call of `operation`: whether it succeeded and what it cost.
pub fn record_operation(
    env: &Env,
    operation: Symbol,
    success: bool,
    cost: u64,
) -> Result<OperationStats, HealthError> {
    track(env, &HealthKey::HealthOpRegistry, &operation)?;
    mark_started(env);
    let key = HealthKey::HealthOperation(operation.clone());
    let mut stats: OperationStats =
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(OperationStats {
                operation: operation.clone(),
                calls: 0,
                errors: 0,
                total_cost: 0,
                max_cost: 0,
                last_error_at: 0,
            });
    stats.calls = stats.calls.saturating_add(1);
    stats.total_cost = stats.total_cost.saturating_add(cost);
    stats.max_cost = stats.max_cost.max(cost);
    if !success {
        stats.errors = stats.errors.saturating_add(1);
        stats.last_error_at = env.ledger().timestamp();
    }
    env.storage().persistent().set(&key, &stats);
    env.events().publish(
        (symbol_short!("health"), symbol_short!("op")),
        (operation, success, cost),
    );
    Ok(stats)
}

/// Record the current usage of a bounded resource (storage entries, pool slots, ...).
pub fn record_resource_usage(
    env: &Env,
    resource: Symbol,
    used: u64,
    capacity: u64,
) -> Result<ResourceUsage, HealthError> {
    if capacity == 0 {
        return Err(HealthError::InvalidCapacity);
    }
    track(env, &HealthKey::HealthResourceRegistry, &resource)?;
    mark_started(env);
    let key = HealthKey::HealthResource(resource.clone());
    let peak_used = env
        .storage()
        .persistent()
        .get::<HealthKey, ResourceUsage>(&key)
        .map_or(used, |prev| prev.peak_used.max(used));
    let usage = ResourceUsage {
        resource,
        used,
        capacity,
        peak_used,
        updated_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&key, &usage);
    Ok(usage)
}

/// Replace the alert thresholds. The module is crate-private: callers enforce auth.
pub fn set_health_thresholds(env: &Env, thresholds: HealthThresholds) -> Result<(), HealthError> {
    let max = BPS as u32;
    if thresholds.degraded_error_bps > thresholds.unhealthy_error_bps
        || thresholds.unhealthy_error_bps > max
        || thresholds.degraded_resource_bps > thresholds.unhealthy_resource_bps
        || thresholds.unhealthy_resource_bps > max
    {
        return Err(HealthError::InvalidThresholds);
    }
    env.storage()
        .persistent()
        .set(&HealthKey::HealthThresholdConfig, &thresholds);
    Ok(())
}

pub fn get_health_thresholds(env: &Env) -> HealthThresholds {
    env.storage()
        .persistent()
        .get(&HealthKey::HealthThresholdConfig)
        .unwrap_or_else(default_thresholds)
}

fn level(value: u32, degraded: u32, unhealthy: u32) -> HealthStatus {
    if value >= unhealthy {
        HealthStatus::Unhealthy
    } else if value >= degraded {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    }
}

/// Run all health checks and return the report. Emits `("health", "alert")`
/// with the status and failed checks when the contract is not healthy.
pub fn check_health(env: &Env) -> HealthReport {
    let now = env.ledger().timestamp();
    let started_at: Option<u64> = env.storage().persistent().get(&HealthKey::HealthStartedAt);
    let thresholds = get_health_thresholds(env);

    let mut operations = Vec::new(env);
    let (mut total_calls, mut total_errors, mut total_cost, mut max_cost) =
        (0u64, 0u64, 0u64, 0u64);
    let op_names: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&HealthKey::HealthOpRegistry)
        .unwrap_or_else(|| Vec::new(env));
    for name in op_names.iter() {
        if let Some(stats) = env
            .storage()
            .persistent()
            .get::<HealthKey, OperationStats>(&HealthKey::HealthOperation(name))
        {
            total_calls = total_calls.saturating_add(stats.calls);
            total_errors = total_errors.saturating_add(stats.errors);
            total_cost = total_cost.saturating_add(stats.total_cost);
            max_cost = max_cost.max(stats.max_cost);
            operations.push_back(stats);
        }
    }

    let mut resources = Vec::new(env);
    let (mut worst_resource, mut worst_resource_bps) = (symbol_short!("none"), 0u32);
    let res_names: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&HealthKey::HealthResourceRegistry)
        .unwrap_or_else(|| Vec::new(env));
    for name in res_names.iter() {
        if let Some(usage) = env
            .storage()
            .persistent()
            .get::<HealthKey, ResourceUsage>(&HealthKey::HealthResource(name))
        {
            let bps = ratio_bps(usage.used, usage.capacity);
            if bps >= worst_resource_bps {
                worst_resource_bps = bps;
                worst_resource = usage.resource.clone();
            }
            resources.push_back(usage);
        }
    }

    let error_rate_bps = ratio_bps(total_errors, total_calls);
    let avg_cost = total_cost.checked_div(total_calls).unwrap_or(0);

    let mut status = HealthStatus::Healthy;
    let mut failed_checks = Vec::new(env);
    let t = &thresholds;
    let perf = if t.max_avg_cost > 0 && avg_cost > t.max_avg_cost {
        HealthStatus::Degraded
    } else {
        HealthStatus::Healthy
    };
    let checks = [
        (
            symbol_short!("err_rate"),
            level(error_rate_bps, t.degraded_error_bps, t.unhealthy_error_bps),
        ),
        (
            symbol_short!("resource"),
            level(
                worst_resource_bps,
                t.degraded_resource_bps,
                t.unhealthy_resource_bps,
            ),
        ),
        (symbol_short!("perf"), perf),
    ];
    for (name, check) in checks {
        if check != HealthStatus::Healthy {
            failed_checks.push_back(name);
            status = status.max(check);
        }
    }

    if status != HealthStatus::Healthy {
        env.events().publish(
            (symbol_short!("health"), symbol_short!("alert")),
            (status, failed_checks.clone()),
        );
    }

    HealthReport {
        status,
        uptime_secs: started_at.map_or(0, |t| now.saturating_sub(t)),
        total_calls,
        total_errors,
        error_rate_bps,
        avg_cost,
        max_cost,
        worst_resource,
        worst_resource_bps,
        failed_checks,
        checked_at: now,
        operations,
        resources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::{contract, contractimpl, Address};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn with_env(f: impl FnOnce(&Env)) {
        let env = Env::default();
        let id: Address = env.register(Stub, ());
        env.as_contract(&id, || f(&env));
    }

    #[test]
    fn empty_monitor_is_healthy() {
        with_env(|env| {
            let r = check_health(env);
            assert_eq!(r.status, HealthStatus::Healthy);
            assert_eq!((r.uptime_secs, r.total_calls, r.error_rate_bps), (0, 0, 0));
            assert_eq!(r.worst_resource, symbol_short!("none"));
            assert_eq!(r.failed_checks.len(), 0);
        });
    }

    #[test]
    fn uptime_counts_from_first_sample() {
        with_env(|env| {
            env.ledger().set_timestamp(1_000);
            record_contract_health(env, symbol_short!("achv"), 1);
            env.ledger().set_timestamp(4_600);
            record_operation(env, symbol_short!("craft"), true, 10).unwrap();
            assert_eq!(check_health(env).uptime_secs, 3_600);
        });
    }

    #[test]
    fn error_rate_drives_status() {
        with_env(|env| {
            let op = symbol_short!("craft");
            for _ in 0..19 {
                record_operation(env, op.clone(), true, 100).unwrap();
            }
            record_operation(env, op.clone(), false, 300).unwrap();
            // 1 / 20 = 500 bps: exactly the degraded threshold.
            let r = check_health(env);
            assert_eq!((r.error_rate_bps, r.status), (500, HealthStatus::Degraded));
            assert_eq!(
                r.failed_checks,
                Vec::from_array(env, [symbol_short!("err_rate")])
            );
            assert_eq!((r.avg_cost, r.max_cost), (110, 300));
            let stats = r.operations.get(0).unwrap();
            assert_eq!((stats.calls, stats.errors), (20, 1));

            for _ in 0..5 {
                record_operation(env, op.clone(), false, 1).unwrap();
            }
            // 6 / 25 = 2400 bps.
            assert_eq!(check_health(env).status, HealthStatus::Unhealthy);
        });
    }

    #[test]
    fn resource_usage_tracks_peak_and_worst() {
        with_env(|env| {
            record_resource_usage(env, symbol_short!("slots"), 50, 100).unwrap();
            record_resource_usage(env, symbol_short!("store"), 96, 100).unwrap();
            let u = record_resource_usage(env, symbol_short!("store"), 10, 100).unwrap();
            assert_eq!((u.used, u.peak_used), (10, 96));
            assert_eq!(
                record_resource_usage(env, symbol_short!("x"), 1, 0),
                Err(HealthError::InvalidCapacity)
            );

            record_resource_usage(env, symbol_short!("store"), 97, 100).unwrap();
            let r = check_health(env);
            assert_eq!(r.worst_resource, symbol_short!("store"));
            assert_eq!(
                (r.worst_resource_bps, r.status),
                (9_700, HealthStatus::Unhealthy)
            );
            assert_eq!(r.resources.len(), 2);
        });
    }

    #[test]
    fn performance_check_uses_configured_budget() {
        with_env(|env| {
            record_operation(env, symbol_short!("scan"), true, 5_000).unwrap();
            assert_eq!(check_health(env).status, HealthStatus::Healthy);

            let mut t = default_thresholds();
            t.max_avg_cost = 1_000;
            set_health_thresholds(env, t.clone()).unwrap();
            assert_eq!(get_health_thresholds(env), t);
            let r = check_health(env);
            assert_eq!(r.status, HealthStatus::Degraded);
            assert_eq!(
                r.failed_checks,
                Vec::from_array(env, [symbol_short!("perf")])
            );
        });
    }

    #[test]
    fn invalid_thresholds_are_rejected() {
        with_env(|env| {
            let mut t = default_thresholds();
            t.degraded_error_bps = 3_000;
            assert_eq!(
                set_health_thresholds(env, t),
                Err(HealthError::InvalidThresholds)
            );
            let mut t = default_thresholds();
            t.unhealthy_resource_bps = 10_001;
            assert_eq!(
                set_health_thresholds(env, t),
                Err(HealthError::InvalidThresholds)
            );
        });
    }

    #[test]
    fn registry_is_bounded() {
        with_env(|env| {
            let names = [
                "o0", "o1", "o2", "o3", "o4", "o5", "o6", "o7", "o8", "o9", "o10", "o11", "o12",
                "o13", "o14", "o15", "o16", "o17", "o18", "o19", "o20", "o21", "o22", "o23", "o24",
                "o25", "o26", "o27", "o28", "o29", "o30", "o31",
            ];
            for n in names {
                record_operation(env, Symbol::new(env, n), true, 1).unwrap();
            }
            assert_eq!(
                record_operation(env, symbol_short!("extra"), true, 1),
                Err(HealthError::RegistryFull)
            );
            // Already-tracked operations keep recording.
            record_operation(env, Symbol::new(env, "o0"), true, 1).unwrap();
        });
    }
}
