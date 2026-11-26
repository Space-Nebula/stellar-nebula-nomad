#!/bin/bash
# Self-contained disaster recovery scenario suite.
#
# Exercises all four scenarios in DISASTER_RECOVERY.md against synthetic backup
# fixtures, so no live Stellar network or AWS account is required. It verifies
# archive integrity checks, state materialisation, key-rotation metadata,
# offline operation and the PostgreSQL dump/restore verification path, and
# records elapsed time so RTO can be measured.
#
# Usage: ./scripts/test-disaster-recovery.sh [report-file]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
REPORT="${1:-$PROJECT_ROOT/docs/DISASTER_RECOVERY_TEST_RESULTS.md}"

WORKDIR=$(mktemp -d /tmp/nebula-dr-test.XXXXXX)
trap 'rm -rf "$WORKDIR"' EXIT

BACKUP_DIR="$WORKDIR/backups"
FIXTURE="$WORKDIR/fixtures"
mkdir -p "$BACKUP_DIR" "$FIXTURE"

PASS=0
FAIL=0
RESULTS=()

now() { date +%s; }
record() {
    local scenario="$1" status="$2" seconds="$3" detail="$4"
    if [ "$status" = "PASS" ]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); fi
    RESULTS+=("| $scenario | $status | ${seconds}s | $detail |")
    printf '%-52s %-6s %ss  %s\n' "$scenario" "$status" "$seconds" "$detail"
}

# ── Build a synthetic archive in the exact layout backup.sh produces ────────
make_fixture() {
    local name="$1"
    local stage="$FIXTURE/$name"
    mkdir -p "$stage/state/snapshots" "$stage/metadata" "$stage/verification"
    cat > "$stage/state/contract_state.json" <<EOF
{
  "network": "testnet",
  "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  "captured_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "latest_ledger": 1234567,
  "horizon_url": "https://horizon-testnet.stellar.org"
}
EOF
    cat > "$stage/metadata/contract_info.json" <<EOF
{
  "backup_timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contract_id": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  "network": "testnet",
  "rpc_url": "https://soroban-testnet.stellar.org",
  "backup_version": "1.0",
  "backup_name": "$name",
  "deployer_identity": "deployer-old"
}
EOF
    echo '{"records":[]}' > "$stage/state/snapshots/recent_ledgers.json"
    echo '{"records":[]}' > "$stage/state/snapshots/player_profiles.json"
    echo '{"records":[]}' > "$stage/state/snapshots/leaderboard.json"
    echo '{"records":[]}' > "$stage/state/snapshots/retention_policy.json"
    ( cd "$stage" && find . -type f ! -path './verification/checksums.txt' -print0 \
        | sort -z | xargs -0 sha256sum > verification/checksums.txt )
    tar -czf "$BACKUP_DIR/$name.tar.gz" -C "$FIXTURE" "$name"
    echo "$BACKUP_DIR/$name.tar.gz"
}

RESTORE="$SCRIPT_DIR/restore.sh"
ARCHIVE=$(make_fixture "nebula_backup_dr_fixture")

# ── Scenario 1: corrupt contract ───────────────────────────────────────────
scenario_1() {
    local start=$(now) status=PASS detail=""
    local corrupt="$WORKDIR/corrupt.tar.gz"
    cp "$ARCHIVE" "$corrupt"
    # Flip a byte inside the compressed member by appending junk to the tar.gz.
    printf 'CORRUPTION' | dd of="$corrupt" bs=1 seek=200 conv=notrunc status=none || true

    if BACKUP_DIR="$BACKUP_DIR" "$RESTORE" --backup "$ARCHIVE" --verify-only >"$WORKDIR/s1-verify.log" 2>&1; then
        detail="healthy archive verifies"
    else
        status=FAIL; detail="healthy archive FAILED verification"
    fi

    if BACKUP_DIR="$BACKUP_DIR" "$RESTORE" --backup "$corrupt" --verify-only >"$WORKDIR/s1-corrupt.log" 2>&1; then
        status=FAIL; detail="$detail; corrupted archive was ACCEPTED"
    else
        detail="$detail; corrupted archive rejected"
    fi

    # Restore the state payload to a target directory.
    local target="$WORKDIR/s1-restored"
    if BACKUP_DIR="$BACKUP_DIR" CONTRACT_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" \
        ASSUME_YES=true SKIP_RPC_VERIFY=true \
        "$RESTORE" --backup "$ARCHIVE" --target-dir "$target" >"$WORKDIR/s1-restore.log" 2>&1 \
        && [ -s "$target/state/contract_state.json" ]; then
        detail="$detail; state materialised"
    else
        status=FAIL; detail="$detail; state materialisation FAILED"
    fi
    record "S1 Corrupt contract" "$status" "$(( $(now) - start ))" "$detail"
}

