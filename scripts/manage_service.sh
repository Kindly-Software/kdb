#!/bin/bash
################################################################################
# Atomic Capsule HTTP Server - Service Management Script
# Production-grade systemd service manager with diagnostics and health checks
################################################################################

set -euo pipefail

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SERVICE_NAME="atomic-http-server"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
CONFIG_FILE="/home/samuel/Primitives/config/server.toml"
LOG_DIR="/home/samuel/Primitives/logs"
DATA_DIR="/home/samuel/Primitives/data"

################################################################################
# Helper Functions
################################################################################

print_info() {
    echo -e "${BLUE}ℹ${NC} $*"
}

print_success() {
    echo -e "${GREEN}✓${NC} $*"
}

print_error() {
    echo -e "${RED}✗${NC} $*"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $*"
}

check_sudo() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This command requires sudo privileges"
        exit 1
    fi
}

print_header() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

################################################################################
# Service Commands
################################################################################

cmd_start() {
    print_header "Starting Atomic HTTP Server"
    check_sudo

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        print_warning "Service is already running"
        return 0
    fi

    print_info "Starting service..."
    if sudo systemctl start "$SERVICE_NAME"; then
        sleep 2
        if systemctl is-active --quiet "$SERVICE_NAME"; then
            print_success "Service started successfully"
            cmd_status
        else
            print_error "Service failed to start. Check logs:"
            sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
            exit 1
        fi
    else
        print_error "Failed to start service"
        exit 1
    fi
}

cmd_stop() {
    print_header "Stopping Atomic HTTP Server"
    check_sudo

    if ! systemctl is-active --quiet "$SERVICE_NAME"; then
        print_warning "Service is not running"
        return 0
    fi

    print_info "Stopping service (graceful timeout: 30s)..."
    sudo systemctl stop "$SERVICE_NAME"
    print_success "Service stopped"
}

cmd_restart() {
    print_header "Restarting Atomic HTTP Server"
    check_sudo

    print_info "Restarting service..."
    if sudo systemctl restart "$SERVICE_NAME"; then
        sleep 2
        if systemctl is-active --quiet "$SERVICE_NAME"; then
            print_success "Service restarted successfully"
            cmd_status
        else
            print_error "Service failed to restart. Check logs:"
            sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
            exit 1
        fi
    else
        print_error "Failed to restart service"
        exit 1
    fi
}

cmd_status() {
    print_header "Service Status"

    # Basic status
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        print_success "Service is RUNNING"
    else
        print_error "Service is STOPPED"
    fi

    if systemctl is-enabled --quiet "$SERVICE_NAME"; then
        print_success "Service is ENABLED (auto-start on boot)"
    else
        print_warning "Service is DISABLED (manual start only)"
    fi

    echo ""
    print_info "Detailed status:"
    sudo systemctl status "$SERVICE_NAME" --no-pager

    # Process details
    if systemctl is-active --quiet "$SERVICE_NAME"; then
        echo ""
        print_info "Process information:"
        PID=$(sudo systemctl show -p MainPID --value "$SERVICE_NAME")
        if [[ "$PID" != "0" ]]; then
            print_info "PID: $PID"
            print_info "Memory (RSS): $(ps -p "$PID" -o rss=) KB"
            print_info "CPU Time: $(ps -p "$PID" -o cputime=)"
            print_info "Number of threads: $(ps -p "$PID" -o nlwp=)"
        fi
    fi
}

cmd_logs() {
    print_header "Service Logs (Real-time)"
    print_info "Following logs... (Press Ctrl+C to exit)"
    echo ""
    sudo journalctl -u "$SERVICE_NAME" -f --no-pager
}

cmd_logs_tail() {
    local lines="${1:-50}"
    print_header "Service Logs (Last $lines lines)"
    sudo journalctl -u "$SERVICE_NAME" -n "$lines" --no-pager
}

cmd_enable() {
    print_header "Enabling Auto-Start"
    check_sudo

    print_info "Enabling service to start on boot..."
    if sudo systemctl enable "$SERVICE_NAME"; then
        print_success "Service enabled for auto-start"
    else
        print_error "Failed to enable service"
        exit 1
    fi
}

cmd_disable() {
    print_header "Disabling Auto-Start"
    check_sudo

    print_info "Disabling service auto-start..."
    if sudo systemctl disable "$SERVICE_NAME"; then
        print_success "Service disabled for auto-start"
    else
        print_error "Failed to disable service"
        exit 1
    fi
}

cmd_reload() {
    print_header "Reloading Service Configuration"
    check_sudo

    if ! systemctl is-active --quiet "$SERVICE_NAME"; then
        print_error "Service is not running. Start it first."
        exit 1
    fi

    print_info "Sending SIGHUP to reload configuration..."
    sudo systemctl reload "$SERVICE_NAME"
    print_success "Configuration reload signal sent"
}

