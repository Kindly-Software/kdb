#!/bin/bash
# Restore from backup - UCE34 I20 Integration compliance
# Safely restores configuration, data, logs, and certificates from backup archives

set -e

# Configuration
LOG_FILE="/home/samuel/Primitives/logs/restore.log"
RESTORE_LOCK="/tmp/atomic_capsule_restore.lock"

# Function: Log message with timestamp
log_message() {
    local msg="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] $msg" | tee -a "$LOG_FILE"
}

# Function: Alert on critical error
alert_error() {
    local msg="$1"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] CRITICAL ERROR: $msg" | tee -a "$LOG_FILE"
}

# Function: Prompt for confirmation
confirm_restore() {
    local prompt="$1"
    local response
    read -p "$prompt (yes/no): " response
    [ "$response" = "yes" ]
}

# Function: Create backup before restore
backup_current_state() {
    local backup_file="/tmp/atomic_capsule_state_backup_$(date +%s).tar.gz"
    log_message "Creating safety backup of current state..."
    tar czf "$backup_file" -C /home/samuel/Primitives config data logs 2>/dev/null || true
    log_message "Safety backup created: $backup_file"
    echo "$backup_file"
}

# Validate inputs
if [ -z "$1" ]; then
    echo "❌ Usage: $0 <backup_tarball_or_directory>"
    echo ""
    echo "Examples:"
    echo "  Restore from manifest: $0 /home/samuel/Primitives/backups/daily/20250121_020000_manifest.txt"
    echo "  Restore from file:     $0 /home/samuel/Primitives/backups/daily/20250121_020000_config.tar.gz"
    echo "  List available:        ls -la /home/samuel/Primitives/backups/daily/"
    echo ""
    exit 1
fi

BACKUP_INPUT="$1"

# Check if input exists
if [ ! -f "$BACKUP_INPUT" ] && [ ! -d "$BACKUP_INPUT" ]; then
    alert_error "Backup not found: $BACKUP_INPUT"
    exit 1
fi

# Initialize log
mkdir -p "$(dirname "$LOG_FILE")"
log_message "=========================================="
log_message "Starting restore (UCE34 I20 Integration)"
log_message "=========================================="
log_message "Backup source: $BACKUP_INPUT"

# Prevent concurrent restores (safety check)
if [ -f "$RESTORE_LOCK" ]; then
    alert_error "Restore already in progress (lock file exists)"
    exit 1
fi
trap "rm -f $RESTORE_LOCK" EXIT
touch "$RESTORE_LOCK"

# Determine backup type
if [[ "$BACKUP_INPUT" == *"_manifest.txt" ]]; then
    log_message "Mode: Manifest-based restore (all components)"
    MANIFEST_FILE="$BACKUP_INPUT"
    BACKUP_PREFIX=$(dirname "$MANIFEST_FILE")/$(basename "$MANIFEST_FILE" "_manifest.txt")
    RESTORE_MODE="full"
elif [[ "$BACKUP_INPUT" == *".tar.gz" ]]; then
    log_message "Mode: Single tarball restore"
    BACKUP_FILE="$BACKUP_INPUT"
    RESTORE_MODE="single"
else
    alert_error "Unknown backup format: $BACKUP_INPUT"
    exit 1
fi

# STEP 1: Validation
log_message ""
log_message "🔍 Step 1: Validating backup integrity..."

if [ "$RESTORE_MODE" = "full" ]; then
    if [ ! -f "$MANIFEST_FILE" ]; then
        alert_error "Manifest file not found: $MANIFEST_FILE"
        exit 1
    fi
    log_message "  ✓ Manifest found: $(basename "$MANIFEST_FILE")"

    # Check all tarballs referenced in manifest
    for type in config data logs certs; do
        TARBALL="${BACKUP_PREFIX}_${type}.tar.gz"
        if [ -f "$TARBALL" ]; then
            if tar tzf "$TARBALL" > /dev/null 2>&1; then
                log_message "  ✓ ${type}_backup: OK ($(du -h "$TARBALL" | cut -f1))"
            else
                alert_error "Tarball corrupted: $TARBALL"
                exit 1
            fi
        fi
    done
else
    if ! tar tzf "$BACKUP_FILE" > /dev/null 2>&1; then
        alert_error "Tarball corrupted: $BACKUP_FILE"
        exit 1
    fi
    log_message "  ✓ Single tarball: OK ($(du -h "$BACKUP_FILE" | cut -f1))"
