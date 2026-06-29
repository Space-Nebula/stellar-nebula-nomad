#!/usr/bin/env bash
set -euo pipefail

# Rollback script for Nebula Nomad contract.
# Reverts to a previously deployed version by re-deploying a prior WASM artifact.
# Usage:
#   ./scripts/rollback.sh <network> [identity] [--to PREVIOUS_HASH]
# Examples:
#   ./scripts/rollback.sh futurenet default
#   ./scripts/rollback.sh testnet admin --to <sha256>

NETWORK="${1:?Usage: $0 <network> [identity] [--to <hash>]}"
IDENTITY="${2:-default}"
TARGET_HASH=""
shift 2 || true

while [[ $# -gt 0 ]]; do
    case "$1" in
        --to) TARGET_HASH="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

ARTIFACTS_DIR="deployment/artifacts/${NETWORK}"
DEPLOY_LOG=".deploy-${NETWORK}.log"

command -v soroban >/dev/null 2>&1 || {
    echo "soroban CLI not found."
    exit 1
}

if [ ! -f "$DEPLOY_LOG" ]; then
    echo "No deployment log found at $DEPLOY_LOG. Nothing to rollback."
    exit 1
fi

echo "==> Deployment history for $NETWORK:"
cat "$DEPLOY_LOG"

if [ -z "$TARGET_HASH" ]; then
    PREV_COUNT=$(wc -l < "$DEPLOY_LOG")
    if [ "$PREV_COUNT" -lt 2 ]; then
        echo "Only one deployment found. Nothing to rollback to."
        exit 1
    fi
    PREV_LINE=$(tail -2 "$DEPLOY_LOG" | head -1)
    TARGET_HASH=$(echo "$PREV_LINE" | awk -F' |' '{print $3}')
    echo "Rolling back to previous version (hash: $TARGET_HASH)"
fi

WASM_FILE=$(find "$ARTIFACTS_DIR" -name "*.wasm" 2>/dev/null | head -1)
if [ -z "$WASM_FILE" ]; then
    echo "No WASM artifact found in $ARTIFACTS_DIR."
    echo "Rebuilding from source at matching commit..."
    cargo build --target wasm32-unknown-unknown --release
    WASM_FILE="target/wasm32-unknown-unknown/release/stellar_nebula_nomad.wasm"
fi

echo "==> Deploying rollback WASM: $WASM_FILE"
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_FILE" \
    --source-account "$IDENTITY" \
    --network "$NETWORK" 2>/dev/null)

echo "Rollback contract deployed: $CONTRACT_ID"

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "$TIMESTAMP | $CONTRACT_ID | ROLLBACK | $IDENTITY" >> "$DEPLOY_LOG"

echo ""
echo "Rollback complete."
echo "  New contract ID: $CONTRACT_ID"
echo "  Previous target hash: $TARGET_HASH"
echo ""
echo "Run verification:"
echo "  ./scripts/verify.sh $NETWORK $CONTRACT_ID $IDENTITY"
