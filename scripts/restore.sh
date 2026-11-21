#!/bin/bash
# Disaster Recovery Script for Stellar Nebula Nomad contract backups.
#
# Soroban does not expose a general "import ledger entries" API: arbitrary
# contract state cannot be written back into a deployed contract from a backup
# archive. This script therefore does the part that IS verifiable offline:
#   1. verify the archive and its SHA-256 checksums (fatal on any mismatch),
#   2. materialise the archived state/metadata/WASM into a target directory,
#   3. verify the materialised files, and
#   4. emit an explicit checklist for the remaining on-chain steps.
#
# It never claims "SUCCESS" for on-chain restoration; see generate_restore_report.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKUP_DIR="${BACKUP_DIR:-$PROJECT_ROOT/backups}"
mkdir -p "$BACKUP_DIR"
LOG_FILE="${BACKUP_DIR}/restore.log"

NETWORK="${STELLAR_NETWORK:-testnet}"
CONTRACT_ID="${CONTRACT_ID:-}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"
SKIP_RPC_VERIFY="${SKIP_RPC_VERIFY:-false}"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

log()         { echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOG_FILE"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$LOG_FILE"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$LOG_FILE"; }
log_info()    { echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$LOG_FILE"; }

usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Restore a Stellar Nebula Nomad backup archive.

OPTIONS:
    -b, --backup FILE       Backup archive to restore from (required)
    -c, --contract ID       Target contract ID (required for a live restore)
    -n, --network NAME      Network name (testnet/mainnet), default: testnet
    -t, --target-dir DIR    Where to materialise the archived state
    -d, --dry-run           Validate and report without writing any files
    -v, --verify-only       Only verify archive integrity and checksums
        --test-mode         Verify + dry-run (used by backup.sh --test-restore)
    -h, --help              Show this help

MODE SEMANTICS:
    --verify-only   archive + checksum verification only
    --dry-run       verification + what-if report, no files written
    (default)       verification + materialise state into --target-dir, then
                    print the manual on-chain steps that still must be done

ENVIRONMENT:
    CONTRACT_ID, STELLAR_NETWORK, RPC_URL, BACKUP_DIR, TARGET_DIR,
    SKIP_RPC_VERIFY=true  (skip the best-effort live contract probe),
    WEBHOOK_URL
EOF
    exit 1
}

BACKUP_FILE=""
DRY_RUN=false
VERIFY_ONLY=false
TARGET_DIR="${TARGET_DIR:-}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        -b|--backup)     BACKUP_FILE="$2"; shift 2 ;;
        -c|--contract)   CONTRACT_ID="$2"; shift 2 ;;
        -n|--network)    NETWORK="$2"; shift 2 ;;
        -t|--target-dir) TARGET_DIR="$2"; shift 2 ;;
        -d|--dry-run)    DRY_RUN=true; shift ;;
        -v|--verify-only) VERIFY_ONLY=true; shift ;;
        --test-mode)     VERIFY_ONLY=true; DRY_RUN=true; shift ;;
        -h|--help)       usage ;;
        *) log_error "Unknown option: $1"; usage ;;
    esac
done

[ -n "$BACKUP_FILE" ] || { log_error "Backup file not specified"; usage; }
[ -f "$BACKUP_FILE" ] || { log_error "Backup file not found: $BACKUP_FILE"; exit 1; }

TEMP_DIR=""
MATERIALISED=false
REPORT_STATUS=""

check_prerequisites() {
    log "Checking prerequisites..."
    if [ "$VERIFY_ONLY" = false ] && [ "$DRY_RUN" = false ] && [ -z "$CONTRACT_ID" ]; then
        log_error "CONTRACT_ID is required for a live restore (or use --dry-run)."
        exit 1
    fi
    log_success "Prerequisites check passed"
}