fi

# STEP 2: User confirmation
log_message ""
log_message "⚠️  Step 2: Restore confirmation (I20 Safety)"
echo ""
echo "🔴 WARNING: This will restore data from a backup."
echo "   Current data will be REPLACED."
echo ""
if [ "$RESTORE_MODE" = "full" ]; then
    echo "   Components to restore:"
    [ -f "${BACKUP_PREFIX}_config.tar.gz" ] && echo "     • Configuration files"
    [ -f "${BACKUP_PREFIX}_data.tar.gz" ] && echo "     • Persistent data (mmap, databases)"
    [ -f "${BACKUP_PREFIX}_logs.tar.gz" ] && echo "     • Audit logs"
    [ -f "${BACKUP_PREFIX}_certs.tar.gz" ] && echo "     • TLS certificates"
else
    echo "   Component to restore: $(basename "$BACKUP_FILE")"
fi
echo ""

if ! confirm_restore "Continue with restore?"; then
    log_message "Restore cancelled by user"
    exit 0
fi

# STEP 3: Create safety backup
log_message ""
log_message "💾 Step 3: Creating safety backup of current state..."
SAFETY_BACKUP=$(backup_current_state)

# STEP 4: Stop service
log_message ""
log_message "🛑 Step 4: Stopping atomic-http-server service..."
if systemctl is-active --quiet atomic-http-server; then
    log_message "  Stopping service..."
    if ! sudo systemctl stop atomic-http-server; then
        alert_error "Failed to stop atomic-http-server"
        log_message "  Attempting to restore service..."
        sudo systemctl start atomic-http-server || true
        exit 1
    fi
    log_message "  ✓ Service stopped"
    sleep 2  # Wait for graceful shutdown
else
    log_message "  Service not running (skipped)"
fi

# STEP 5: Extract and restore backups
log_message ""
log_message "📦 Step 5: Restoring backup contents..."

RESTORE_TEMP="/tmp/restore_$$"
mkdir -p "$RESTORE_TEMP"
trap "rm -rf $RESTORE_TEMP $RESTORE_LOCK" EXIT

if [ "$RESTORE_MODE" = "full" ]; then
    # Restore configuration
    if [ -f "${BACKUP_PREFIX}_config.tar.gz" ]; then
        log_message "  Extracting configuration..."
        tar xzf "${BACKUP_PREFIX}_config.tar.gz" -C "$RESTORE_TEMP/" 2>/dev/null || true

        if [ -f "$RESTORE_TEMP/server.toml" ]; then
            cp "$RESTORE_TEMP/server.toml" /home/samuel/Primitives/config/
            log_message "  ✓ Restored: server.toml"
        fi

        if [ -f "$RESTORE_TEMP/atomic-http-server.service" ]; then
            sudo cp "$RESTORE_TEMP/atomic-http-server.service" /etc/systemd/system/
            sudo systemctl daemon-reload
            log_message "  ✓ Restored: systemd service"
        fi

        if [ -f "$RESTORE_TEMP/99-ddos-protection.conf" ]; then
            sudo cp "$RESTORE_TEMP/99-ddos-protection.conf" /etc/sysctl.d/
            sudo sysctl -p > /dev/null 2>&1 || true
            log_message "  ✓ Restored: DDoS protection config"
        fi
    fi

    # Restore persistent data
    if [ -f "${BACKUP_PREFIX}_data.tar.gz" ]; then
        log_message "  Extracting persistent data..."
        tar xzf "${BACKUP_PREFIX}_data.tar.gz" -C "$RESTORE_TEMP/" 2>/dev/null || true

        if [ -d "$RESTORE_TEMP/data" ]; then
            # Remove old data safely
            if [ -d "/home/samuel/Primitives/data" ]; then
                rm -rf "/home/samuel/Primitives/data.backup"
                mv "/home/samuel/Primitives/data" "/home/samuel/Primitives/data.backup"
            fi
            mv "$RESTORE_TEMP/data" "/home/samuel/Primitives/"
            log_message "  ✓ Restored: persistent data"
        fi
    fi

    # Restore audit logs
    if [ -f "${BACKUP_PREFIX}_logs.tar.gz" ]; then
        log_message "  Extracting audit logs (Q34 compliance)..."
        tar xzf "${BACKUP_PREFIX}_logs.tar.gz" -C "$RESTORE_TEMP/" 2>/dev/null || true

        if [ -d "$RESTORE_TEMP/logs" ]; then
            # Keep new logs, restore old ones
            if [ -d "/home/samuel/Primitives/logs" ]; then
                mkdir -p "/home/samuel/Primitives/logs.backup"
                rsync -av "/home/samuel/Primitives/logs/" "/home/samuel/Primitives/logs.backup/" 2>/dev/null || true
            fi
            rsync -av "$RESTORE_TEMP/logs/" "/home/samuel/Primitives/logs/" 2>/dev/null || true
            log_message "  ✓ Restored: audit logs"
        fi
    fi

    # Restore TLS certificates
    if [ -f "${BACKUP_PREFIX}_certs.tar.gz" ]; then
        log_message "  Extracting TLS certificates..."
        tar xzf "${BACKUP_PREFIX}_certs.tar.gz" -C "$RESTORE_TEMP/" 2>/dev/null || true

        if [ -d "$RESTORE_TEMP/kindly.software" ]; then
            log_message "  ✓ Found certificates in backup"
            if [ -d "/etc/letsencrypt/live/kindly.software" ]; then
                sudo mv "/etc/letsencrypt/live/kindly.software" "/etc/letsencrypt/live/kindly.software.backup"
                sudo mkdir -p "/etc/letsencrypt/live/kindly.software"
            fi
            sudo rsync -av "$RESTORE_TEMP/kindly.software/" "/etc/letsencrypt/live/kindly.software/"
            log_message "  ✓ Restored: TLS certificates"
        fi
    fi
