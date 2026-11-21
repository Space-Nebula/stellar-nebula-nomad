#!/usr/bin/env bash
# Standalone backup integrity verifier for Stellar Nebula Nomad.
#
# Verifies a .tar.gz backup archive without contacting the network:
#   * gzip integrity + tar listing
#   * extraction to a local staging directory
#   * SHA256 checksum verification against verification/checksums.txt
#   * presence and JSON validity of required metadata/state files
#   * optional dry-run restoration (staging only, never deploys)
#
# Always writes a Markdown report, a JSON report and a Prometheus status file
# into --report-dir (default: the directory containing the archive).
#
# Exit status: 0 = all checks passed, 1 = one or more checks failed,
#              2 = bad invocation/usage.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ---------------------------------------------------------------------------
# Arguments / globals
# ---------------------------------------------------------------------------
ARCHIVE=""
REPORT_DIR=""
DRY_RUN=false
JSON_OUTPUT=false
QUIET=false

TIMESTAMP=""
NOW_EPOCH="0"
START_SECONDS="${SECONDS:-0}"
ARCHIVE_ABS=""
ARCHIVE_SIZE="0"
ARCHIVE_SIZE_HUMAN="unknown"
ARCHIVE_SHA256=""
FILE_COUNT="0"
ROOT_DIR=""
CHECKSUM_FILE=""
TEMP_DIR=""
OVERALL_FAIL=false
CHECKSUMS_OK=false
CHECKSUMS_VALID=false
REQUIRED_FILES_OK=true
DRY_RUN_MARKER=""

declare -a CHECK_NAMES=()
declare -a CHECK_STATUS=()
declare -a CHECK_DETAILS=()
declare -a ERRORS=()
declare -a WARNINGS=()

usage() {
    cat <<EOF
Usage: $0 [OPTIONS] [ARCHIVE]

Standalone verification of a Stellar Nebula Nomad backup archive.
No network access is performed and nothing is ever deployed.

OPTIONS:
    -b, --backup FILE        Backup archive to verify (.tar.gz)
    -r, --report-dir DIR     Where to write reports (default: archive directory)
    -d, --dry-run            Perform a staging-only restoration dry run
    -j, --json               Print the JSON report to stdout (suppresses progress)
    -q, --quiet              Suppress informational progress lines
    -h, --help               Show this help message

OUTPUT FILES (in --report-dir):
    backup-validation-report.md   Human-readable validation report
    backup-validation.json        Machine-readable validation report
    backup_status.prom            Prometheus-style backup status metrics
    restore-dry-run.marker        Written only with --dry-run and a passing run

EXAMPLES:
    $0 --backup backups/nebula_backup_20260428_120000.tar.gz
    $0 backups/nebula_backup_20260428_120000.tar.gz --dry-run --report-dir reports/
    $0 --backup backup.tar.gz --json --quiet

EXIT CODES:
    0  all checks passed
    1  one or more checks failed
    2  invalid usage
EOF
}

warn_line() {
    printf '%s\n' "$*" >&2
}

info() {
    if [ "$JSON_OUTPUT" = false ] && [ "$QUIET" = false ]; then
        printf '%s\n' "$*"
    fi
}

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            -b|--backup)
                ARCHIVE="${2:-}"
                if [ -z "$ARCHIVE" ]; then warn_line "ERROR: --backup requires a path"; exit 2; fi
                shift 2
                ;;
            --backup=*)
                ARCHIVE="${1#*=}"
                shift
                ;;
            -r|--report-dir)
                REPORT_DIR="${2:-}"
                if [ -z "$REPORT_DIR" ]; then warn_line "ERROR: --report-dir requires a path"; exit 2; fi
                shift 2
                ;;
            --report-dir=*)
                REPORT_DIR="${1#*=}"
                shift
                ;;
            -d|--dry-run)
                DRY_RUN=true
                shift
                ;;
            -j|--json)
                JSON_OUTPUT=true
                QUIET=true
                shift
                ;;
            -q|--quiet)
                QUIET=true
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            --)
                shift
                if [ $# -gt 0 ] && [ -z "$ARCHIVE" ]; then ARCHIVE="$1"; shift; fi
                ;;
            -*)
                warn_line "ERROR: unknown option: $1"
                usage >&2
                exit 2
                ;;
            *)
                if [ -z "$ARCHIVE" ]; then
                    ARCHIVE="$1"
                else
                    warn_line "WARNING: ignoring unexpected positional argument: $1"
                fi
                shift
                ;;
        esac
    done
}