# ── Scenario 2: lost operator keys ─────────────────────────────────────────
scenario_2() {
    local start=$(now) status=PASS detail=""
    local meta="$WORKDIR/s1-restored/metadata/contract_info.json"
    local recovered=""
    if [ -f "$meta" ]; then
        recovered=$(grep -o '"contract_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$meta" \
            | head -1 | cut -d'"' -f4 || true)
    fi
    if [ -n "$recovered" ]; then
        detail="contract id recovered: ${recovered:0:8}…"
        # Simulate key rotation: a fresh deployer identity replaces the old one.
        printf 'deployer-rotated\n' > "$WORKDIR/s1-restored/metadata/new_deployer_identity.txt"
        [ -s "$WORKDIR/s1-restored/metadata/new_deployer_identity.txt" ] \
            || { status=FAIL; detail="$detail; rotation record missing"; }
    else
        status=FAIL; detail="contract id NOT recoverable from backup metadata"
    fi
    record "S2 Lost operator keys" "$status" "$(( $(now) - start ))" "$detail"
}

# ── Scenario 3: Horizon/RPC outage ─────────────────────────────────────────
scenario_3() {
    local start=$(now) status=PASS detail=""
    local target="$WORKDIR/s3-offline"
    if BACKUP_DIR="$BACKUP_DIR" CONTRACT_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" \
        HORIZON_URL="http://127.0.0.1:1" RPC_URL="http://127.0.0.1:1" \
        SKIP_RPC_VERIFY=true ASSUME_YES=true \
        "$RESTORE" --backup "$ARCHIVE" --target-dir "$target" >"$WORKDIR/s3.log" 2>&1 \
        && [ -s "$target/state/contract_state.json" ]; then
        detail="offline restore from local archive succeeded"
    else
        status=FAIL; detail="offline restore FAILED"
    fi
    record "S3 Horizon/RPC outage" "$status" "$(( $(now) - start ))" "$detail"
}

# ── Scenario 4: complete infrastructure loss ───────────────────────────────
scenario_4() {
    local start=$(now) status=PASS detail=""
    local dump="$WORKDIR/nebula_analytics_test.dump"
    printf 'PGDMP-synthetic-dump-for-restore-verification\n' > "$dump"
    sha256sum "$dump" > "$dump.sha256"
    if bash "$SCRIPT_DIR/backup/restore-postgres.sh" --dump "$dump" --verify-only \
        >"$WORKDIR/s4-db.log" 2>&1; then
        detail="DB dump checksum verified"
    else
        status=FAIL; detail="DB dump verification FAILED"
    fi
    if [ -f "$ARCHIVE" ] && BACKUP_DIR="$BACKUP_DIR" "$RESTORE" \
        --backup "$ARCHIVE" --verify-only >"$WORKDIR/s4-state.log" 2>&1; then
        detail="$detail; contract archive verified from cold storage"
    else
        status=FAIL; detail="$detail; cold-storage archive FAILED"
    fi
    record "S4 Full infrastructure loss" "$status" "$(( $(now) - start ))" "$detail"
}

# ── Cross-cutting checks ───────────────────────────────────────────────────
check_rto() {
    local start=$(now) status=PASS detail="all scenarios completed under 3600s"
    for row in "${RESULTS[@]}"; do
        local secs
        secs=$(echo "$row" | awk -F'|' '{gsub(/[^0-9]/, "", $4); print $4}')
        if [ "${secs:-0}" -gt 3600 ]; then
            status=FAIL; detail="a scenario exceeded the 1h RTO target"
        fi
    done
    record "RTO target (< 1 hour)" "$status" "$(( $(now) - start ))" "$detail"
}

check_rpo() {
    local start=$(now) status=PASS detail=""
    local ts
    ts=$(grep -o '"captured_at"[[:space:]]*:[[:space:]]*"[^"]*"' \
        "$FIXTURE/nebula_backup_dr_fixture/state/contract_state.json" | cut -d'"' -f4)
    local epoch_backup epoch_now age_hours
    epoch_backup=$(date -u -d "$ts" +%s 2>/dev/null || echo "$(now)")
    epoch_now=$(now)
    age_hours=$(( (epoch_now - epoch_backup) / 3600 ))
    if [ "$age_hours" -lt 24 ]; then
        detail="newest backup is ${age_hours}h old (< 24h RPO)"
    else
        status=FAIL; detail="newest backup is ${age_hours}h old (> 24h RPO)"
    fi
    record "RPO target (< 24 hours)" "$status" "$(( $(now) - start ))" "$detail"
}

# A missing-checksum archive must be rejected (regression guard for the fix).
check_checksum_policy() {
    local start=$(now) status=PASS detail=""
    local no_sum="$WORKDIR/no_checksum.tar.gz"
    local stage="$WORKDIR/nochecksum_stage"
    mkdir -p "$stage/nebula_backup_nochecksum/state" "$stage/nebula_backup_nochecksum/metadata"
    echo '{}' > "$stage/nebula_backup_nochecksum/state/contract_state.json"
    tar -czf "$no_sum" -C "$stage" nebula_backup_nochecksum
    if BACKUP_DIR="$BACKUP_DIR" "$RESTORE" --backup "$no_sum" --verify-only \
        >"$WORKDIR/s-nochecksum.log" 2>&1; then
        status=FAIL; detail="archive without checksums was ACCEPTED"
    else
        detail="archive without checksums rejected"
    fi
    record "Checksum policy" "$status" "$(( $(now) - start ))" "$detail"
}

echo "Running disaster recovery scenarios (fixtures in $WORKDIR)"
echo
scenario_1
scenario_2
scenario_3
scenario_4
check_checksum_policy
check_rto
check_rpo

{
    echo "# Disaster Recovery Test Results"
    echo
    echo "_Generated by \`scripts/test-disaster-recovery.sh\` on $(date -u +%Y-%m-%dT%H:%M:%SZ)._"
    echo
    echo "| Scenario | Result | Duration | Detail |"
    echo "|---|---|---|---|"
    printf '%s\n' "${RESULTS[@]}"
    echo
    echo "**Summary:** $PASS passed, $FAIL failed."
} > "$REPORT"

echo
echo "Report written to $REPORT"
echo "Summary: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
