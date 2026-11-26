//! # Smart Alerts for Anomalies (#373)
//!
//! Threshold-configurable alerting layered on top of the existing
//! `health_monitor`/`anomaly_classifier` metrics infrastructure. Operators
//! register per-metric thresholds; each time a metric sample is reported,
//! this module evaluates it against the configured threshold and, on
//! breach, raises an alert with a severity that escalates the more times
//! the same metric keeps breaching without being acknowledged.
//!
//! ## Escalation Policy
//!
//! - 1st unacknowledged breach → `Warning`
//! - 2nd/3rd unacknowledged breach → `Critical`
//! - 4th+ unacknowledged breach → `PageOps` (highest severity)
//!
//! Acknowledging an alert (`acknowledge`) resets the breach streak for that
//! metric back to `Warning` on the next breach.
//!
//! ## Priority and aggregation (#710)
//!
//! Each threshold carries an [`AlertPriority`] that is copied to its alerts.
//! Repeated breaches of the same metric while its latest alert is still
//! unacknowledged and was seen within [`AGGREGATION_WINDOW_SECS`] update that
//! alert (`occurrences`, `severity`, `value`, `last_seen`) instead of adding a
//! new log entry, so one ongoing incident is one alert.

use soroban_sdk::{contracterror, contracttype, Env, Symbol, Vec};

use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

// ── Error ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AlertError {
    /// No threshold has been configured for this metric.
    ThresholdNotConfigured = 1,
    /// Referenced alert does not exist.
    AlertNotFound = 2,
}

impl StandardContractError for AlertError {
    fn descriptor(self) -> ErrorDescriptor {
        ErrorDescriptor {
            module: "smart_alerts",
            code: self as u32,
            kind: match self {
                AlertError::ThresholdNotConfigured => ErrorKind::Validation,
                AlertError::AlertNotFound => ErrorKind::NotFound,
            },
            retryable: false,
        }
    }
}

// ── Storage Keys ────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AlertKey {
    /// Configured threshold for a metric.
    Threshold(Symbol),
    /// Escalation state (consecutive unacknowledged breaches) for a metric.
    EscalationState(Symbol),
    /// Active alert log entries, most-recent-last, capped at `MAX_ALERTS`.
    ActiveAlerts,
    /// Monotonic alert id counter.
    AlertCounter,
}

// ── Constants ───────────────────────────────────────────────────────────

/// Maximum number of active alerts retained in the on-chain log.
const MAX_ALERTS: u32 = 30;
/// Breach streak thresholds for escalation.
const CRITICAL_AT_STREAK: u32 = 2;
const PAGE_OPS_AT_STREAK: u32 = 4;
/// Breaches closer together than this fold into the open alert for the metric.
pub const AGGREGATION_WINDOW_SECS: u64 = 3_600;

// ── Data Types ──────────────────────────────────────────────────────────

/// How urgently a metric's alerts must be handled. Ordered low to high.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[contracttype]
pub enum AlertPriority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// Threshold configuration for a single metric.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AlertThreshold {
    pub metric: Symbol,
    /// Alert fires when the sampled value is >= this value.
    pub max_value: u64,
    pub priority: AlertPriority,
}

/// Alert severity, escalating with repeated unacknowledged breaches.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum AlertSeverity {
    Warning,
    Critical,
    PageOps,
}

/// A raised alert record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AlertRecord {
    pub alert_id: u64,
    pub metric: Symbol,
    pub value: u64,
    pub threshold: u64,
    pub severity: AlertSeverity,
    pub timestamp: u64,
    pub acknowledged: bool,
    pub priority: AlertPriority,
    /// Breaches folded into this alert (1 for a fresh alert).
    pub occurrences: u32,
    /// Ledger time of the latest folded breach.
    pub last_seen: u64,
}

// ── Configuration ───────────────────────────────────────────────────────

/// Configure (or replace) the threshold for `metric` with `Medium` priority.
pub fn configure_threshold(env: &Env, metric: Symbol, max_value: u64) {
    configure_threshold_with_priority(env, metric, max_value, AlertPriority::Medium);
}

/// Configure (or replace) the threshold and priority for `metric`.
pub fn configure_threshold_with_priority(
    env: &Env,
    metric: Symbol,
    max_value: u64,
    priority: AlertPriority,
) {
    env.storage().persistent().set(
        &AlertKey::Threshold(metric.clone()),
        &AlertThreshold {
            metric,
            max_value,
            priority,
        },
    );
}

/// Fetch the configured threshold for `metric`, if any.
pub fn get_threshold(env: &Env, metric: &Symbol) -> Option<AlertThreshold> {
    env.storage()
        .persistent()
        .get(&AlertKey::Threshold(metric.clone()))
}

// ── Core Evaluation ─────────────────────────────────────────────────────

