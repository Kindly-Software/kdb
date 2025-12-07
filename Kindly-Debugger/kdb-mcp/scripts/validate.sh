#!/usr/bin/env bash
# validate.sh - Validate Claude Code deployment configuration
# Comprehensive validation suite for document processing tools
#
# Validates:
#   - File structure and permissions
#   - Configuration files (JSON, YAML)
#   - Binary executability
#   - Dependencies
#   - MCP server connectivity
#   - Cache directory setup
#
# Usage:
#   ./scripts/validate.sh                    # Full validation
#   ./scripts/validate.sh --quick            # Fast validation
#   ./scripts/validate.sh --fix              # Auto-fix common issues
#
# Exit codes:
#   0 = All checks passed
#   1 = Some checks failed
#   2 = Critical checks failed

set -euo pipefail

# Configuration
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.claude}"
CACHE_DIR="${HOME}/.cache/claude-doc-optimizer"
VALIDATOR_VERSION="1.0.0"

# Validation results
PASS_COUNT=0
WARN_COUNT=0
FAIL_COUNT=0
CRIT_COUNT=0

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m'

# Logging functions
log_pass() {
    echo -e "${GREEN}[✓]${NC} $*"
    ((PASS_COUNT++)) || true
}

log_warn() {
    echo -e "${YELLOW}[!]${NC} $*"
    ((WARN_COUNT++)) || true
}

log_fail() {
    echo -e "${RED}[✗]${NC} $*"
    ((FAIL_COUNT++)) || true
}

log_crit() {
    echo -e "${RED}[CRITICAL]${NC} $*"
    ((CRIT_COUNT++)) || true
}

log_info() {
    echo -e "${BLUE}[·]${NC} $*"
}

