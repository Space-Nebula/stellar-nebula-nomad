#!/bin/bash
# Automated Backup Script for Stellar Nebula Nomad Contract
# Performs daily automated backups of contract state

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BACKUP_DIR="${BACKUP_DIR:-$PROJECT_ROOT/backups}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="nebula_backup_${TIMESTAMP}"
LOG_FILE="${BACKUP_DIR}/backup.log"

# Stellar configuration
NETWORK="${STELLAR_NETWORK:-testnet}"
CONTRACT_ID="${CONTRACT_ID:-}"
RPC_URL="${RPC_URL:-https://soroban-testnet.stellar.org}"

# Backup retention (days)
RETENTION_DAYS="${RETENTION_DAYS:-30}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Logging functions
log() {
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$LOG_FILE"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$LOG_FILE"
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."
    
    if ! command -v stellar &> /dev/null; then
        log_error "Stellar CLI not found. Please install it first."
        exit 1
    fi
    
    if [ -z "$CONTRACT_ID" ]; then
        log_error "CONTRACT_ID environment variable not set"
        exit 1
    fi
    
    log_success "Prerequisites check passed"
}

# Create backup directory structure
setup_backup_dir() {
    log "Setting up backup directory..."
    
    mkdir -p "$BACKUP_DIR/$BACKUP_NAME"
    mkdir -p "$BACKUP_DIR/$BACKUP_NAME/state"
    mkdir -p "$BACKUP_DIR/$BACKUP_NAME/metadata"
    mkdir -p "$BACKUP_DIR/$BACKUP_NAME/verification"
    
    log_success "Backup directory created: $BACKUP_DIR/$BACKUP_NAME"
}

# Export contract state
export_contract_state() {
    log "Exporting contract state..."
    
    local state_file="$BACKUP_DIR/$BACKUP_NAME/state/contract_state.json"
    
    # Export contract storage entries
    stellar contract read \
        --id "$CONTRACT_ID" \
        --network "$NETWORK" \
        --rpc-url "$RPC_URL" \
        > "$state_file" 2>&1 || {
        log_error "Failed to export contract state"
        return 1
    }
    
    log_success "Contract state exported to $state_file"
}

# Export contract metadata
export_metadata() {
    log "Exporting contract metadata..."
    
    local metadata_file="$BACKUP_DIR/$BACKUP_NAME/metadata/contract_info.json"
    
    cat > "$metadata_file" <<EOF
{
  "backup_timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "contract_id": "$CONTRACT_ID",
  "network": "$NETWORK",
  "rpc_url": "$RPC_URL",
  "backup_version": "1.0",
  "backup_name": "$BACKUP_NAME"
}
EOF
    
    log_success "Metadata exported to $metadata_file"
}

# Export contract WASM
export_contract_wasm() {
    log "Exporting contract WASM..."
    
    local wasm_file="$BACKUP_DIR/$BACKUP_NAME/state/contract.wasm"
    
    stellar contract fetch \
        --id "$CONTRACT_ID" \
        --network "$NETWORK" \
        --rpc-url "$RPC_URL" \
        --out-file "$wasm_file" 2>&1 || {
        log_warning "Failed to export contract WASM (may not be supported)"
        return 0
    }
    
    log_success "Contract WASM exported to $wasm_file"
}

# Create state snapshots for key data
create_state_snapshots() {
    log "Creating state snapshots..."
    
    local snapshot_dir="$BACKUP_DIR/$BACKUP_NAME/state/snapshots"
    mkdir -p "$snapshot_dir"
    
    # Export player profiles
    log "Exporting player profiles..."
    stellar contract invoke \
        --id "$CONTRACT_ID" \
        --network "$NETWORK" \
        --rpc-url "$RPC_URL" \
        -- get_global_stats \
        > "$snapshot_dir/global_stats.json" 2>&1 || log_warning "Failed to export global stats"
    
    # Export leaderboard
    log "Exporting leaderboard..."
    stellar contract invoke \
        --id "$CONTRACT_ID" \
        --network "$NETWORK" \
        --rpc-url "$RPC_URL" \
        -- snapshot_leaderboard \
        --top_n 100 \
        > "$snapshot_dir/leaderboard.json" 2>&1 || log_warning "Failed to export leaderboard"
    
    log_success "State snapshots created"
}