# ---------------------------------------------------------------------------
# Check bookkeeping
# ---------------------------------------------------------------------------
record_check() {
    local name="$1" status="$2" detail="$3"
    detail="$(printf '%.400s' "$detail")"
    CHECK_NAMES+=("$name")
    CHECK_STATUS+=("$status")
    CHECK_DETAILS+=("$detail")
    if [ "$status" = "fail" ]; then
        OVERALL_FAIL=true
        ERRORS+=("$name: $detail")
        if [ "$JSON_OUTPUT" = false ]; then
            printf 'FAIL: %s - %s\n' "$name" "$detail" >&2
        fi
    elif [ "$JSON_OUTPUT" = false ] && [ "$QUIET" = false ]; then
        printf 'PASS: %s - %s\n' "$name" "$detail"
    fi
}

add_warning() {
    WARNINGS+=("$1")
    if [ "$JSON_OUTPUT" = false ]; then
        printf 'WARN: %s\n' "$1" >&2
    fi
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
json_escape() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    s="${s//$'\n'/\\n}"
    s="${s//$'\r'/\\r}"
    s="${s//$'\t'/\\t}"
    printf '%s' "$s"
}

prom_escape() {
    local s="$1"
    s="${s//\\/\\\\}"
    s="${s//\"/\\\"}"
    s="${s//$'\n'/\\n}"
    printf '%s' "$s"
}

json_string_array() {
    local first=true item
    for item in "$@"; do
        if [ "$first" = true ]; then
            first=false
        else
            printf ','
        fi
        printf '"%s"' "$(json_escape "$item")"
    done
}

duration_seconds() {
    local d=$((SECONDS - START_SECONDS))
    if [ "$d" -lt 0 ]; then d=0; fi
    printf '%s' "$d"
}

# Locate the backup root inside the extraction/staging directory. Supports
# both layouts: an archive with a single top-level backup directory (produced
# by scripts/backup.sh) and a flat archive with files at the archive root.
find_backup_root() {
    local base="$1" checksum root
    checksum="$(find "$base" -type f -path '*/verification/checksums.txt' -print -quit 2>/dev/null || true)"
    if [ -n "$checksum" ]; then
        root="$(dirname "$(dirname "$checksum")")"
        (cd "$root" && pwd)
        return 0
    fi
    if [ -f "$base/metadata/contract_info.json" ]; then
        (cd "$base" && pwd)
        return 0
    fi
    root="$(find "$base" -mindepth 1 -maxdepth 1 -type d -print -quit 2>/dev/null || true)"
    if [ -n "$root" ] && [ -f "$root/metadata/contract_info.json" ]; then
        (cd "$root" && pwd)
        return 0
    fi
    return 1
}

# Returns 0 = valid JSON, 1 = invalid JSON, 125 = no validator available.
validate_json_file() {
    local file="$1"
    if command -v python3 >/dev/null 2>&1; then
        python3 -c 'import json,sys; json.load(open(sys.argv[1], "r"))' "$file" >/dev/null 2>&1
    elif command -v jq >/dev/null 2>&1; then
        jq empty "$file" >/dev/null 2>&1
    else
        return 125
    fi
}

status_word() {
    if [ "$OVERALL_FAIL" = true ]; then printf 'fail'; else printf 'pass'; fi
}