cmd_install() {
    print_header "Installing Service"
    check_sudo

    print_info "Checking prerequisites..."

    # Check if service file exists
    if [[ ! -f "/home/samuel/Primitives/config/${SERVICE_NAME}.service" ]]; then
        print_error "Service file not found at /home/samuel/Primitives/config/${SERVICE_NAME}.service"
        exit 1
    fi

    print_info "Creating required directories..."
    mkdir -p "$LOG_DIR" "$DATA_DIR"
    sudo chown samuel:samuel "$LOG_DIR" "$DATA_DIR"
    sudo chmod 755 "$LOG_DIR" "$DATA_DIR"

    print_info "Copying service file to systemd..."
    sudo cp "/home/samuel/Primitives/config/${SERVICE_NAME}.service" "$SERVICE_FILE"
    sudo chmod 644 "$SERVICE_FILE"

    print_info "Reloading systemd daemon..."
    sudo systemctl daemon-reload

    print_success "Service installed successfully"

    echo ""
    print_info "Next steps:"
    echo "  1. Start the service:  sudo systemctl start $SERVICE_NAME"
    echo "  2. Enable auto-start:  sudo systemctl enable $SERVICE_NAME"
    echo "  3. Check status:       sudo systemctl status $SERVICE_NAME"
    echo "  4. View logs:          sudo journalctl -u $SERVICE_NAME -f"
}

cmd_uninstall() {
    print_header "Uninstalling Service"
    check_sudo

    print_warning "This will remove the service from systemd"

    if systemctl is-active --quiet "$SERVICE_NAME"; then
        print_info "Stopping running service..."
        sudo systemctl stop "$SERVICE_NAME"
    fi

    print_info "Removing service file..."
    sudo rm -f "$SERVICE_FILE"

    print_info "Reloading systemd daemon..."
    sudo systemctl daemon-reload

    print_success "Service uninstalled"
}

cmd_health_check() {
    print_header "Health Check"

    if ! systemctl is-active --quiet "$SERVICE_NAME"; then
        print_error "Service is not running"
        return 1
    fi

    PID=$(sudo systemctl show -p MainPID --value "$SERVICE_NAME")

    if [[ "$PID" == "0" ]]; then
        print_error "Cannot determine service PID"
        return 1
    fi

    print_success "Service is running (PID: $PID)"

    # Check if process is responsive
    if kill -0 "$PID" 2>/dev/null; then
        print_success "Process is responsive"
    else
        print_error "Process is not responsive"
        return 1
    fi

    # Check resource usage
    echo ""
    print_info "Resource usage:"
    if ps -p "$PID" > /dev/null 2>&1; then
        RSS=$(ps -p "$PID" -o rss= | tr -d ' ')
        VSZ=$(ps -p "$PID" -o vsz= | tr -d ' ')
        PCPU=$(ps -p "$PID" -o %cpu= | tr -d ' ')
        PMEM=$(ps -p "$PID" -o %mem= | tr -d ' ')

        print_info "  Memory (RSS): $((RSS / 1024)) MB"
        print_info "  Virtual: $((VSZ / 1024)) MB"
        print_info "  CPU: $PCPU%"
        print_info "  Memory %: $PMEM%"

        # Check limits
        if [[ -f "/proc/$PID/limits" ]]; then
            echo ""
            print_info "Resource limits:"
            grep "open files" "/proc/$PID/limits" | awk '{print "  Open files: " $4 " (soft), " $5 " (hard)"}'
            grep "processes" "/proc/$PID/limits" | awk '{print "  Processes: " $4 " (soft), " $5 " (hard)"}'
        fi
    fi

    return 0
}

cmd_security_check() {
    print_header "Security Analysis"
    check_sudo

    if ! command -v systemd-analyze &> /dev/null; then
        print_warning "systemd-analyze not available, skipping security check"
        return 0
    fi

    print_info "Running systemd security profile analysis..."
    echo ""
    sudo systemd-analyze security "$SERVICE_NAME" --no-pager || true
}

cmd_validate_config() {
    print_header "Validating Configuration"

    if [[ ! -f "$CONFIG_FILE" ]]; then
        print_error "Configuration file not found: $CONFIG_FILE"
        exit 1
    fi

    print_success "Configuration file found"

    # Check if TOML is valid (requires toml-cli or similar)
    if command -v toml &> /dev/null; then
        if toml get "$CONFIG_FILE" > /dev/null 2>&1; then
            print_success "Configuration file is valid TOML"
        else
            print_error "Configuration file has TOML syntax errors"
            exit 1
        fi
    else
        print_warning "toml-cli not installed, skipping TOML validation"
    fi

    # Check permissions
    if [[ -r "$CONFIG_FILE" ]]; then
        print_success "Configuration file is readable"
    else
        print_error "Configuration file is not readable"
        exit 1
    fi
}