# Calculate checksums for verification
calculate_checksums() {
    log "Calculating checksums..."
    
    local checksum_file="$BACKUP_DIR/$BACKUP_NAME/verification/checksums.txt"
    
    cd "$BACKUP_DIR/$BACKUP_NAME"
    find . -type f -exec sha256sum {} \; > "$checksum_file"
    cd - > /dev/null
    
    log_success "Checksums calculated and saved to $checksum_file"
}

# Compress backup
compress_backup() {
    log "Compressing backup..."
    
    local archive_file="$BACKUP_DIR/${BACKUP_NAME}.tar.gz"
    
    tar -czf "$archive_file" -C "$BACKUP_DIR" "$BACKUP_NAME" || {
        log_error "Failed to compress backup"
        return 1
    }
    
    # Remove uncompressed directory
    rm -rf "$BACKUP_DIR/$BACKUP_NAME"
    
    local size=$(du -h "$archive_file" | cut -f1)
    log_success "Backup compressed to $archive_file (Size: $size)"
}

# Upload to off-chain storage (optional)
upload_to_storage() {
    log "Uploading to off-chain storage..."
    
    local archive_file="$BACKUP_DIR/${BACKUP_NAME}.tar.gz"
    
    # S3 upload (if configured)
    if [ -n "$S3_BUCKET" ]; then
        log "Uploading to S3 bucket: $S3_BUCKET"
        aws s3 cp "$archive_file" "s3://$S3_BUCKET/backups/" || {
            log_warning "Failed to upload to S3"
            return 0
        }
        log_success "Uploaded to S3"
    fi
    
    # IPFS upload (if configured)
    if command -v ipfs &> /dev/null && [ "$IPFS_UPLOAD" = "true" ]; then
        log "Uploading to IPFS..."
        local ipfs_hash=$(ipfs add -Q "$archive_file")
        echo "$ipfs_hash" > "$BACKUP_DIR/${BACKUP_NAME}_ipfs.txt"
        log_success "Uploaded to IPFS: $ipfs_hash"
    fi
}

# Clean old backups
cleanup_old_backups() {
    log "Cleaning up old backups (retention: $RETENTION_DAYS days)..."
    
    find "$BACKUP_DIR" -name "nebula_backup_*.tar.gz" -type f -mtime +$RETENTION_DAYS -delete
    
    local remaining=$(find "$BACKUP_DIR" -name "nebula_backup_*.tar.gz" -type f | wc -l)
    log_success "Cleanup complete. Remaining backups: $remaining"
}

# Verify backup integrity
verify_backup() {
    log "Verifying backup integrity..."
    
    local archive_file="$BACKUP_DIR/${BACKUP_NAME}.tar.gz"
    
    # Test archive integrity
    tar -tzf "$archive_file" > /dev/null 2>&1 || {
        log_error "Backup archive is corrupted!"
        return 1
    }
    
    # Extract and verify checksums
    local temp_dir=$(mktemp -d)
    tar -xzf "$archive_file" -C "$temp_dir"
    
    cd "$temp_dir/$BACKUP_NAME"
    sha256sum -c verification/checksums.txt > /dev/null 2>&1 || {
        log_error "Checksum verification failed!"
        rm -rf "$temp_dir"
        return 1
    }
    cd - > /dev/null
    
    rm -rf "$temp_dir"
    
    log_success "Backup integrity verified"
}

# Send notification
send_notification() {
    local status=$1
    local message=$2
    
    if [ -n "$WEBHOOK_URL" ]; then
        curl -X POST "$WEBHOOK_URL" \
            -H "Content-Type: application/json" \
            -d "{\"status\": \"$status\", \"message\": \"$message\", \"backup\": \"$BACKUP_NAME\"}" \
            > /dev/null 2>&1 || log_warning "Failed to send notification"
    fi
}

# Main backup process
main() {
    log "=========================================="
    log "Starting backup process: $BACKUP_NAME"
    log "=========================================="
    
    local start_time=$(date +%s)
    
    # Execute backup steps
    check_prerequisites
    setup_backup_dir
    export_contract_state
    export_metadata
    export_contract_wasm
    create_state_snapshots
    calculate_checksums
    compress_backup
    verify_backup
    upload_to_storage
    cleanup_old_backups
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log "=========================================="
    log_success "Backup completed successfully in ${duration}s"
    log "Backup location: $BACKUP_DIR/${BACKUP_NAME}.tar.gz"
    log "=========================================="
    
    send_notification "success" "Backup completed successfully in ${duration}s"
}

# Error handling
trap 'log_error "Backup failed with error"; send_notification "error" "Backup failed"; exit 1' ERR

# Run main process
main "$@"