write_json() {
    local i status rc count_pass=0 count_fail=0 comma
    status="$(status_word)"
    for i in "${!CHECK_STATUS[@]}"; do
        if [ "${CHECK_STATUS[$i]}" = "pass" ]; then
            count_pass=$((count_pass + 1))
        else
            count_fail=$((count_fail + 1))
        fi
    done

    printf '{\n'
    printf '  "status": "%s",\n' "$status"
    printf '  "timestamp": "%s",\n' "$(json_escape "$TIMESTAMP")"
    printf '  "archive": {\n'
    printf '    "path": "%s",\n' "$(json_escape "$ARCHIVE")"
    printf '    "absolute_path": "%s",\n' "$(json_escape "$ARCHIVE_ABS")"
    printf '    "size_bytes": %s,\n' "$ARCHIVE_SIZE"
    printf '    "size_human": "%s",\n' "$(json_escape "$ARCHIVE_SIZE_HUMAN")"
    printf '    "sha256": "%s"\n' "$(json_escape "$ARCHIVE_SHA256")"
    printf '  },\n'
    if [ "$DRY_RUN" = true ]; then printf '  "dry_run": true,\n'; else printf '  "dry_run": false,\n'; fi
    printf '  "duration_seconds": %s,\n' "$(duration_seconds)"
    printf '  "checks_passed": %s,\n' "$count_pass"
    printf '  "checks_failed": %s,\n' "$count_fail"
    printf '  "checks": [\n'
    for i in "${!CHECK_NAMES[@]}"; do
        if [ "$i" -gt 0 ]; then printf ','; fi
        printf '    {"name": "%s", "status": "%s", "details": "%s"}' \
            "$(json_escape "${CHECK_NAMES[$i]}")" \
            "$(json_escape "${CHECK_STATUS[$i]}")" \
            "$(json_escape "${CHECK_DETAILS[$i]}")"
        printf '\n'
    done
    printf '  ],\n'
    printf '  "errors": [%s],\n' "$(json_string_array ${ERRORS[@]+"${ERRORS[@]}"})"
    printf '  "warnings": [%s]\n' "$(json_string_array ${WARNINGS[@]+"${WARNINGS[@]}"})"
    printf '}\n'
}

write_markdown() {
    local i status
    status="$(status_word)"
    {
        printf '# Backup Validation Report\n\n'
        printf -- '- **Status:** %s\n' "$(printf '%s' "$status" | tr '[:lower:]' '[:upper:]')"
        printf -- '- **Timestamp (UTC):** %s\n' "$TIMESTAMP"
        printf -- '- **Archive:** `%s`\n' "$ARCHIVE"
        printf -- '- **Archive size:** %s bytes (%s)\n' "$ARCHIVE_SIZE" "$ARCHIVE_SIZE_HUMAN"
        printf -- '- **Archive SHA256:** `%s`\n' "$ARCHIVE_SHA256"
        if [ "$DRY_RUN" = true ]; then
            printf -- '- **Mode:** verification + restoration dry run (staging only, no network/deploy)\n\n'
        else
            printf -- '- **Mode:** verification only\n\n'
        fi

        printf '## Checks\n\n'
        printf '| Check | Status | Details |\n'
        printf '| --- | --- | --- |\n'
        for i in "${!CHECK_NAMES[@]}"; do
            local detail="${CHECK_DETAILS[$i]}"
            detail="${detail//|/\\|}"
            printf '| %s | %s | %s |\n' "${CHECK_NAMES[$i]}" "${CHECK_STATUS[$i]}" "$detail"
        done
        printf '\n'

        printf '## Errors\n\n'
        if [ "${#ERRORS[@]}" -eq 0 ]; then
            printf '_None._\n\n'
        else
            for i in "${!ERRORS[@]}"; do printf -- '- %s\n' "${ERRORS[$i]}"; done
            printf '\n'
        fi

        printf '## Warnings\n\n'
        if [ "${#WARNINGS[@]}" -eq 0 ]; then
            printf '_None._\n\n'
        else
            for i in "${!WARNINGS[@]}"; do printf -- '- %s\n' "${WARNINGS[$i]}"; done
            printf '\n'
        fi

        printf '## Prometheus status\n\n'
        printf 'A Prometheus-compatible status file is written alongside this report as `backup_status.prom`.\n'
        if [ -n "$DRY_RUN_MARKER" ]; then
            printf '\nDry-run marker: `%s`\n' "$DRY_RUN_MARKER"
        fi
    }
}

