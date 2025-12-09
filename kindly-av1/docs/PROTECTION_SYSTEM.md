# kindly-av1 Protection System

## Overview
kindly-av1 includes a multi-layer protection system to prevent unauthorized use and tampering. This document explains how it works and how to resolve common issues.

## Protection Layers

### Detection Methods
1. **Binary Integrity** - Verifies executable hasn't been modified
2. **Debugger Detection** - Detects attached debuggers (gdb, lldb, strace)
3. **Memory Checksum** - Validates code sections haven't been patched
4. **Import Table** - Checks system library addresses
5. **Timing Analysis** - Detects execution slowdowns from instrumentation
6. **Stack Canary** - Detects stack-based attacks
7. **Heap Integrity** - Verifies heap isn't corrupted
8. **Environment Check** - Detects LD_PRELOAD library injection

### Escalation Tiers
- **Tier 1 (Warning)**: 1-2 detections, logged but encoding continues
- **Tier 2 (Degrade)**: 3-4 detections, output limited to 720p with watermark
- **Tier 3 (Corrupt)**: Extended detections, output may contain artifacts
- **Tier 4 (Ban)**: 5+ detections, permanent hardware ban

## For Developers

### Development Mode
Set `KINDLY_DEV_MODE=1` to disable protection during development:

```bash
KINDLY_DEV_MODE=1 kindly-av1 encode input.mp4 -o output.av1
```

### Exempt Environments
Protection is automatically disabled in:
- Docker containers (DOCKER_HOST)
- WSL (WSL_DISTRO_NAME)
- CI/CD (GITHUB_ACTIONS, GITLAB_CI, CI)
- IDEs (VSCODE_PID, JETBRAINS_IDE)

## Troubleshooting

### "Hardware Banned" Error
If you receive a hardware ban:
1. Check if you were running a debugger or profiler
2. Contact samuel@kindly.software with your hardware ID
3. Include details about what you were doing
4. We may issue a one-time reset code

### Applying a Reset Code
```bash
kindly-av1 license reset-ban KINDLY-XXXX-XXXX-XXXX
```

### Log Files
Tamper events are logged to: `~/.config/kindly-av1/tamper_events.log`
Ban status stored in: `~/.kindly/ban.enc`

## FAQ

### Why was I banned?
Common causes:
- Running under a debugger
- Using profiling tools (valgrind, perf)
- LD_PRELOAD library injection
- Running in certain sandboxes

### Is my data safe?
Yes. The protection system only monitors the kindly-av1 process itself. It does not access your video files or other data beyond what's needed for encoding.

### Can I use kindly-av1 in Docker?
Yes! Docker environments are automatically exempted from protection checks.

## Support
- Email: samuel@kindly.software
- Include hardware ID and tamper_events.log when reporting issues
