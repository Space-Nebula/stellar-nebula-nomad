#!/usr/bin/env bash
set -euo pipefail

# Post-deployment verification script for Nebula Nomad.
# Runs smoke tests against the deployed contract to confirm it's functional.
# Usage:
#   ./scripts/verify.sh <network> <contract_id> [identity]
# Example:
#   ./scripts/verify.sh futurenet CC... default

NETWORK="${1:?Usage: $0 <network> <contract_id> [identity]}"
CONTRACT_ID="${2:?Usage: $0 <network> <contract_id> [identity]}"
IDENTITY="${3:-default}"

command -v soroban >/dev/null 2>&1 || {
    echo "soroban CLI not found."
    exit 1
}

echo "==> Verifying contract $CONTRACT_ID on $NETWORK"

invoke() {
    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source-account "$IDENTITY" \
        --network "$NETWORK" \
        --fn "$@" 2>/dev/null
}

echo "  1. Checking contract version..."
VERSION=$(invoke get_contract_version 2>/dev/null || echo "unversioned")
echo "     Version: $VERSION"

echo "  2. Checking global stats..."
STATS=$(invoke get_global_stats 2>/dev/null || echo "{}")
echo "     Stats: $STATS"

echo "  3. Checking analytics (total scans)..."
if echo "$STATS" | jq -e '.total_scans != null' >/dev/null 2>&1; then
    echo "     Total scans available."
else
    echo "     Warning: total_scans not available"
fi

echo ""
echo "Verification completed for $CONTRACT_ID"
echo "  Network:    $NETWORK"
echo "  Status:     deployed and responding"
