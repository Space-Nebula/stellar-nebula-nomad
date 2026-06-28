#!/usr/bin/env bash
# ─── pin_metadata.sh — Batch IPFS Metadata Pinning Utility ───────────────────
#
# Automates batched metadata serialization, upload to IPFS via a pinning
# service API (Pinata-compatible), and pin status validation.
#
# Usage:
#   ./scripts/pin_metadata.sh pin <cid_or_file>          Pin a single CID or file
#   ./scripts/pin_metadata.sh batch <directory>          Pin all JSON files in a directory
#   ./scripts/pin_metadata.sh status <cid>               Check pin status for a CID
#   ./scripts/pin_metadata.sh verify <cid> <expected>    Verify pinned content matches expected
#
# Environment Variables:
#   PINATA_API_KEY      Pinata API key (required for pin/status commands)
#   PINATA_SECRET_KEY   Pinata secret key (required for pin/status commands)
#   IPFS_NODE_URL       Custom IPFS node URL (default: https://api.pinata.cloud)
#   PIN_TIMEOUT         Timeout in seconds for pin status polling (default: 120)
#   PIN_INTERVAL        Poll interval in seconds (default: 10)
#
# Exit Codes:
#   0 — Success
#   1 — Usage error / missing arguments
#   2 — API authentication failure
#   3 — Pin operation failed
#   4 — Pin status check failed / timeout

set -euo pipefail

# ─── Configuration ──────────────────────────────────────────────────────────

PINATA_API_KEY="${PINATA_API_KEY:-}"
PINATA_SECRET_KEY="${PINATA_SECRET_KEY:-}"
IPFS_NODE_URL="${IPFS_NODE_URL:-https://api.pinata.cloud}"
PIN_TIMEOUT="${PIN_TIMEOUT:-120}"
PIN_INTERVAL="${PIN_INTERVAL:-10}"
GATEWAY_URL="${GATEWAY_URL:-https://ipfs.io/ipfs}"

# ─── Helpers ────────────────────────────────────────────────────────────────

die() {
  echo "ERROR: $*" >&2
  exit "${2:-1}"
}

info() {
  echo "INFO:  $*"
}

warn() {
  echo "WARN:  $*" >&2
}

require_auth() {
  if [[ -z "$PINATA_API_KEY" || -z "$PINATA_SECRET_KEY" ]]; then
    die "PINATA_API_KEY and PINATA_SECRET_KEY must be set" 2
  fi
}

# Validate that a CID is non-empty and looks like an IPFS hash.
validate_cid() {
  local cid="$1"
  if [[ -z "$cid" ]]; then
    die "CID cannot be empty" 1
  fi
  # Support both Qm... (SHA-256) and bafy... (CIDv1) formats.
  if [[ ! "$cid" =~ ^(Qm[1-9A-HJ-NP-Za-km-z]{44}|bafy[0-9A-Za-z]{50,})$ ]]; then
    warn "CID '$cid' does not match standard IPFS format — proceeding anyway"
  fi
}

# ─── Commands ───────────────────────────────────────────────────────────────

# Pin a single CID or local file to the IPFS pinning service.
cmd_pin() {
  local target="${1:-}"
  [[ -n "$target" ]] || die "Usage: $0 pin <cid_or_file>" 1
  require_auth

  if [[ -f "$target" ]]; then
    info "Pinning local file: $target"
    local cid
    cid=$(curl -s -X POST "${IPFS_NODE_URL}/pinning/pinByHash" \
      -H "pinata_api_key: ${PINATA_API_KEY}" \
      -H "pinata_secret_api_key: ${PINATA_SECRET_KEY}" \
      -H "Content-Type: application/json" \
      -d "{\"hash\": \"$(ipfs add -q "$target" | tail -n1)\"}" | \
      jq -r '.ipfs_hash // .cid // empty')

    if [[ -z "$cid" ]]; then
      die "Failed to extract CID from pin response" 3
    fi
    info "File pinned with CID: $cid"
  else
    # Target is already a CID — request a persistent pin.
    validate_cid "$target"
    info "Requesting persistent pin for CID: $target"

    local response
    response=$(curl -s -w "\n%{http_code}" -X POST \
      "${IPFS_NODE_URL}/pinning/pins" \
      -H "pinata_api_key: ${PINATA_API_KEY}" \
      -H "pinata_secret_api_key: ${PINATA_SECRET_KEY}" \
      -H "Content-Type: application/json" \
      -d "{
        \"cid\": \"${target}\",
        \"pinataMetadata\": {
          \"name\": \"stellar-nebula-nomad-metadata\"
        },
        \"pinataOptions\": {
          \"replicationFactor\": 3
        }
      }")

    local http_code
    http_code=$(echo "$response" | tail -n1)
    local body
    body=$(echo "$response" | sed '$d')

    if [[ "$http_code" -lt 200 || "$http_code" -ge 300 ]]; then
      die "Pin request failed (HTTP $http_code): $body" 3
    fi

    info "Pin request queued successfully"
    echo "$body" | jq -r '.cid // .ipfs_hash // "unknown"' 2>/dev/null || true
  fi
}

