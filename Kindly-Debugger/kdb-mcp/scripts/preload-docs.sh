#!/usr/bin/env bash
# preload-docs.sh - Preload CLAUDE.md framework files into XPath cache
# SessionStart hook for Claude Code integration
# Invoked automatically on session initialization
#
# Usage:
#   ./scripts/preload-docs.sh                    # Auto-detect paths
#   CACHE_DIR=/tmp/cache ./scripts/preload-docs.sh
#   DOCS_ROOT=/custom/path ./scripts/preload-docs.sh
#
# Exit codes:
#   0 = Success (all documents preloaded)
#   1 = Error (one or more documents missing/failed)
#   2 = Configuration error (paths invalid)

set -euo pipefail

# Configuration (with defaults)
CACHE_DIR="${CACHE_DIR:-${HOME}/.cache/claude-doc-optimizer}"
DOCS_ROOT="${CLAUDE_DOCS_ROOT:-${HOME}}"
LOG_FILE="${CACHE_DIR}/preload.log"
PRELOAD_TIMEOUT="${PRELOAD_TIMEOUT:-30}"  # seconds

# Document list: Framework files to preload in order
declare -a FRAMEWORK_DOCS=(
    "xml/shared/shared-components.xml"      # T0-T11 tier definitions (foundation)
    "xml/frameworks/uce34.xml"               # Q1-Q34 systematic discovery
    "xml/frameworks/t28.xml"                 # Testing framework (4 tiers)
    "xml/frameworks/b32.xml"                 # Benchmarking methodology
    "xml/frameworks/assum.xml"               # Safety assumptions
    "xml/frameworks/i20.xml"                 # Integration validation
    "xml/frameworks/uce-d7.xml"              # Debugging framework
)

# Color output for better readability
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'  # No Color

# Logging function
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Color logging
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
    log "INFO" "$*"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $*"
    log "SUCCESS" "$*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
    log "WARN" "$*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
    log "ERROR" "$*"
}

# Validate paths
validate_paths() {
    log_info "Validating configuration paths..."

    if [[ ! -d "$CACHE_DIR" ]]; then
        log_info "Creating cache directory: $CACHE_DIR"
        mkdir -p "$CACHE_DIR" || {
            log_error "Failed to create cache directory: $CACHE_DIR"
            return 2
        }
    fi

    if [[ ! -d "$DOCS_ROOT" ]]; then
        log_error "Documentation root not found: $DOCS_ROOT"
        return 2
    fi

    log_success "Paths validated"
    return 0
}

# Check if document exists
doc_exists() {
    local doc_path="${DOCS_ROOT}/${1}"
    [[ -f "$doc_path" ]]
}

# Get document size (human-readable)
get_doc_size() {
    local doc_path="${DOCS_ROOT}/${1}"
    if [[ -f "$doc_path" ]]; then
        du -h "$doc_path" | cut -f1
    else
        echo "N/A"
    fi
}

# Preload single document
preload_document() {
    local doc_id="$1"
    local doc_path="${DOCS_ROOT}/${doc_id}"

    # Skip if document doesn't exist
    if ! doc_exists "$doc_id"; then
        log_warn "Document not found: $doc_id"
        return 1
    fi

    local doc_size=$(get_doc_size "$doc_id")
    log_info "Preloading: $doc_id ($doc_size)"

    # Create cache file path
    local cache_file="${CACHE_DIR}/$(basename "$doc_id").cache"

    # Check if already cached and recent
    if [[ -f "$cache_file" ]]; then
        local doc_mtime=$(stat -f%m "$doc_path" 2>/dev/null || stat -c%Y "$doc_path")
        local cache_mtime=$(stat -f%m "$cache_file" 2>/dev/null || stat -c%Y "$cache_file")

        if [[ $cache_mtime -ge $doc_mtime ]]; then
            log_info "  Cache is current, skipping parse"
            return 0
        fi
    fi

    # Validate XML schema (requires xmllint)
    if command -v xmllint &> /dev/null; then
        if ! timeout "$PRELOAD_TIMEOUT" xmllint --noout "$doc_path" 2>/dev/null; then
            log_error "  XML validation failed: $doc_id"
            return 1
        fi
        log_info "  XML validation passed"
    else
        log_warn "  xmllint not found, skipping validation"
    fi

    # Create cache entry (simple: timestamp + path)
    {
        echo "# Cache entry for: $doc_id"
        echo "# Generated: $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
        echo "# Source: $doc_path"
        echo "# Size: $doc_size"
        echo "# Expires: $(date -u -d '+24 hours' '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || echo 'N/A')"
    } > "$cache_file"

    log_success "  Preloaded: $doc_id"
    return 0
}