write_prom() {
    local status label last_success
    if [ "$OVERALL_FAIL" = true ]; then
        status=0
        last_success=0
    else
        status=1
        last_success="$NOW_EPOCH"
    fi
    label="archive=\"$(prom_escape "$(basename "$ARCHIVE")")\""

    printf '# HELP backup_success Whether the most recent backup verification succeeded.\n'
    printf '# TYPE backup_success gauge\n'
    printf 'backup_success{%s} %s\n' "$label" "$status"
    printf '# HELP backup_last_success_timestamp Unix timestamp of the last successful backup verification (0 on failure).\n'
    printf '# TYPE backup_last_success_timestamp gauge\n'
    printf 'backup_last_success_timestamp %s\n' "$last_success"
    printf '# HELP backup_duration_seconds Duration of the verification run in seconds.\n'
    printf '# TYPE backup_duration_seconds gauge\n'
    printf 'backup_duration_seconds %s\n' "$(duration_seconds)"
    printf '# HELP backup_size_bytes Size of the verified archive in bytes.\n'
    printf '# TYPE backup_size_bytes gauge\n'
    printf 'backup_size_bytes %s\n' "$ARCHIVE_SIZE"
    printf '# HELP backup_verified_timestamp Unix timestamp of this verification run.\n'
    printf '# TYPE backup_verified_timestamp gauge\n'
    printf 'backup_verified_timestamp %s\n' "$NOW_EPOCH"
}

write_reports() {
    if ! mkdir -p "$REPORT_DIR" 2>/dev/null; then
        warn_line "WARNING: could not create report directory: $REPORT_DIR"
        return 0
    fi
    if [ "$DRY_RUN" = true ] && [ "$OVERALL_FAIL" = false ]; then
        DRY_RUN_MARKER="$REPORT_DIR/restore-dry-run.marker"
        if ! printf 'dry_run=pass\narchive=%s\nsha256=%s\ntimestamp=%s\n' \
            "$ARCHIVE" "$ARCHIVE_SHA256" "$TIMESTAMP" > "$DRY_RUN_MARKER" 2>/dev/null; then
            warn_line "WARNING: failed to write dry-run marker"
            DRY_RUN_MARKER=""
        fi
    fi
    if ! write_markdown > "$REPORT_DIR/backup-validation-report.md" 2>/dev/null; then
        warn_line "WARNING: failed to write Markdown validation report"
    fi
    if ! write_json > "$REPORT_DIR/backup-validation.json" 2>/dev/null; then
        warn_line "WARNING: failed to write JSON validation report"
    fi
    if ! write_prom > "$REPORT_DIR/backup_status.prom" 2>/dev/null; then
        warn_line "WARNING: failed to write Prometheus status file"
    fi
}