cmd_test_startup() {
    print_header "Testing Service Startup"
    check_sudo

    print_warning "This will restart the service"

    print_info "Stopping service..."
    sudo systemctl stop "$SERVICE_NAME" || true
    sleep 2

    print_info "Starting service..."
    START_TIME=$(date +%s%N)

    if sudo systemctl start "$SERVICE_NAME"; then
        sleep 3
        END_TIME=$(date +%s%N)
        DURATION=$(( (END_TIME - START_TIME) / 1000000 ))

        if systemctl is-active --quiet "$SERVICE_NAME"; then
            print_success "Service started successfully in ${DURATION}ms"
            cmd_health_check
        else
            print_error "Service failed to stay running after startup"
            echo ""
            print_info "Recent logs:"
            sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
            exit 1
        fi
    else
        print_error "Service failed to start"
        exit 1
    fi
}

cmd_crash_test() {
    print_header "Testing Crash Recovery"
    check_sudo

    if ! systemctl is-active --quiet "$SERVICE_NAME"; then
        print_error "Service is not running"
        exit 1
    fi

    PID=$(sudo systemctl show -p MainPID --value "$SERVICE_NAME")

    if [[ "$PID" == "0" ]]; then
        print_error "Cannot determine service PID"
        exit 1
    fi

    print_warning "About to kill process $PID to test crash recovery"
    print_info "Waiting 5 seconds... (Press Ctrl+C to cancel)"
    sleep 5

    print_info "Killing process..."
    sudo kill -9 "$PID"
    sleep 2

    print_info "Checking if service auto-restarted..."
    NEW_PID=$(sudo systemctl show -p MainPID --value "$SERVICE_NAME")

    if [[ "$NEW_PID" != "0" && "$NEW_PID" != "$PID" ]]; then
        print_success "Service auto-restarted successfully (new PID: $NEW_PID)"
    else
        print_error "Service failed to auto-restart"
        exit 1
    fi
}

################################################################################
# Help and Main
################################################################################

print_usage() {
    cat << EOF
${BLUE}Atomic Capsule HTTP Server - Service Manager${NC}

${BLUE}USAGE:${NC}
    $0 <command> [options]

${BLUE}COMMANDS:${NC}
    ${GREEN}start${NC}              Start the service
    ${GREEN}stop${NC}               Stop the service
    ${GREEN}restart${NC}            Restart the service
    ${GREEN}status${NC}             Show service status
    ${GREEN}logs${NC}               Follow service logs (real-time)
    ${GREEN}logs-tail${NC} [N]      Show last N log lines (default: 50)
    ${GREEN}enable${NC}             Enable auto-start on boot
    ${GREEN}disable${NC}            Disable auto-start on boot
    ${GREEN}reload${NC}             Reload service configuration (SIGHUP)
    ${GREEN}install${NC}            Install service to systemd
    ${GREEN}uninstall${NC}          Uninstall service from systemd
    ${GREEN}health${NC}             Run health check
    ${GREEN}security${NC}           Show security profile analysis
    ${GREEN}validate${NC}           Validate configuration file
    ${GREEN}test-startup${NC}       Test service startup
    ${GREEN}test-crash${NC}         Test crash recovery mechanism
    ${GREEN}help${NC}               Show this help message

${BLUE}EXAMPLES:${NC}
    # Install and start the service
    sudo $0 install
    sudo $0 start
    sudo $0 enable

    # Monitor service
    $0 status
    $0 logs
    $0 health

    # Test recovery
    sudo $0 test-startup
    sudo $0 test-crash

    # Administration
    sudo $0 restart
    sudo $0 stop

${BLUE}NOTES:${NC}
    - Most commands require sudo privileges
    - Logs are sent to journald (systemd journal)
    - Service auto-restarts on crash after 5 seconds
    - Max 3 restart attempts per 60 seconds (rate limiting)

${BLUE}SERVICE FILE:${NC}
    $SERVICE_FILE

${BLUE}CONFIG FILE:${NC}
    $CONFIG_FILE

EOF
}

main() {
    local command="${1:-help}"

    case "$command" in
        start)          cmd_start ;;
        stop)           cmd_stop ;;
        restart)        cmd_restart ;;
        status)         cmd_status ;;
        logs)           cmd_logs ;;
        logs-tail)      cmd_logs_tail "${2:-50}" ;;
        enable)         cmd_enable ;;
        disable)        cmd_disable ;;
        reload)         cmd_reload ;;
        install)        cmd_install ;;
        uninstall)      cmd_uninstall ;;
        health)         cmd_health_check ;;
        security)       cmd_security_check ;;
        validate)       cmd_validate_config ;;
        test-startup)   cmd_test_startup ;;
        test-crash)     cmd_crash_test ;;
        help|-h|--help) print_usage ;;
        *)
            print_error "Unknown command: $command"
            echo ""
            print_usage
            exit 1
            ;;
    esac
}

main "$@"