# Batch pin all JSON metadata files in a directory.
cmd_batch() {
  local dir="${1:-}"
  [[ -n "$dir" && -d "$dir" ]] || die "Usage: $0 batch <directory>" 1
  require_auth

  local count=0
  local success=0
  local failed=0

  info "Scanning directory: $dir"

  for file in "$dir"/*.json; do
    [[ -f "$file" ]] || continue
    count=$((count + 1))

    local basename
    basename=$(basename "$file")
    info "Pinning [$count]: $basename"

    if cmd_pin "$file" >/dev/null 2>&1; then
      success=$((success + 1))
    else
      failed=$((failed + 1))
      warn "Failed to pin: $basename"
    fi
  done

  info "Batch complete: $count total, $success pinned, $failed failed"
  [[ "$failed" -eq 0 ]] || exit 3
}

# Check the pin status for a given CID.
cmd_status() {
  local cid="${1:-}"
  [[ -n "$cid" ]] || die "Usage: $0 status <cid>" 1
  require_auth
  validate_cid "$cid"

  info "Checking pin status for CID: $cid"

  local response
  response=$(curl -s -w "\n%{http_code}" -X GET \
    "${IPFS_NODE_URL}/pinning/pins/${cid}" \
    -H "pinata_api_key: ${PINATA_API_KEY}" \
    -H "pinata_secret_api_key: ${PINATA_SECRET_KEY}")

  local http_code
  http_code=$(echo "$response" | tail -n1)
  local body
  body=$(echo "$response" | sed '$d')

  if [[ "$http_code" -ge 200 && "$http_code" -lt 300 ]]; then
    local status
    status=$(echo "$body" | jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")
    info "Pin status: $status"
    echo "$body" | jq '.' 2>/dev/null || echo "$body"
    return 0
  else
    die "Status check failed (HTTP $http_code): $body" 4
  fi
}

# Verify that a pinned CID is accessible and matches expected content.
cmd_verify() {
  local cid="${1:-}"
  local expected="${2:-}"
  [[ -n "$cid" ]] || die "Usage: $0 verify <cid> [expected_content_hash]" 1

  validate_cid "$cid"
  info "Verifying CID accessibility: $cid"

  # Attempt to fetch the content via the gateway.
  local http_code
  http_code=$(curl -s -o /dev/null -w "%{http_code}" \
    --max-time 30 \
    "${GATEWAY_URL}/${cid}")

  if [[ "$http_code" -eq 200 ]]; then
    info "CID $cid is accessible via gateway"
    if [[ -n "$expected" ]]; then
      local actual
      actual=$(curl -s "${GATEWAY_URL}/${cid}" | sha256sum | awk '{print $1}')
      if [[ "$actual" == "$expected" ]]; then
        info "Content hash matches expected: $expected"
      else
        die "Content hash mismatch: expected $expected, got $actual" 4
      fi
    fi
  else
    die "CID $cid not accessible (HTTP $http_code)" 4
  fi
}

# Poll until a CID reaches "pinned" status or timeout.
cmd_wait_for_pin() {
  local cid="${1:-}"
  [[ -n "$cid" ]] || die "Usage: $0 wait <cid>" 1
  require_auth

  local elapsed=0
  info "Waiting for CID $cid to reach 'pinned' status (timeout: ${PIN_TIMEOUT}s)..."

  while [[ "$elapsed" -lt "$PIN_TIMEOUT" ]]; do
    local status
    status=$(curl -s "${IPFS_NODE_URL}/pinning/pins/${cid}" \
      -H "pinata_api_key: ${PINATA_API_KEY}" \
      -H "pinata_secret_api_key: ${PINATA_SECRET_KEY}" | \
      jq -r '.status // "unknown"' 2>/dev/null || echo "unknown")

    case "$status" in
      pinned)
        info "CID $cid is now pinned after ${elapsed}s"
        return 0
        ;;
      failed)
        die "Pin failed for CID $cid" 3
        ;;
      *)
        info "Status: $status (elapsed: ${elapsed}s)"
        ;;
    esac

    sleep "$PIN_INTERVAL"
    elapsed=$((elapsed + PIN_INTERVAL))
  done

  die "Pin timeout for CID $cid after ${PIN_TIMEOUT}s" 4
}

# ─── Main ───────────────────────────────────────────────────────────────────

show_help() {
  cat <<'EOF'
Usage: pin_metadata.sh <command> [arguments]

Commands:
  pin <cid_or_file>          Pin a single CID or local file to IPFS
  batch <directory>          Batch pin all JSON files in a directory
  status <cid>               Check pin status for a CID
  verify <cid> [hash]        Verify CID is accessible and optionally check content hash
  wait <cid>                 Poll until CID reaches 'pinned' status or timeout

Environment:
  PINATA_API_KEY             Pinata API key (required)
  PINATA_SECRET_KEY          Pinata secret key (required)
  IPFS_NODE_URL              Pinning service URL (default: https://api.pinata.cloud)
  GATEWAY_URL                IPFS gateway for verification (default: https://ipfs.io/ipfs)
  PIN_TIMEOUT                Timeout in seconds for wait command (default: 120)
  PIN_INTERVAL               Poll interval in seconds (default: 10)
EOF
}

case "${1:-}" in
  pin)       shift; cmd_pin "$@" ;;
  batch)     shift; cmd_batch "$@" ;;
  status)    shift; cmd_status "$@" ;;
  verify)    shift; cmd_verify "$@" ;;
  wait)      shift; cmd_wait_for_pin "$@" ;;
  -h|--help) show_help ;;
  *)         die "Unknown command: ${1:-}\n\n$(show_help)" 1 ;;
esac