cleanup() {
    if [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT

finish() {
    write_reports
    if [ "$JSON_OUTPUT" = true ]; then
        write_json
    fi
    if [ "$OVERALL_FAIL" = true ]; then
        printf 'FAIL: backup verification failed for %s\n' "$ARCHIVE" >&2
        exit 1
    fi
    if [ "$QUIET" = false ]; then
        printf 'PASS: backup verification succeeded for %s\n' "$ARCHIVE" >&2
    fi
    exit 0
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    parse_args "$@"

    if [ -z "$ARCHIVE" ]; then
        usage >&2
        exit 2
    fi

    TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    NOW_EPOCH="$(date -u +%s)"

    if [ -z "$REPORT_DIR" ]; then
        REPORT_DIR="$(cd "$(dirname "$ARCHIVE")" 2>/dev/null && pwd)"
    fi

    TEMP_DIR="$(mktemp -d)"
    mkdir -p "$TEMP_DIR/staging"

    info "=== Backup verification: $ARCHIVE ==="
    info "Report directory: $REPORT_DIR"

    if [ ! -f "$ARCHIVE" ]; then
        record_check "archive_exists" "fail" "Archive file not found: $ARCHIVE"
        finish
    fi
    if [ ! -r "$ARCHIVE" ]; then
        record_check "archive_readable" "fail" "Archive is not readable: $ARCHIVE"
        finish
    fi

    ARCHIVE_SIZE="$(stat -c %s "$ARCHIVE" 2>/dev/null || wc -c < "$ARCHIVE" | tr -d '[:space:]')"
    if [ -z "$ARCHIVE_SIZE" ]; then ARCHIVE_SIZE=0; fi
    ARCHIVE_SIZE_HUMAN="$(du -h "$ARCHIVE" 2>/dev/null | cut -f1 || printf 'unknown')"
    ARCHIVE_ABS="$(cd "$(dirname "$ARCHIVE")" && pwd)/$(basename "$ARCHIVE")"
    ARCHIVE_SHA256="$(sha256sum "$ARCHIVE" 2>/dev/null | awk '{print $1}' || true)"

    record_check "archive_exists" "pass" "Archive found ($ARCHIVE_SIZE_HUMAN, $ARCHIVE_SIZE bytes)"

    case "$(basename "$ARCHIVE")" in
        *.tar.gz|*.tgz)
            record_check "archive_format" "pass" "Archive has a supported .tar.gz extension"
            ;;
        *)
            record_check "archive_format" "fail" "Archive filename must end in .tar.gz or .tgz"
            finish
            ;;
    esac

    if [ "$ARCHIVE_SIZE" -gt 0 ]; then
        record_check "archive_nonempty" "pass" "Archive is not empty"
    else
        record_check "archive_nonempty" "fail" "Archive is empty (0 bytes)"
        finish
    fi

    if gzip -t "$ARCHIVE" 2> "$TEMP_DIR/gzip.err"; then
        record_check "gzip_integrity" "pass" "gzip -t passed"
    else
        record_check "gzip_integrity" "fail" "gzip -t failed: $(head -n 2 "$TEMP_DIR/gzip.err" | tr '\n' ' ')"
        finish
    fi

    if tar -tzf "$ARCHIVE" > "$TEMP_DIR/listing.txt" 2> "$TEMP_DIR/tar.err"; then
        FILE_COUNT="$(wc -l < "$TEMP_DIR/listing.txt" | tr -d '[:space:]')"
        if [ -z "$FILE_COUNT" ]; then FILE_COUNT=0; fi
        record_check "tar_listing" "pass" "tar -tzf listed $FILE_COUNT entries"
    else
        record_check "tar_listing" "fail" "tar -tzf failed: $(head -n 2 "$TEMP_DIR/tar.err" | tr '\n' ' ')"
        finish
    fi

    if [ "$FILE_COUNT" -ge 3 ] && [ "$FILE_COUNT" -le 100000 ]; then
        record_check "sane_file_count" "pass" "Entry count $FILE_COUNT is within sane bounds (3..100000)"
    else
        record_check "sane_file_count" "fail" "Unexpected entry count: $FILE_COUNT (expected 3..100000)"
    fi

    if grep -Eq '(^|/)\.\.(/|$)|^/' "$TEMP_DIR/listing.txt"; then
        record_check "path_safety" "fail" "Archive contains absolute paths or '..' traversal entries"
    else
        record_check "path_safety" "pass" "No absolute paths or '..' traversal entries found"
    fi

    if tar -xzf "$ARCHIVE" -C "$TEMP_DIR/staging" 2> "$TEMP_DIR/extract.err"; then
        record_check "extraction" "pass" "Archive extracted to a local staging directory"
    else
        record_check "extraction" "fail" "Failed to extract archive: $(head -n 2 "$TEMP_DIR/extract.err" | tr '\n' ' ')"
        finish
    fi

    if ROOT_DIR="$(find_backup_root "$TEMP_DIR/staging")"; then
        record_check "backup_structure" "pass" "Backup root located with expected layout"
    else
        record_check "backup_structure" "fail" "Could not locate metadata/ and verification/ in the extracted archive"
        finish
    fi

    CHECKSUM_FILE="$ROOT_DIR/verification/checksums.txt"
    if [ -f "$CHECKSUM_FILE" ] && [ -s "$CHECKSUM_FILE" ]; then
        CHECKSUMS_OK=true
        record_check "checksums_present" "pass" "verification/checksums.txt exists and is non-empty"
    else
        record_check "checksums_present" "fail" "verification/checksums.txt is missing or empty"
    fi

    if [ "$CHECKSUMS_OK" = true ]; then
        if (cd "$ROOT_DIR" && sha256sum -c verification/checksums.txt) > "$TEMP_DIR/sha.out" 2>&1; then
            CHECKSUMS_VALID=true
            record_check "checksums_valid" "pass" "All SHA256 checksums matched"
        else
            local failed_lines
            failed_lines="$(grep -c 'FAILED' "$TEMP_DIR/sha.out" || true)"
            record_check "checksums_valid" "fail" \
                "${failed_lines} checksum failure(s): $(head -n 2 "$TEMP_DIR/sha.out" | tr '\n' ' ')"
        fi
    else
        record_check "checksums_valid" "fail" "Not run: checksums file missing"
    fi

    local required rel
    for rel in metadata/contract_info.json state/contract_state.json; do
        if [ -f "$ROOT_DIR/$rel" ] && [ -s "$ROOT_DIR/$rel" ]; then
            record_check "file_present:$rel" "pass" "Required file exists and is non-empty"
        else
            REQUIRED_FILES_OK=false
            record_check "file_present:$rel" "fail" "Required file is missing or empty: $rel"
        fi
    done

    if command -v python3 >/dev/null 2>&1; then
        info "JSON validator: python3"
    elif command -v jq >/dev/null 2>&1; then
        info "JSON validator: jq"
    else
        add_warning "Neither python3 nor jq is available; JSON validity checks were skipped"
    fi

    for required in metadata/contract_info.json state/contract_state.json; do
        if [ ! -f "$ROOT_DIR/$required" ]; then
            continue
        fi
        validate_json_file "$ROOT_DIR/$required" && rc=0 || rc=$?
        if [ "$rc" -eq 125 ]; then
            info "Skipped JSON validation for $required (no validator available)"
        elif [ "$rc" -eq 0 ]; then
            record_check "json_valid:$required" "pass" "Valid JSON"
        else
            record_check "json_valid:$required" "fail" "Invalid JSON: $required"
        fi
    done

    if [ "$DRY_RUN" = true ]; then
        info "Performing restoration dry run (staging only; no network calls, no deployments)"
        if [ "$CHECKSUMS_VALID" = true ] && [ "$REQUIRED_FILES_OK" = true ]; then
            record_check "dry_run_restore" "pass" \
                "Staging restore verified structure and checksums; no network, no deploy"
        else
            record_check "dry_run_restore" "fail" \
                "Staging restore could not be completed because integrity/file checks failed"
        fi
    else
        info "Verification-only mode (no restoration dry run)"
    fi

    finish
}

main "$@"