/// Evaluate a metric sample against its configured threshold. If it
/// breaches, raises and stores an escalating alert and returns it.
/// Returns `Ok(None)` if the metric is within bounds, and
/// `Err(ThresholdNotConfigured)` if no threshold was set for this metric.
pub fn evaluate_sample(
    env: &Env,
    metric: Symbol,
    value: u64,
) -> Result<Option<AlertRecord>, AlertError> {
    let threshold = get_threshold(env, &metric).ok_or(AlertError::ThresholdNotConfigured)?;

    if value < threshold.max_value {
        // Sample is healthy: reset escalation streak.
        env.storage()
            .temporary()
            .set(&AlertKey::EscalationState(metric.clone()), &0u32);
        return Ok(None);
    }

    let streak: u32 = env
        .storage()
        .temporary()
        .get(&AlertKey::EscalationState(metric.clone()))
        .unwrap_or(0)
        + 1;
    env.storage()
        .temporary()
        .set(&AlertKey::EscalationState(metric.clone()), &streak);

    let severity = if streak >= PAGE_OPS_AT_STREAK {
        AlertSeverity::PageOps
    } else if streak >= CRITICAL_AT_STREAK {
        AlertSeverity::Critical
    } else {
        AlertSeverity::Warning
    };

    let now = env.ledger().timestamp();
    let mut active: Vec<AlertRecord> = env
        .storage()
        .persistent()
        .get(&AlertKey::ActiveAlerts)
        .unwrap_or_else(|| Vec::new(env));

    // Fold into the metric's open alert while the incident is ongoing.
    let mut i = active.len();
    while i > 0 {
        i -= 1;
        let mut open = active.get(i).unwrap();
        if open.metric != metric {
            continue;
        }
        if !open.acknowledged && now.saturating_sub(open.last_seen) <= AGGREGATION_WINDOW_SECS {
            open.value = value;
            open.threshold = threshold.max_value;
            open.severity = severity;
            open.priority = threshold.priority;
            open.occurrences = open.occurrences.saturating_add(1);
            open.last_seen = now;
            active.set(i, open.clone());
            env.storage()
                .persistent()
                .set(&AlertKey::ActiveAlerts, &active);
            return Ok(Some(open));
        }
        break;
    }

    let alert_id: u64 = env
        .storage()
        .persistent()
        .get(&AlertKey::AlertCounter)
        .unwrap_or(0)
        + 1;
    env.storage()
        .persistent()
        .set(&AlertKey::AlertCounter, &alert_id);

    let record = AlertRecord {
        alert_id,
        metric,
        value,
        threshold: threshold.max_value,
        severity,
        timestamp: now,
        acknowledged: false,
        priority: threshold.priority,
        occurrences: 1,
        last_seen: now,
    };

    if active.len() >= MAX_ALERTS {
        active.remove(0);
    }
    active.push_back(record.clone());
    env.storage()
        .persistent()
        .set(&AlertKey::ActiveAlerts, &active);

    Ok(Some(record))
}

/// Acknowledge the alert with `alert_id`, resetting its metric's escalation
/// streak so the next breach starts back at `Warning`.
pub fn acknowledge(env: &Env, alert_id: u64) -> Result<(), AlertError> {
    let mut active: Vec<AlertRecord> = env
        .storage()
        .persistent()
        .get(&AlertKey::ActiveAlerts)
        .unwrap_or_else(|| Vec::new(env));

    for i in 0..active.len() {
        let mut record = active.get(i).unwrap();
        if record.alert_id == alert_id {
            record.acknowledged = true;
            let metric = record.metric.clone();
            active.set(i, record);
            env.storage()
                .persistent()
                .set(&AlertKey::ActiveAlerts, &active);
            env.storage()
                .temporary()
                .set(&AlertKey::EscalationState(metric), &0u32);
            return Ok(());
        }
    }
    Err(AlertError::AlertNotFound)
}

/// List currently stored active alerts (most-recent-last).
pub fn list_active_alerts(env: &Env) -> Vec<AlertRecord> {
    env.storage()
        .persistent()
        .get(&AlertKey::ActiveAlerts)
        .unwrap_or_else(|| Vec::new(env))
}

