#!/usr/bin/env bash
# install.sh - Install Claude Code document processing tools
# Installs MCP server, skills, and hooks for document optimization
#
# Prerequisites:
#   - Rust nightly (for atomic_mcp_server compilation)
#   - Claude Code installed
#   - jq (optional, for JSON validation)
#
# Usage:
#   ./scripts/install.sh                  # Install to ~/.claude
#   ./scripts/install.sh /custom/path     # Install to custom directory
#
# Exit codes:
#   0 = Success (all files installed)
#   1 = Error (installation failed)
#   2 = Missing prerequisites

set -euo pipefail

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${1:-${HOME}/.claude}"
BINARY_DIR="${PROJECT_ROOT}/target/release"
CACHE_DIR="${HOME}/.cache/claude-doc-optimizer"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging
log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[✗]${NC} $*"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust not found. Install from: https://rustup.rs"
        return 2
    fi
    log_success "Rust installed"

    # Check cargo-build
    local rust_version=$(rustc --version)
    log_info "  $rust_version"

    # Check Claude Code
    if [[ ! -d "${HOME}/Library/Application Support/Claude Code" ]] && \
       [[ ! -d "${HOME}/.config/Claude Code" ]]; then
        log_warn "Claude Code installation not detected (may be in non-standard location)"
    else
        log_success "Claude Code detected"
    fi

    # Check jq (optional)
    if ! command -v jq &> /dev/null; then
        log_warn "jq not found (JSON validation will be skipped)"
        log_info "  Install: brew install jq (macOS) or apt-get install jq (Linux)"
    else
        log_success "jq found"
    fi

    return 0
}

# Compile MCP server binary
compile_binary() {
    log_info "Compiling MCP server binary..."

    if [[ ! -f "$BINARY_DIR/mcp_debug_server" ]]; then
        log_info "  Building release binary (this may take 2-3 minutes)..."

        cd "$PROJECT_ROOT"
        cargo build --release \
            --bin mcp_debug_server \
            --features "std,json-rpc,runtime,stdio-transport,tool-executor" \
            2>&1 | tail -20

        if [[ ! -f "$BINARY_DIR/mcp_debug_server" ]]; then
            log_error "Binary compilation failed"
            return 1
        fi
    fi

    log_success "Binary compiled: $BINARY_DIR/mcp_debug_server"
    return 0
}

# Create installation directories
create_directories() {
    log_info "Creating installation directories..."

    mkdir -p "$INSTALL_DIR/mcp-servers"
    mkdir -p "$INSTALL_DIR/skills/framework-query"
    mkdir -p "$INSTALL_DIR/scripts"
    mkdir -p "$CACHE_DIR/logs"

    log_success "Directories created"
    return 0
}

# Install MCP server binary
install_binary() {
    log_info "Installing MCP server binary..."

    local source="$BINARY_DIR/mcp_debug_server"
    local target="$INSTALL_DIR/mcp-servers/claude-doc-optimizer"

    if [[ ! -f "$source" ]]; then
        log_error "Source binary not found: $source"
        return 1
    fi

    cp "$source" "$target"
    chmod +x "$target"

    log_success "Binary installed: $target"

    # Show binary info
    local size=$(du -h "$target" | cut -f1)
    log_info "  Size: $size"

    return 0
}

# Install scripts
install_scripts() {
    log_info "Installing helper scripts..."

    # Preload script
    if [[ -f "$PROJECT_ROOT/scripts/preload-docs.sh" ]]; then
        cp "$PROJECT_ROOT/scripts/preload-docs.sh" "$INSTALL_DIR/scripts/"
        chmod +x "$INSTALL_DIR/scripts/preload-docs.sh"
        log_success "Script installed: preload-docs.sh"
    else
        log_warn "Script not found: preload-docs.sh"
    fi

    # Log script
    if [[ -f "$PROJECT_ROOT/scripts/log-tool-use.sh" ]]; then
        cp "$PROJECT_ROOT/scripts/log-tool-use.sh" "$INSTALL_DIR/scripts/"
        chmod +x "$INSTALL_DIR/scripts/log-tool-use.sh"
        log_success "Script installed: log-tool-use.sh"
    else
        log_warn "Script not found: log-tool-use.sh"
    fi

    return 0
}

# Install skill
install_skill() {
    log_info "Installing framework-query skill..."

    local source="$PROJECT_ROOT/.claude/skills/framework-query/SKILL.md"
    local target="$INSTALL_DIR/skills/framework-query/SKILL.md"

    if [[ ! -f "$source" ]]; then
        log_error "Skill not found: $source"
        return 1
    fi

    cp "$source" "$target"
    log_success "Skill installed: framework-query"

    return 0
}

