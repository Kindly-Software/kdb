#!/bin/bash
# Install system responsiveness daemon
# Computational capsule-based monitoring (T6 Mixed: T1+T4+T5)

set -e

echo "🚀 Installing System Responsiveness Daemon"
echo "================================================"

# Build release binary
echo "📦 Building release binary..."
cargo build --release

# Install binary
echo "📋 Installing binary to ~/bin/sysrespond..."
mkdir -p ~/bin
cp target/release/sysrespond ~/bin/
chmod +x ~/bin/sysrespond

# Install systemd service (user service)
echo "⚙️  Installing systemd user service..."
mkdir -p ~/.config/systemd/user
cp systemd/sysrespond.service ~/.config/systemd/user/

# Create data directories
echo "📁 Creating data directories..."
mkdir -p ~/.local/share/sysrespond
mkdir -p ~/.config/sysrespond

# Create default config (if doesn't exist)
if [ ! -f ~/.config/sysrespond/config.toml ]; then
    echo "📝 Creating default configuration..."
    cat > ~/.config/sysrespond/config.toml <<'EOF'
# System Responsiveness Daemon Configuration
# Computational Capsule Architecture: T6 (Mixed)

[thresholds]
# CPU threshold for hung detection (percentage)
cpu_threshold_pct = 100.0

# Runtime threshold for hung detection (seconds)
runtime_threshold_sec = 300  # 5 minutes

# Scan interval (seconds)
scan_interval_sec = 10

# Grace period before SIGKILL (seconds)
sigkill_grace_sec = 30

[circuit_breaker]
# Kills per minute before circuit trips
kill_threshold = 5

# Cooldown period (seconds)
cooldown_sec = 60

[test_patterns]
# Process name patterns to detect as tests (conservative kill criteria)
patterns = [
    "test",
    "bench",
    "resource_exhaustion",
    "integration_test",
    "lockfree_table_bench",
    "parallel_training",
]

[whitelist_patterns]
# Process names to NEVER kill
patterns = [
    "claude",
    "firefox",
    "gnome-shell",
    "systemd",
    "X11",
    "Xorg",
]
EOF
fi

# Reload systemd user daemon
echo "🔄 Reloading systemd user daemon..."
systemctl --user daemon-reload

# Enable and start service
echo "✅ Enabling and starting service..."
systemctl --user enable sysrespond.service
systemctl --user start sysrespond.service

# Check status
echo ""
echo "📊 Service Status:"
systemctl --user status sysrespond.service --no-pager

echo ""
echo "✅ Installation complete!"
echo ""
echo "Commands:"
echo "  • View logs:    journalctl --user -u sysrespond.service -f"
echo "  • Stop service: systemctl --user stop sysrespond.service"
echo "  • Status:       systemctl --user status sysrespond.service"
echo "  • Edit config:  vi ~/.config/sysrespond/config.toml"
echo ""
echo "🎯 The daemon is now monitoring for hung processes!"
