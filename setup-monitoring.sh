#!/usr/bin/env bash
# setup-monitoring.sh — Bootstrap the stellar-nebula-nomad monitoring stack
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MONITORING_DIR="$SCRIPT_DIR/monitoring"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()  { echo -e "${GREEN}[INFO]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# Check prerequisites
check_prerequisites() {
  log "Checking prerequisites..."
  for cmd in docker curl; do
    if ! command -v "$cmd" &>/dev/null; then
      err "'$cmd' is required but not installed."
      exit 1
    fi
  done

  if ! docker compose version &>/dev/null && ! docker-compose version &>/dev/null; then
    err "Docker Compose is required but not installed."
    exit 1
  fi
  log "Prerequisites OK."
}

# Create .env if it doesn't exist
setup_env() {
  local env_file="$MONITORING_DIR/.env"
  if [[ ! -f "$env_file" ]]; then
    warn ".env not found — creating from defaults."
    cat > "$env_file" <<EOF
GRAFANA_USER=admin
GRAFANA_PASSWORD=changeme_in_production
SLACK_WEBHOOK_URL=https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK
HORIZON_HOST=horizon:8000
EOF
    warn "Edit $env_file before deploying to production!"
  else
    log ".env already exists — skipping."
  fi
}

# Start the monitoring stack
start_stack() {
  log "Starting monitoring stack..."
  docker compose -f "$MONITORING_DIR/docker-compose.yml" up -d
  log "Stack started."
}

# Wait for Grafana to become healthy
wait_for_grafana() {
  log "Waiting for Grafana to be ready..."
  local retries=30
  while ! curl -sf http://localhost:3000/api/health &>/dev/null; do
    retries=$((retries - 1))
    if [[ $retries -eq 0 ]]; then
      err "Grafana did not become ready in time."
      exit 1
    fi
    sleep 2
  done
  log "Grafana is ready."
}

# Print access info
print_info() {
  echo ""
  log "Monitoring stack is running!"
  echo ""
  echo "  Grafana:      http://localhost:3000  (admin / see .env)"
  echo "  Prometheus:   http://localhost:9090"
  echo "  Alertmanager: http://localhost:9093"
  echo ""
  warn "Remember to set GRAFANA_PASSWORD and SLACK_WEBHOOK_URL in monitoring/.env"
}

main() {
  check_prerequisites
  setup_env
  start_stack
  wait_for_grafana
  print_info
}

main "$@"