/// Unacknowledged alerts at or above `min_priority`, highest priority first,
/// newest first within a priority.
pub fn list_open_alerts(env: &Env, min_priority: AlertPriority) -> Vec<AlertRecord> {
    let active = list_active_alerts(env);
    let mut out = Vec::new(env);
    for priority in [
        AlertPriority::Critical,
        AlertPriority::High,
        AlertPriority::Medium,
        AlertPriority::Low,
    ] {
        if priority < min_priority {
            break;
        }
        for record in active.iter().rev() {
            if !record.acknowledged && record.priority == priority {
                out.push_back(record);
            }
        }
    }
    out
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger as _;
    use soroban_sdk::{contract, contractimpl, symbol_short, Env};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    #[test]
    fn test_no_alert_below_threshold() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("latency");
            configure_threshold(&env, metric.clone(), 100);
            let result = evaluate_sample(&env, metric, 50).unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn test_missing_threshold_errors() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("unknown");
            let err = evaluate_sample(&env, metric, 50).unwrap_err();
            assert_eq!(err, AlertError::ThresholdNotConfigured);
        });
    }

    #[test]
    fn test_breach_escalates_severity() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("errs");
            configure_threshold(&env, metric.clone(), 10);

            let a1 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a1.severity, AlertSeverity::Warning);

            let a2 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a2.severity, AlertSeverity::Critical);

            let a3 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a3.severity, AlertSeverity::Critical);

            let a4 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a4.severity, AlertSeverity::PageOps);
        });
    }

    #[test]
    fn test_acknowledge_resets_streak() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("errs");
            configure_threshold(&env, metric.clone(), 10);

            let a1 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            let a2 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a2.severity, AlertSeverity::Critical);

            acknowledge(&env, a1.alert_id).unwrap();

            let a3 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(a3.severity, AlertSeverity::Warning);
        });
    }

    #[test]
    fn test_acknowledge_unknown_alert_errors() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let err = acknowledge(&env, 999).unwrap_err();
            assert_eq!(err, AlertError::AlertNotFound);
        });
    }

    #[test]
    fn test_healthy_sample_resets_streak() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("errs");
            configure_threshold(&env, metric.clone(), 10);

            evaluate_sample(&env, metric.clone(), 15).unwrap();
            evaluate_sample(&env, metric.clone(), 15).unwrap();
            // Healthy sample resets streak.
            let healthy = evaluate_sample(&env, metric.clone(), 1).unwrap();
            assert!(healthy.is_none());

            let next = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_eq!(next.severity, AlertSeverity::Warning);
        });
    }

    #[test]
    fn test_repeated_breaches_fold_into_one_alert() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let metric = symbol_short!("errs");
            configure_threshold(&env, metric.clone(), 10);

            let a1 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            env.ledger().set_timestamp(60);
            let a2 = evaluate_sample(&env, metric.clone(), 20).unwrap().unwrap();
            assert_eq!(a2.alert_id, a1.alert_id);
            assert_eq!((a2.occurrences, a2.value, a2.last_seen), (2, 20, 60));
            assert_eq!(list_active_alerts(&env).len(), 1);

            // A breach after the window opens a new alert.
            env.ledger().set_timestamp(60 + AGGREGATION_WINDOW_SECS + 1);
            let a3 = evaluate_sample(&env, metric.clone(), 15).unwrap().unwrap();
            assert_ne!(a3.alert_id, a1.alert_id);
            assert_eq!(a3.occurrences, 1);
            assert_eq!(list_active_alerts(&env).len(), 2);
        });
    }

    #[test]
    fn test_other_metrics_do_not_fold() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            configure_threshold(&env, symbol_short!("a"), 1);
            configure_threshold(&env, symbol_short!("b"), 1);
            let a = evaluate_sample(&env, symbol_short!("a"), 5)
                .unwrap()
                .unwrap();
            let b = evaluate_sample(&env, symbol_short!("b"), 5)
                .unwrap()
                .unwrap();
            let a2 = evaluate_sample(&env, symbol_short!("a"), 5)
                .unwrap()
                .unwrap();
            assert_ne!(a.alert_id, b.alert_id);
            assert_eq!(a2.alert_id, a.alert_id);
        });
    }

    #[test]
    fn test_open_alerts_respect_priority() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            configure_threshold_with_priority(&env, symbol_short!("low"), 1, AlertPriority::Low);
            configure_threshold_with_priority(
                &env,
                symbol_short!("crit"),
                1,
                AlertPriority::Critical,
            );
            configure_threshold(&env, symbol_short!("med"), 1);
            let low = evaluate_sample(&env, symbol_short!("low"), 5)
                .unwrap()
                .unwrap();
            let crit = evaluate_sample(&env, symbol_short!("crit"), 5)
                .unwrap()
                .unwrap();
            let med = evaluate_sample(&env, symbol_short!("med"), 5)
                .unwrap()
                .unwrap();
            assert_eq!(med.priority, AlertPriority::Medium);

            let ids = |min| {
                let mut v = std::vec::Vec::new();
                for r in list_open_alerts(&env, min).iter() {
                    v.push(r.alert_id);
                }
                v
            };
            assert_eq!(
                ids(AlertPriority::Low),
                [crit.alert_id, med.alert_id, low.alert_id]
            );
            assert_eq!(ids(AlertPriority::High), [crit.alert_id]);

            acknowledge(&env, crit.alert_id).unwrap();
            assert_eq!(ids(AlertPriority::Medium), [med.alert_id]);
        });
    }
}