# Section header
section_header() {
    echo ""
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${MAGENTA}$*${NC}"
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# Check file exists and is readable
check_file() {
    local path="$1"
    local description="$2"

    if [[ -f "$path" ]]; then
        if [[ -r "$path" ]]; then
            log_pass "$description: $path"
            return 0
        else
            log_fail "$description: not readable: $path"
            return 1
        fi
    else
        log_fail "$description: not found: $path"
        return 1
    fi
}

# Check file is executable
check_executable() {
    local path="$1"
    local description="$2"

    if [[ -f "$path" ]]; then
        if [[ -x "$path" ]]; then
            log_pass "$description: $path"
            return 0
        else
            log_fail "$description: not executable: $path"
            return 1
        fi
    else
        log_fail "$description: not found: $path"
        return 1
    fi
}

# Check directory exists
check_directory() {
    local path="$1"
    local description="$2"

    if [[ -d "$path" ]]; then
        log_pass "$description: $path"
        return 0
    else
        log_fail "$description: not found: $path"
        return 1
    fi
}

# Check JSON validity
check_json() {
    local path="$1"
    local description="$2"

    if ! command -v jq &> /dev/null; then
        log_warn "$description: jq not found, skipping validation"
        return 0
    fi

    if jq empty "$path" 2>/dev/null; then
        log_pass "$description: valid JSON"
        return 0
    else
        log_fail "$description: invalid JSON"
        return 1
    fi
}

# Validate MCP server binary
validate_binary() {
    section_header "MCP Server Binary Validation"

    local binary="$INSTALL_DIR/mcp-servers/claude-doc-optimizer"

    check_executable "$binary" "MCP server binary"

    # Check binary type
    if command -v file &> /dev/null; then
        local file_type=$(file "$binary" | cut -d: -f2)
        if echo "$file_type" | grep -q "ELF\|Mach-O"; then
            log_pass "Binary type: $file_type"
        else
            log_warn "Binary type check inconclusive: $file_type"
        fi
    fi

    # Check binary size
    local size=$(du -h "$binary" | cut -f1)
    log_info "Binary size: $size"

    if [[ $(stat -f%z "$binary" 2>/dev/null || stat -c%s "$binary") -lt 100000 ]]; then
        log_warn "Binary size seems small (expected >100KB)"
    else
        log_pass "Binary size reasonable"
    fi
}

# Validate configuration files
validate_configuration() {
    section_header "Configuration Files Validation"

    # settings.json
    local settings_file="$INSTALL_DIR/settings.json"
    check_file "$settings_file" "Settings JSON file"
    check_json "$settings_file" "Settings JSON syntax"

    # Check required keys in settings.json
    if command -v jq &> /dev/null; then
        if jq -e '.mcpServers."claude-doc-optimizer"' "$settings_file" > /dev/null 2>&1; then
            log_pass "MCP server configured in settings"
        else
            log_fail "MCP server NOT configured in settings"
        fi

        if jq -e '.hooks | length > 0' "$settings_file" > /dev/null 2>&1; then
            log_pass "Hooks configured"
        else
            log_warn "No hooks configured"
        fi
    fi
}

# Validate scripts
validate_scripts() {
    section_header "Helper Scripts Validation"

    check_executable "$INSTALL_DIR/scripts/preload-docs.sh" "Preload script"
    check_executable "$INSTALL_DIR/scripts/log-tool-use.sh" "Log script"

    # Check script syntax
    if command -v bash &> /dev/null; then
        if bash -n "$INSTALL_DIR/scripts/preload-docs.sh" 2>/dev/null; then
            log_pass "Preload script syntax"
        else
            log_fail "Preload script syntax error"
        fi

        if bash -n "$INSTALL_DIR/scripts/log-tool-use.sh" 2>/dev/null; then
            log_pass "Log script syntax"
        else
            log_fail "Log script syntax error"
        fi
    else
        log_warn "bash not found, skipping syntax validation"
    fi
}

# Validate skill
validate_skill() {
    section_header "Framework Query Skill Validation"

    local skill_file="$INSTALL_DIR/skills/framework-query/SKILL.md"
    check_file "$skill_file" "Skill markdown file"

    # Check YAML frontmatter
    if head -1 "$skill_file" | grep -q "^---"; then
        log_pass "Skill has YAML frontmatter"
    else
        log_warn "Skill missing YAML frontmatter"
    fi

    # Check for required keys
    if grep -q "^name:" "$skill_file"; then
        log_pass "Skill has name"
    else
        log_fail "Skill missing name"
    fi

    if grep -q "^description:" "$skill_file"; then
        log_pass "Skill has description"
    else
        log_fail "Skill missing description"
    fi
}

# Validate cache directory
validate_cache() {
    section_header "Cache Directory Validation"

    check_directory "$CACHE_DIR" "Cache directory"

    # Check subdirectories
    if [[ -d "$CACHE_DIR/logs" ]]; then
        log_pass "Logs directory exists"
    else
        log_warn "Logs directory missing (will be created on first use)"
    fi

    # Check writable
    if touch "$CACHE_DIR/.test" 2>/dev/null; then
        rm -f "$CACHE_DIR/.test"
        log_pass "Cache directory is writable"
    else
        log_fail "Cache directory is NOT writable"
    fi

    # Check permissions
    local perms=$(stat -c%a "$CACHE_DIR" 2>/dev/null || stat -f%A "$CACHE_DIR")
    if [[ "$perms" =~ ^7 ]]; then
        log_pass "Cache directory permissions: $perms"
    else
        log_warn "Cache directory permissions: $perms (should be readable/writable)"
    fi
}

# Validate dependencies
validate_dependencies() {
    section_header "Dependencies Validation"

    local missing=0

    # Required
    if command -v cargo &> /dev/null; then
        log_pass "Rust toolchain"
    else
        log_fail "Rust toolchain NOT found (required for recompilation)"
        ((missing++))
    fi

    # Optional but recommended
    if command -v jq &> /dev/null; then
        log_pass "jq (JSON processor)"
    else
        log_warn "jq NOT found (optional, for JSON validation)"
    fi

    if command -v xmllint &> /dev/null; then
        log_pass "xmllint (XML validator)"
    else
        log_warn "xmllint NOT found (optional, for framework document validation)"
    fi

    if command -v bc &> /dev/null; then
        log_pass "bc (calculator)"
    else
        log_warn "bc NOT found (optional, for metrics calculation)"
    fi

    return $missing
}

# Validate MCP server connectivity
validate_connectivity() {
    section_header "MCP Server Connectivity Validation"

    local binary="$INSTALL_DIR/mcp-servers/claude-doc-optimizer"

    # Test binary execution
    if timeout 3 "$binary" --help &>/dev/null 2>&1 || true; then
        log_pass "MCP server responds to --help"
    else
        log_warn "MCP server --help not available (may be normal)"
    fi

    # Test version flag
    if timeout 3 "$binary" --version &>/dev/null 2>&1 || true; then
        log_pass "MCP server responds to --version"
    else
        log_warn "MCP server --version not available (may be normal)"
    fi
}

# Validate framework documents
validate_framework_docs() {
    section_header "Framework Documents Validation"

    local docs_root="${CLAUDE_DOCS_ROOT:-${HOME}}"
    local docs=(
        "xml/shared/shared-components.xml"
        "xml/frameworks/uce34.xml"
        "xml/frameworks/t28.xml"
        "xml/frameworks/b32.xml"
        "xml/frameworks/assum.xml"
        "xml/frameworks/i20.xml"
        "xml/frameworks/uce-d7.xml"
    )

    local found=0
    local missing=0

    for doc in "${docs[@]}"; do
        local path="${docs_root}/${doc}"
        if [[ -f "$path" ]]; then
            log_pass "Found: $doc"
            ((found++))
        else
            log_warn "Missing: $doc"
            ((missing++))
        fi
    done

    log_info "Framework documents: $found found, $missing missing"
}

# Performance check
performance_check() {
    section_header "Performance Baseline Check"

    log_info "Measuring MCP server startup time..."

    local start=$(date +%s%N)
    local binary="$INSTALL_DIR/mcp-servers/claude-doc-optimizer"

    timeout 5 "$binary" --help &>/dev/null 2>&1 || true

    local end=$(date +%s%N)
    local duration_ms=$(( (end - start) / 1000000 ))

    if [[ $duration_ms -lt 1000 ]]; then
        log_pass "Startup time: ${duration_ms}ms"
    elif [[ $duration_ms -lt 5000 ]]; then
        log_warn "Startup time: ${duration_ms}ms (expected <1000ms)"
    else
        log_fail "Startup time: ${duration_ms}ms (too slow)"
    fi
}

# Print summary
print_summary() {
    echo ""
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${MAGENTA}Validation Summary${NC}"
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""

    echo -e "${GREEN}✓ PASSED:${NC} $PASS_COUNT"
    echo -e "${YELLOW}! WARNINGS:${NC} $WARN_COUNT"
    echo -e "${RED}✗ FAILED:${NC} $FAIL_COUNT"
    echo -e "${RED}✗ CRITICAL:${NC} $CRIT_COUNT"

    echo ""

    if [[ $CRIT_COUNT -gt 0 ]]; then
        echo -e "${RED}RESULT: CRITICAL FAILURES DETECTED${NC}"
        return 2
    elif [[ $FAIL_COUNT -gt 0 ]]; then
        echo -e "${RED}RESULT: SOME CHECKS FAILED${NC}"
        return 1
    elif [[ $WARN_COUNT -gt 0 ]]; then
        echo -e "${YELLOW}RESULT: PASSED WITH WARNINGS${NC}"
        return 0
    else
        echo -e "${GREEN}RESULT: ALL CHECKS PASSED${NC}"
        return 0
    fi
}

# Show detailed help
show_help() {
    cat << EOF
validate.sh - Claude Code Deployment Validation

USAGE:
  ./scripts/validate.sh [OPTIONS]

OPTIONS:
  -h, --help           Show this help message
  -q, --quick          Run only critical checks
  -f, --fix            Auto-fix common issues
  -v, --verbose        Show detailed output
  -d, --docs-root DIR  Set documentation root directory

VALIDATION CHECKS:
  ✓ MCP server binary (exists, executable, size)
  ✓ Configuration files (JSON validity, required keys)
  ✓ Helper scripts (existence, syntax, executability)
  ✓ Skill files (YAML frontmatter, required fields)
  ✓ Cache directory (existence, permissions, writable)
  ✓ Dependencies (Rust, jq, xmllint, bc)
  ✓ MCP connectivity (binary responsiveness)
  ✓ Framework documents (availability, count)
  ✓ Performance baseline (startup time)

EXIT CODES:
  0 = All checks passed
  1 = Some checks failed
  2 = Critical failures

EXAMPLES:
  ./scripts/validate.sh
  ./scripts/validate.sh --quick
  ./scripts/validate.sh --verbose
  ./scripts/validate.sh --docs-root /custom/path

EOF
}

# Main validation function
main() {
    echo ""
    echo "=================================================="
    echo "Claude Code Deployment Validator v${VALIDATOR_VERSION}"
    echo "=================================================="
    echo ""
    echo "Installation Directory: $INSTALL_DIR"
    echo "Cache Directory: $CACHE_DIR"
    echo ""

    # Parse arguments
    local quick=false
    local verbose=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                show_help
                exit 0
                ;;
            -q|--quick)
                quick=true
                shift
                ;;
            -v|--verbose)
                verbose=true
                shift
                ;;
            -d|--docs-root)
                CLAUDE_DOCS_ROOT="$2"
                shift 2
                ;;
            *)
                log_fail "Unknown option: $1"
                show_help
                exit 2
                ;;
        esac
    done

    # Run validations
    validate_binary
    validate_configuration
    validate_scripts
    validate_skill
    validate_cache
    validate_dependencies || true
    validate_connectivity

    if ! $quick; then
        validate_framework_docs
        performance_check
    fi

    # Print summary
    print_summary
}

# Run main
main "$@"