# Preload all documents
preload_all() {
    log_info "Starting document preload (timeout: ${PRELOAD_TIMEOUT}s)..."
    log_info "Framework documents to preload: ${#FRAMEWORK_DOCS[@]}"
    echo ""

    local success_count=0
    local fail_count=0

    for doc in "${FRAMEWORK_DOCS[@]}"; do
        if preload_document "$doc"; then
            ((success_count++)) || true
        else
            ((fail_count++)) || true
        fi
    done

    echo ""
    log_info "Preload complete:"
    log_success "  ✓ $success_count documents preloaded"

    if [[ $fail_count -gt 0 ]]; then
        log_warn "  ✗ $fail_count documents skipped (not found)"
    fi

    # Verify cache directory contents
    local cache_count=$(find "$CACHE_DIR" -name "*.cache" -type f | wc -l)
    log_info "Cache directory contains: $cache_count cache entries"

    # Summary
    echo ""
    echo "Cache Directory: $CACHE_DIR"
    echo "Documentation Root: $DOCS_ROOT"
    echo "Log File: $LOG_FILE"
    echo ""

    if [[ $fail_count -eq 0 ]]; then
        log_success "All framework documents preloaded successfully"
        return 0
    else
        log_warn "Some documents failed to preload"
        return 1
    fi
}

# Show usage
usage() {
    cat << EOF
preload-docs.sh - Preload Claude framework files for XPath caching

USAGE:
  ./scripts/preload-docs.sh [OPTIONS]

OPTIONS:
  -h, --help              Show this help message
  -c, --cache-dir DIR     Cache directory (default: ~/.cache/claude-doc-optimizer)
  -d, --docs-root DIR     Documentation root (default: ~)
  -l, --list              List documents to be preloaded
  -v, --validate          Validate XML schemas only
  --clear-cache           Clear existing cache

ENVIRONMENT VARIABLES:
  CACHE_DIR               Override cache directory
  CLAUDE_DOCS_ROOT        Override documentation root
  PRELOAD_TIMEOUT         Timeout for preloading (default: 30s)

EXAMPLES:
  ./scripts/preload-docs.sh
  ./scripts/preload-docs.sh --list
  CACHE_DIR=/tmp/cache ./scripts/preload-docs.sh
  ./scripts/preload-docs.sh --clear-cache

EXIT CODES:
  0 = Success (all documents preloaded)
  1 = Partial failure (some documents skipped)
  2 = Configuration error (paths invalid)

EOF
}

# List documents
list_documents() {
    log_info "Framework documents to preload:"
    echo ""

    for i in "${!FRAMEWORK_DOCS[@]}"; do
        local doc="${FRAMEWORK_DOCS[$i]}"
        local doc_path="${DOCS_ROOT}/${doc}"

        if doc_exists "$doc"; then
            local size=$(get_doc_size "$doc")
            echo "  [$((i+1))] ✓ $doc ($size)"
        else
            echo "  [$((i+1))] ✗ $doc (NOT FOUND)"
        fi
    done
    echo ""
}

# Clear cache
clear_cache() {
    log_info "Clearing cache directory: $CACHE_DIR"

    if [[ -d "$CACHE_DIR" ]]; then
        rm -rf "${CACHE_DIR:?}"/* 2>/dev/null || {
            log_warn "Some files could not be removed (may require elevated privileges)"
        }
        log_success "Cache cleared"
    else
        log_warn "Cache directory does not exist"
    fi
}

# Validate schemas
validate_schemas() {
    log_info "Validating XML schemas..."

    if ! command -v xmllint &> /dev/null; then
        log_error "xmllint not found. Install libxml2-utils:"
        log_error "  Ubuntu/Debian: sudo apt-get install libxml2-utils"
        log_error "  macOS: brew install libxml2"
        return 2
    fi

    local all_valid=true

    for doc in "${FRAMEWORK_DOCS[@]}"; do
        if ! doc_exists "$doc"; then
            log_warn "Skipping (not found): $doc"
            continue
        fi

        local doc_path="${DOCS_ROOT}/${doc}"

        if timeout "$PRELOAD_TIMEOUT" xmllint --noout "$doc_path" 2>/dev/null; then
            log_success "Valid: $doc"
        else
            log_error "Invalid: $doc"
            all_valid=false
        fi
    done

    echo ""
    if $all_valid; then
        log_success "All XML documents are valid"
        return 0
    else
        log_error "Some documents failed validation"
        return 1
    fi
}

# Main entry point
main() {
    # Create log file
    mkdir -p "$CACHE_DIR"
    touch "$LOG_FILE"

    log_info "preload-docs.sh started"
    log_info "PID: $$, User: $(id -un), Date: $(date)"

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                usage
                exit 0
                ;;
            -c|--cache-dir)
                CACHE_DIR="$2"
                shift 2
                ;;
            -d|--docs-root)
                DOCS_ROOT="$2"
                shift 2
                ;;
            -l|--list)
                validate_paths || exit 2
                list_documents
                exit 0
                ;;
            -v|--validate)
                validate_paths || exit 2
                validate_schemas
                exit $?
                ;;
            --clear-cache)
                clear_cache
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                usage
                exit 2
                ;;
        esac
    done

    # Validate and preload
    validate_paths || exit 2
    preload_all
}

# Run main
main "$@"