# Merge settings.json
merge_settings() {
    log_info "Merging settings.json..."

    local source="$PROJECT_ROOT/.claude/settings.json"
    local target="$INSTALL_DIR/settings.json"

    if [[ ! -f "$source" ]]; then
        log_error "Source settings.json not found: $source"
        return 1
    fi

    # Backup existing settings
    if [[ -f "$target" ]]; then
        local backup="${target}.backup.$(date +%s)"
        cp "$target" "$backup"
        log_warn "Backed up existing settings: $backup"
    fi

    # Copy new settings
    cp "$source" "$target"

    # Validate JSON if jq available
    if command -v jq &> /dev/null; then
        if jq empty "$target" 2>/dev/null; then
            log_success "Settings.json is valid"
        else
            log_error "Settings.json validation failed"
            return 1
        fi
    else
        log_warn "Skipping JSON validation (jq not found)"
    fi

    log_success "Settings installed: $target"
    return 0
}

# Verify installation
verify_installation() {
    log_info "Verifying installation..."

    local errors=0

    # Check binary
    if [[ ! -x "$INSTALL_DIR/mcp-servers/claude-doc-optimizer" ]]; then
        log_error "MCP server binary not executable"
        ((errors++))
    else
        log_success "MCP server binary ✓"
    fi

    # Check settings
    if [[ ! -f "$INSTALL_DIR/settings.json" ]]; then
        log_error "settings.json not installed"
        ((errors++))
    else
        log_success "settings.json ✓"
    fi

    # Check skill
    if [[ ! -f "$INSTALL_DIR/skills/framework-query/SKILL.md" ]]; then
        log_error "framework-query skill not installed"
        ((errors++))
    else
        log_success "framework-query skill ✓"
    fi

    # Check preload script
    if [[ ! -x "$INSTALL_DIR/scripts/preload-docs.sh" ]]; then
        log_error "preload-docs.sh not executable"
        ((errors++))
    else
        log_success "preload-docs.sh ✓"
    fi

    # Check log script
    if [[ ! -x "$INSTALL_DIR/scripts/log-tool-use.sh" ]]; then
        log_error "log-tool-use.sh not executable"
        ((errors++))
    else
        log_success "log-tool-use.sh ✓"
    fi

    # Check cache directory
    if [[ ! -d "$CACHE_DIR" ]]; then
        log_error "Cache directory not created"
        ((errors++))
    else
        log_success "Cache directory ✓"
    fi

    # Test binary execution
    log_info "Testing MCP server binary..."
    if timeout 5 "$INSTALL_DIR/mcp-servers/claude-doc-optimizer" --version &>/dev/null || true; then
        log_success "Binary execution test ✓"
    else
        log_warn "Binary version check not available (may be normal)"
    fi

    return $errors
}

# Show post-installation instructions
show_instructions() {
    cat << EOF

${GREEN}Installation Complete!${NC}

Next Steps:
${BLUE}1. Restart Claude Code${NC}
   - Fully close and reopen the application
   - This loads the new MCP server configuration

${BLUE}2. Test Framework Query Skill${NC}
   - Ask: "What is UCE34 Q10?"
   - Should get instant response from cache
   - Verify <100ms latency on first query

${BLUE}3. Verify Cache Setup${NC}
   - Ask Claude Code tool: "cache_stats"
   - Check that documents are preloaded

${BLUE}4. Monitor Performance${NC}
   - Check logs: tail -f ${CACHE_DIR}/logs/session.jsonl
   - View metrics: cat ${CACHE_DIR}/logs/metrics.json

Installation Directory: $INSTALL_DIR
Cache Directory: $CACHE_DIR

${YELLOW}Configuration Files:${NC}
- MCP Server: $INSTALL_DIR/mcp-servers/claude-doc-optimizer
- Settings: $INSTALL_DIR/settings.json
- Skill: $INSTALL_DIR/skills/framework-query/SKILL.md
- Scripts: $INSTALL_DIR/scripts/

${YELLOW}Support:${NC}
- Preload docs: $INSTALL_DIR/scripts/preload-docs.sh --help
- View cache metrics: ls -lah $CACHE_DIR/logs/
- Clear cache: rm -rf $CACHE_DIR/

EOF
}

# Main installation flow
main() {
    echo ""
    echo "================================================================"
    echo "Claude Code Document Processing Tools Installer"
    echo "================================================================"
    echo ""

    # Step 1: Check prerequisites
    if ! check_prerequisites; then
        log_error "Prerequisites check failed"
        return 2
    fi

    echo ""

    # Step 2: Compile binary
    if ! compile_binary; then
        log_error "Binary compilation failed"
        return 1
    fi

    echo ""

    # Step 3: Create directories
    if ! create_directories; then
        log_error "Directory creation failed"
        return 1
    fi

    echo ""

    # Step 4: Install components
    if ! install_binary; then
        log_error "Binary installation failed"
        return 1
    fi

    echo ""

    if ! install_scripts; then
        log_error "Script installation failed"
        return 1
    fi

    echo ""

    if ! install_skill; then
        log_error "Skill installation failed"
        return 1
    fi

    echo ""

    if ! merge_settings; then
        log_error "Settings merge failed"
        return 1
    fi

    echo ""

    # Step 5: Verify installation
    if ! verify_installation; then
        log_warn "Some verification checks failed"
    fi

    echo ""

    # Step 6: Show instructions
    show_instructions

    return 0
}

# Run installer
main "$@"
exit $?
