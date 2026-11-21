//! Regression guards for the disaster-recovery runbook and scripts.
//!
//! The executable scenario suite lives in `scripts/test-disaster-recovery.sh`
//! (it needs Bash, tar and sha256sum). These tests assert that the documented
//! recovery assets stay present and that the previously discovered gaps do not
//! silently regress: the restore placeholder, the missing checksum rejection,
//! the backup retention floor and the four documented scenarios.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("missing {relative}: {e}"))
}

#[test]
fn recovery_scripts_exist_and_are_executable() {
    for script in [
        "scripts/backup.sh",
        "scripts/restore.sh",
        "scripts/backup/backup-postgres.sh",
        "scripts/backup/restore-postgres.sh",
        "scripts/test-disaster-recovery.sh",
    ] {
        let path = repo_root().join(script);
        assert!(path.is_file(), "{script} is missing");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{script} is not executable");
        }
    }
}

#[test]
fn restore_rejects_archives_without_checksums() {
    let restore = read("scripts/restore.sh");
    assert!(
        restore.contains("No checksums file found") && restore.contains("return 1"),
        "restore.sh must fail when verification/checksums.txt is absent"
    );
    assert!(
        !restore.contains("This is a placeholder for the restoration logic"),
        "the old restore placeholder must not return"
    );
}

#[test]
fn backup_enforces_a_minimum_retention_floor() {
    let backup = read("scripts/backup.sh");
    assert!(backup.contains("BACKUP_MIN_KEEP"), "backup.sh lost the retention floor");
    assert!(
        backup.contains("snapshots_succeeded"),
        "backup.sh must record Horizon snapshot success"
    );
}

#[test]
fn runbook_documents_all_four_scenarios_and_targets() {
    let runbook = read("DISASTER_RECOVERY.md");
    for scenario in [
        "Scenario 1",
        "Scenario 2",
        "Scenario 3",
        "Scenario 4",
    ] {
        assert!(runbook.contains(scenario), "runbook missing {scenario}");
    }
    assert!(runbook.contains("< 1 hour"), "runbook missing RTO target");
    assert!(runbook.contains("< 24 hours"), "runbook missing RPO target");
    assert!(
        runbook.contains("DISASTER_RECOVERY_TEST_RESULTS.md"),
        "runbook must link the test results"
    );
}

#[test]
fn scenario_suite_report_records_a_passing_run() {
    let report = read("docs/DISASTER_RECOVERY_TEST_RESULTS.md");
    assert!(report.contains("S1 Corrupt contract"), "missing S1 result");
    assert!(report.contains("S2 Lost operator keys"), "missing S2 result");
    assert!(report.contains("S3 Horizon/RPC outage"), "missing S3 result");
    assert!(report.contains("S4 Full infrastructure loss"), "missing S4 result");
    assert!(report.contains("7 passed, 0 failed"), "scenario suite is not all green");
}

#[test]
fn scenario_suite_can_be_re_run_offline() {
    let output = Command::new("bash")
        .arg(repo_root().join("scripts/test-disaster-recovery.sh"))
        .arg(repo_root().join("target/dr-test-results.md"))
        .output()
        .expect("failed to run the DR scenario suite");
    assert!(
        output.status.success(),
        "scenario suite failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = fs::read_to_string(repo_root().join("target/dr-test-results.md"))
        .expect("scenario suite did not write a report");
    assert!(report.contains("7 passed, 0 failed"));
}
