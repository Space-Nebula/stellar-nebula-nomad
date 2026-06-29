#!/usr/bin/env bash
set -euo pipefail

# Deployment automation for Nebula Nomad contract to Soroban.
# Features: build, optimize, deploy, verify, alias, rollback support.
# Usage:
#   ./scripts/deploy.sh [network] [identity] [--with-verify] [--alias NAME]
# Examples:
#   ./scripts/deploy.sh futurenet default --with-verify
#   ./scripts/deploy.sh testnet admin --alias nebula-v1

NETWORK="${1:-futurenet}"
IDENTITY="${2:-default}"
DO_VERIFY=false
ALIAS=""
shift 2 || true

while [[ $# -gt 0 ]]; do
    case "$1" in
        --with-verify) DO_VERIFY=true; shift ;;
        --alias) ALIAS="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

WASM_PATH="target/wasm32-unknown-unknown/release/stellar_nebula_nomad.wasm"
DEPLOY_LOG=".deploy-${NETWORK}.log"
ARTIFACTS_DIR="deployment/artifacts/${NETWORK}"

command -v soroban >/dev/null 2>&1 || {
    echo "soroban CLI not found. Install with: cargo install soroban-cli --locked"
    exit 1
}

mkdir -p "$ARTIFACTS_DIR"

echo "==> Building WASM for $NETWORK (identity: $IDENTITY)"
cargo build --target wasm32-unknown-unknown --release

echo "==> Optimizing WASM"
soroban contract optimize --wasm "$WASM_PATH"

HASH=$(sha256sum "$WASM_PATH" | awk '{print $1}')
echo "WASM SHA256: $HASH"

echo "==> Deploying to network: $NETWORK"
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_PATH" \
    --source-account "$IDENTITY" \
    --network "$NETWORK" \
    --output json 2>/dev/null | jq -r '.contract_id // .')

if [ -z "$CONTRACT_ID" ]; then
    CONTRACT_ID=$(soroban contract deploy \
        --wasm "$WASM_PATH" \
        --source-account "$IDENTITY" \
        --network "$NETWORK" 2>/dev/null)
fi

echo "Contract deployed: $CONTRACT_ID"

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "$TIMESTAMP | $CONTRACT_ID | $HASH | $IDENTITY" >> "$DEPLOY_LOG"

echo "$CONTRACT_ID" > "${ARTIFACTS_DIR}/latest-id.txt"
echo "$HASH" > "${ARTIFACTS_DIR}/latest-hash.txt"
cp "$WASM_PATH" "${ARTIFACTS_DIR}/stellar_nebula_nomad-${CONTRACT_ID:0:8}.wasm"

if [ -n "$ALIAS" ]; then
    mkdir -p "deployment/aliases"
    echo "$CONTRACT_ID" > "deployment/aliases/${ALIAS}.txt"
    echo "Aliased as: $ALIAS -> $CONTRACT_ID"
fi

if [ "$DO_VERIFY" = true ]; then
    echo "==> Running post-deploy verification..."
    if bash scripts/verify.sh "$NETWORK" "$CONTRACT_ID" "$IDENTITY"; then
        echo "Verification passed."
    else
        echo "WARNING: Verification failed. Check contract state manually." >&2
    fi
fi

echo ""
echo "Deployment complete!"
echo "  Network:    $NETWORK"
echo "  Contract:   $CONTRACT_ID"
echo "  WASM hash:  $HASH"
echo ""
echo "Post-deploy smoke test (manual):"
echo "  soroban contract invoke --id $CONTRACT_ID --source-account $IDENTITY --network $NETWORK --fn get_contract_version"
echo ""
echo "To rollback: ./scripts/rollback.sh $NETWORK $IDENTITY"