extract_backup() {
    log "Extracting backup..."
    TEMP_DIR=$(mktemp -d)
    tar -xzf "$BACKUP_FILE" -C "$TEMP_DIR" || {
        log_error "Failed to extract backup"; exit 1;
    }
    local count
    count=$(find "$TEMP_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l)
    if [ "$count" -ne 1 ]; then
        log_error "Expected exactly one top-level directory in archive, found $count"
        exit 1
    fi
    BACKUP_NAME=$(find "$TEMP_DIR" -mindepth 1 -maxdepth 1 -type d -print -quit | xargs basename)
    EXTRACT_DIR="$TEMP_DIR/$BACKUP_NAME"
    log_success "Backup extracted to $EXTRACT_DIR"
}

verify_backup_integrity() {
    log "Verifying backup integrity..."
    tar -tzf "$BACKUP_FILE" > /dev/null 2>&1 || {
        log_error "Backup archive is corrupted!"; return 1;
    }
    if [ ! -f "$EXTRACT_DIR/verification/checksums.txt" ]; then
        log_error "No checksums file found — refusing to restore an unverified archive."
        return 1
    fi
    ( cd "$EXTRACT_DIR" && sha256sum -c verification/checksums.txt ) || {
        log_error "Checksum verification failed!"; return 1;
    }
    if [ ! -f "$EXTRACT_DIR/state/contract_state.json" ]; then
        log_error "Archive is missing state/contract_state.json"; return 1
    fi
    log_success "Backup integrity verified"
}

display_backup_info() {
    log_info "=========================================="
    log_info "Backup Information"
    log_info "=========================================="
    if [ -f "$EXTRACT_DIR/metadata/contract_info.json" ]; then
        while IFS= read -r line; do log_info "$line"; done \
            < "$EXTRACT_DIR/metadata/contract_info.json"
    else
        log_warning "No metadata/contract_info.json in archive"
    fi
    log_info "=========================================="
}

confirm_restore() {
    if [ "$DRY_RUN" = true ]; then
        log_warning "DRY RUN MODE - No files will be written"
        return 0
    fi
    log_warning "=========================================="
    log_warning "This will materialise archived state to: ${TARGET_DIR:-$BACKUP_DIR/restored/$BACKUP_NAME}"
    log_warning "Target Contract: $CONTRACT_ID"
    log_warning "Network: $NETWORK"
    log_warning "=========================================="
    if [ "${ASSUME_YES:-false}" != "true" ]; then
        read -p "Are you sure you want to continue? (yes/no): " -r
        echo
        if [[ ! $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
            log "Restore cancelled by user"; exit 0
        fi
    fi
}

restore_contract_state() {
    log "Restoring archived state payload..."
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would materialise: $EXTRACT_DIR/state/contract_state.json"
        return 0
    fi

    local dest="${TARGET_DIR:-$BACKUP_DIR/restored/$BACKUP_NAME}"
    mkdir -p "$dest/state" "$dest/metadata" "$dest/verification"
    cp -a "$EXTRACT_DIR/state/." "$dest/state/"
    [ ! -d "$EXTRACT_DIR/metadata" ] || cp -a "$EXTRACT_DIR/metadata/." "$dest/metadata/"
    [ ! -f "$EXTRACT_DIR/verification/checksums.txt" ] || \
        cp "$EXTRACT_DIR/verification/checksums.txt" "$dest/verification/"

    [ -s "$dest/state/contract_state.json" ] || {
        log_error "Materialised state file is missing or empty"; return 1;
    }
    MATERIALISED=true
    MATERIALISED_DIR="$dest"
    log_success "State payload materialised to $dest"

    log_warning "Soroban cannot import arbitrary ledger entries. On-chain steps still required:"
    log_info "  1. Deploy the archived WASM (state/contract.wasm) or rebuild from the recorded commit."
    log_info "  2. Re-initialize the contract from metadata/contract_info.json and state/contract_state.json."
    log_info "  3. Replay/reconcile the exported effects, operations and snapshots in state/snapshots/."
    log_info "  4. Re-point deployment/aliases and run scripts/verify.sh before resuming traffic."
}

restore_snapshots() {
    log "Restoring state snapshots..."
    local src="$EXTRACT_DIR/state/snapshots"
    if [ ! -d "$src" ]; then
        log_warning "No snapshots directory found in archive"
        return 0
    fi
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would restore snapshots from: $src"
        return 0
    fi
    mkdir -p "$MATERIALISED_DIR/state/snapshots"
    cp -a "$src/." "$MATERIALISED_DIR/state/snapshots/"
    log_success "Snapshots materialised to $MATERIALISED_DIR/state/snapshots/"
}

verify_restored_state() {
    log "Verifying materialised state..."
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY RUN] Would verify materialised state"
        return 0
    fi
    [ "$MATERIALISED" = true ] || { log_error "State was not materialised"; return 1; }
    for required in state/contract_state.json; do
        [ -s "$MATERIALISED_DIR/$required" ] || {
            log_error "Missing materialised file: $required"; return 1;
        }
    done
    log_success "Materialised state verified"

    if [ "$SKIP_RPC_VERIFY" = true ] || ! command -v stellar >/dev/null 2>&1; then
        log_info "Skipping live RPC probe (offline operation / CLI unavailable)"
        return 0
    fi
    if stellar contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
        --rpc-url "$RPC_URL" -- get_global_stats >/dev/null 2>&1; then
        log_success "Live contract probe succeeded"
    else
        # Non-fatal: a Horizon/RPC outage is Scenario 3; the local payload is
        # still verified and can be applied once RPC returns.
        log_warning "Live contract probe failed — RPC may be unavailable (Scenario 3). Offline payload is intact."
    fi
}

generate_restore_report() {
    log "Generating restore report..."
    local report_file="$BACKUP_DIR/restore_report_$(date +%Y%m%d_%H%M%S).txt"
    if [ "$VERIFY_ONLY" = true ]; then
        REPORT_STATUS="VERIFIED_ONLY"
    elif [ "$DRY_RUN" = true ]; then
        REPORT_STATUS="DRY_RUN"
    else
        REPORT_STATUS="STATE_MATERIALISED_MANUAL_ONCHAIN_STEPS_REQUIRED"
    fi

    cat > "$report_file" <<EOF
========================================
Disaster Recovery Report
========================================
Restore Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Backup File: $BACKUP_FILE
Contract ID: ${CONTRACT_ID:-N/A}
Network: $NETWORK
Verify Only: $VERIFY_ONLY
Dry Run: $DRY_RUN
Status: $REPORT_STATUS

Backup Information:
$(cat "$EXTRACT_DIR/metadata/contract_info.json" 2>/dev/null || echo "N/A")

Restored Components:
- Contract State (archived): $([ -f "$EXTRACT_DIR/state/contract_state.json" ] && echo "YES" || echo "NO")
- Contract WASM: $([ -f "$EXTRACT_DIR/state/contract.wasm" ] && echo "YES" || echo "NO")
- Snapshots: $([ -d "$EXTRACT_DIR/state/snapshots" ] && echo "YES" || echo "NO")
- State Materialised: $MATERIALISED (${MATERIALISED_DIR:-N/A})

NOTE: On-chain state restoration is a manual step; Soroban does not support
importing arbitrary ledger entries. See DISASTER_RECOVERY.md Scenario 1.

Recovery Time: ${SECONDS}s
========================================
EOF
    log_success "Restore report generated: $report_file"
    cat "$report_file"
}

cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"; log "Cleaned up temporary files"
    fi
}

send_notification() {
    local status=$1 message=$2
    if [ -n "$WEBHOOK_URL" ]; then
        curl -X POST "$WEBHOOK_URL" -H "Content-Type: application/json" \
            -d "{\"status\": \"$status\", \"message\": \"$message\", \"contract\": \"$CONTRACT_ID\"}" \
            > /dev/null 2>&1 || log_warning "Failed to send notification"
    fi
}

main() {
    log "=========================================="
    log "Starting restore process"
    log "=========================================="
    local start_time=$(date +%s)

    check_prerequisites
    extract_backup
    verify_backup_integrity
    display_backup_info

    if [ "$VERIFY_ONLY" = true ]; then
        log_success "Verification complete - backup is valid"
        REPORT_STATUS="VERIFIED_ONLY"
        generate_restore_report
        cleanup
        exit 0
    fi

    confirm_restore
    restore_contract_state
    restore_snapshots
    verify_restored_state
    generate_restore_report

    local duration=$(( $(date +%s) - start_time ))
    log_success "Restore workflow completed in ${duration}s ($REPORT_STATUS)"
    send_notification "success" "Restore workflow completed: $REPORT_STATUS in ${duration}s"
    cleanup
}

trap 'log_error "Restore failed"; cleanup; send_notification "error" "Restore failed"; exit 1' ERR
main "$@"