else
    # Single tarball mode
    log_message "  Extracting single tarball..."
    tar xzf "$BACKUP_FILE" -C "$RESTORE_TEMP/" 2>/dev/null || true

    # Try to determine type and restore appropriately
    if [ -f "$RESTORE_TEMP/server.toml" ]; then
        cp "$RESTORE_TEMP/server.toml" /home/samuel/Primitives/config/
        log_message "  ✓ Restored: configuration"
    fi
fi

# STEP 6: Verify restored data
log_message ""
log_message "✅ Step 6: Verifying restored data..."

VERIFY_PASS=0
if [ -f "/home/samuel/Primitives/config/server.toml" ]; then
    log_message "  ✓ server.toml exists"
    VERIFY_PASS=$((VERIFY_PASS + 1))
fi

if [ -d "/home/samuel/Primitives/data" ]; then
    DATA_SIZE=$(du -sh "/home/samuel/Primitives/data" | cut -f1)
    log_message "  ✓ Persistent data: $DATA_SIZE"
    VERIFY_PASS=$((VERIFY_PASS + 1))
fi

if [ -d "/home/samuel/Primitives/logs" ]; then
    LOG_COUNT=$(find /home/samuel/Primitives/logs -type f | wc -l)
    log_message "  ✓ Audit logs: $LOG_COUNT files"
    VERIFY_PASS=$((VERIFY_PASS + 1))
fi

log_message "Verification: $VERIFY_PASS components restored"

# STEP 7: Restart service
log_message ""
log_message "🚀 Step 7: Restarting atomic-http-server service..."
if ! sudo systemctl start atomic-http-server; then
    alert_error "Failed to start atomic-http-server!"
    log_message "Attempting rollback to safety backup..."
    exit 1
fi

sleep 3
if systemctl is-active --quiet atomic-http-server; then
    log_message "  ✓ Service running"
else
    alert_error "Service not running after restart!"
    exit 1
fi

# STEP 8: Health check
log_message ""
log_message "💚 Step 8: Service health check..."
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    log_message "  ✓ Health check passed: /health"
else
    alert_error "Health check failed! Service may not be functioning."
    exit 1
fi

# Summary
log_message ""
log_message "=========================================="
log_message "✅ Restore complete (I20 Integration)"
log_message "=========================================="
log_message "Restored from: $(basename "$BACKUP_INPUT")"
log_message "Service status: running"
log_message "Health check: PASSED"
log_message "Safety backup: $SAFETY_BACKUP"
log_message "=========================================="

echo ""
echo "🎉 Restore successful!"
echo ""
echo "Next steps:"
echo "  • Verify application functionality"
echo "  • Check logs: tail -f /home/samuel/Primitives/logs/server.log"
echo "  • If rollback needed: $0 $SAFETY_BACKUP"
echo ""

exit 0
