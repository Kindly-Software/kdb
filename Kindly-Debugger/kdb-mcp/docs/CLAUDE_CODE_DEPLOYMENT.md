# Claude Code Deployment Guide

Complete documentation for deploying document processing tools to Claude Code with SessionStart preloading and framework-query skill.

## Overview

This deployment provides:

- **MCP Server**: T6 Mixed tier stdio transport with <10μs latency
- **XPath Cache**: 100,000× speedup for repeated framework queries
- **SessionStart Hook**: Automatic framework document preloading
- **Framework Query Skill**: Auto-activated for architecture questions
- **Performance Monitoring**: Built-in logging and metrics

## Prerequisites

### System Requirements

- macOS 10.13+ or Linux (any recent distribution)
- 500 MB disk space (binary + cache)
- 100 MB RAM (typical runtime)
- Bash 4.0+ (for scripts)

### Software Requirements

**Required**:
- Rust nightly (for recompilation): `rustup default nightly`
- Claude Code installed and running

**Recommended**:
- jq (JSON validation): `brew install jq` or `apt-get install jq`
- xmllint (XML validation): `brew install libxml2` or `apt-get install libxml2-utils`
- bc (calculation): standard on macOS/Linux

## Installation

### Quick Start (Recommended)

```bash
cd /home/samuel/Primitives/atomic_mcp_server
./scripts/install.sh

# Follow on-screen instructions
```

**Time**: 2-3 minutes (includes binary compilation)

**Exit codes**:
- `0` = Success
- `1` = Error (see logs)
- `2` = Missing prerequisites

### Custom Installation Directory

```bash
./scripts/install.sh /custom/path/.claude
```

### Manual Installation

If you prefer manual control:

```bash
# 1. Create directories
mkdir -p ~/.claude/mcp-servers
mkdir -p ~/.claude/skills/framework-query
mkdir -p ~/.claude/scripts

# 2. Copy binary
cp target/release/mcp_debug_server ~/.claude/mcp-servers/claude-doc-optimizer
chmod +x ~/.claude/mcp-servers/claude-doc-optimizer

# 3. Copy files
cp .claude/settings.json ~/.claude/
cp .claude/skills/framework-query/SKILL.md ~/.claude/skills/framework-query/
cp scripts/*.sh ~/.claude/scripts/
chmod +x ~/.claude/scripts/*.sh

# 4. Create cache directory
mkdir -p ~/.cache/claude-doc-optimizer/logs
```

## Validation

After installation, validate the deployment:

```bash
./scripts/validate.sh
```

**Full validation output**:
```
✓ PASSED:    12
! WARNINGS:   2
✗ FAILED:     0
✗ CRITICAL:   0

RESULT: ALL CHECKS PASSED
```

**Quick validation** (critical checks only):
```bash
./scripts/validate.sh --quick
```

**Verbose output** (detailed information):
```bash
./scripts/validate.sh --verbose
```

## Configuration

### MCP Server Settings

Location: `~/.claude/settings.json`

Key configuration:

```json
{
  "mcpServers": {
    "claude-doc-optimizer": {
      "command": "/path/to/mcp_debug_server",
      "args": ["--mode", "stdio", "--features", "document-tools"],
      "env": {
        "CLAUDE_DOCS_ROOT": "/home/samuel",
        "RUST_LOG": "info",
        "CACHE_SIZE": "1073741824"
      }
    }
  },
  "allowedTools": [
    "xpath_query",
    "validate_schema",
    "cache_stats",
    "preload_documents"
  ]
}
```

### Hooks Configuration

**SessionStart Hook** (automatic preload):
```json
{
  "matcher": "SessionStart",
  "hooks": [
    {
      "type": "command",
      "command": "/path/to/scripts/preload-docs.sh",
      "timeout": 10000
    }
  ]
}
```

**PostToolUse Hook** (analytics logging):
```json
{
  "matcher": "PostToolUse",
  "hooks": [
    {
      "type": "command",
      "command": "/path/to/scripts/log-tool-use.sh"
    }
  ]
}
```

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `CLAUDE_DOCS_ROOT` | `$HOME` | Framework documentation root |
| `CACHE_DIR` | `$HOME/.cache/claude-doc-optimizer` | Cache directory |
| `RUST_LOG` | `info` | Log level (debug, info, warn, error) |
| `CACHE_SIZE` | `1073741824` | Max cache size (1GB) |
| `PRELOAD_TIMEOUT` | `30` | Preload timeout (seconds) |

## Usage

### Framework Query Skill (Auto-Activated)

The framework-query skill automatically activates when you ask about:

**Framework concepts**:
```
"What is UCE34?"
"Explain T28"
"B32 methodology"
"ASSUM safety"
"I20 integration"
```

**Tier selection**:
```
"Which tier should I use?"
"T2 SIMD performance"
"T6 mixed tier"
"Tier comparison"
```

**Performance**:
```
"What speedups are possible?"
"Is 10× reasonable?"
"SIMD performance metrics"
```

### Manual XPath Queries

If you need explicit control:

```bash
# Query specific framework
xpath_query --document uce34 --xpath '//framework[@id="UCE34"]'

# Query tier definitions
xpath_query --document claude-v6 --xpath '//tier[@id="tier-t2"]'

# View cache statistics
cache_stats --show-metrics

# Preload documents
preload_documents --documents uce34,t28,b32
```

### Cache Management

**View cache statistics**:
```bash
cache_stats --show-metrics
```

Output:
```
XPath Cache Statistics
======================
Documents loaded: 7
Cached queries: 1,247
Hit rate: 98.3%
Cache size: 42 MB / 1 GB
Memory usage: 156 MB
Disk usage: 185 MB
TTL: 24 hours
```

**Clear cache** (if needed):
```bash
cache_stats --clear
# Next query triggers reload
```

**Force reload**:
```bash
xpath_query --document claude-v6 --cache-policy force-reload
```

## Troubleshooting

### MCP Server Not Found

**Problem**: Claude Code cannot find the MCP server

**Solutions**:
1. Verify binary exists: `ls -la ~/.claude/mcp-servers/claude-doc-optimizer`
2. Verify it's executable: `file ~/.claude/mcp-servers/claude-doc-optimizer`
3. Check settings.json path: `cat ~/.claude/settings.json | jq '.mcpServers'`
4. Restart Claude Code completely

### Settings.json Validation Error

**Problem**: Invalid JSON in settings

**Solution**:
```bash
jq empty ~/.claude/settings.json
# Shows syntax error if any
```

**Fix**:
```bash
# Restore backup
cp ~/.claude/settings.json.backup ~/.claude/settings.json
./scripts/install.sh --merge-settings
```

### Cache Directory Permission Error

**Problem**: Cannot write to cache directory

**Solutions**:
1. Check permissions: `ls -la ~/.cache/claude-doc-optimizer`
2. Fix permissions: `chmod 755 ~/.cache/claude-doc-optimizer`
3. Change cache location: `CACHE_DIR=/tmp/cache ./scripts/preload-docs.sh`

### Framework Documents Not Found

**Problem**: Preload script reports missing documents

**Solutions**:
1. Verify docs root: `echo $CLAUDE_DOCS_ROOT`
2. Check documents exist: `ls ~/xml/frameworks/`
3. Set correct path: `CLAUDE_DOCS_ROOT=/custom/path ./scripts/preload-docs.sh --list`

### Slow Query Performance

**Problem**: XPath queries take >10ms

**Possible causes**:
- First query (cold cache) - normal, ~10ms
- Cache expired (>24 hours) - reload with `preload_documents`
- Large document - normal for first parse
- Disk I/O bottleneck - check disk space and speed

**Solution**:
```bash
# Force preload
./scripts/preload-docs.sh

# Verify cache hit rate
cache_stats --show-metrics
# Look for >95% hit rate
```

### SessionStart Hook Not Running

**Problem**: Preload script not executed on Claude Code startup

**Causes**:
- Hook disabled in settings.json
- Timeout too short (default 10s should be enough)
- Script not executable
- Claude Code not fully restarted

**Solutions**:
1. Verify hook enabled: `cat ~/.claude/settings.json | jq '.hooks'`
2. Increase timeout: `"timeout": 30000` (30 seconds)
3. Make executable: `chmod +x ~/.claude/scripts/preload-docs.sh`
4. Force restart: Close Claude Code completely, reopen

## Performance Monitoring

### View Tool Usage Logs

```bash
# Last 10 queries
tail -10 ~/.cache/claude-doc-optimizer/logs/session.jsonl

# Watch live logs
tail -f ~/.cache/claude-doc-optimizer/logs/session.jsonl

# Parse JSON logs with jq
jq '.' ~/.cache/claude-doc-optimizer/logs/session.jsonl | grep "cache_hit.*true" | wc -l
```

### Analyze Cache Hit Rate

```bash
cat ~/.cache/claude-doc-optimizer/logs/metrics.json | jq '.cache.hit_rate'
# Expected: >0.95 (95% hits after first query)
```

### Monitor Startup Time

```bash
# Time first query (cold cache)
time xpath_query --document uce34 --xpath '//framework'

# Time second query (warm cache)
time xpath_query --document uce34 --xpath '//framework'
# Expected: <100μs
```

## Maintenance

### Regular Maintenance Schedule

**Daily**:
- Monitor cache hit rate: `cache_stats --show-metrics`
- Check error logs: `tail ~/.cache/claude-doc-optimizer/logs/*.log`

**Weekly**:
- Rotate logs: `rm ~/.cache/claude-doc-optimizer/logs/daily-*.jsonl.gz`
- Validate cache: `cache_stats --validate`
- Check disk usage: `du -sh ~/.cache/claude-doc-optimizer`

**Monthly**:
- Clear old cache: `find ~/.cache/claude-doc-optimizer -mtime +30 -delete`
- Update documentation: `CLAUDE_DOCS_ROOT=/home/samuel ./scripts/preload-docs.sh`
- Run full validation: `./scripts/validate.sh`

### Update Installation

When updating atomic_mcp_server:

```bash
# Recompile
cargo build --release --bin mcp_debug_server --features "std,json-rpc,runtime,stdio-transport,tool-executor"

# Reinstall
./scripts/install.sh

# Validate
./scripts/validate.sh

# Restart Claude Code
```

### Backup Configuration

```bash
# Backup all settings
mkdir -p ~/.claude-backup
cp -r ~/.claude/* ~/.claude-backup/
cp -r ~/.cache/claude-doc-optimizer ~/.claude-backup/cache/

# Restore if needed
cp -r ~/.claude-backup/* ~/.claude/
```

## Uninstall

To remove the installation:

```bash
# Remove MCP server
rm -rf ~/.claude/mcp-servers/claude-doc-optimizer
rm -rf ~/.claude/skills/framework-query
rm -rf ~/.claude/scripts/{preload-docs,log-tool-use}.sh

# Remove cache
rm -rf ~/.cache/claude-doc-optimizer

# Remove settings (backup first!)
rm ~/.claude/settings.json

# Restart Claude Code
```

## Architecture

### Component Overview

```
Claude Code
    ↓
MCP Server (stdio transport)
    ├── SessionStart Hook
    │   └── preload-docs.sh (framework document preload)
    ├── PostToolUse Hook
    │   └── log-tool-use.sh (analytics logging)
    └── Framework Query Skill
        └── xpath_query tool (100,000× speedup via cache)

Cache Layer (T5 Streaming + T9 Persistent)
    ├── In-Memory Cache (lockfree, <100ns lookup)
    ├── Disk Cache (24-hour TTL)
    └── Metrics (JSON, queryable)

Validation Layer
    ├── JSON schema validation
    ├── XML document validation
    └── Performance baselines
```

### Performance Characteristics

| Operation | Latency | Notes |
|-----------|---------|-------|
| First query (cold) | ~10ms | Parse + disk cache |
| Subsequent queries | <100ns | Lockfree memory lookup |
| Tool dispatch | <100ns | Atomic registry |
| Preload (all docs) | ~100ms | Full parse + cache |
| Cache validation | <1ms | Checksum verification |

### Tier Composition

- **T1**: Atomic (rate limiting, cache coordination)
- **T5**: Streaming (XML parsing, incremental results)
- **T9**: Persistent (disk-backed cache with TTL)
- **T6**: Mixed (overall MCP orchestration)

## Support

### Common Questions

**Q: How much disk space is needed?**
A: ~100-500 MB depending on framework size and cache settings

**Q: Can I use a different cache directory?**
A: Yes, set `CACHE_DIR=/custom/path` before running scripts

**Q: What happens if cache directory fills up?**
A: Old entries are evicted (TTL-based, 24 hours default)

**Q: Is the cache persistent?**
A: Yes, survives Claude Code restarts. TTL is 24 hours.

**Q: Can I preload custom documents?**
A: Yes, edit framework list in preload-docs.sh

**Q: How do I disable caching?**
A: `cache_stats --clear && xpath_query --cache-policy disable`

### Getting Help

1. **Check logs**: `tail -100 ~/.cache/claude-doc-optimizer/logs/session.jsonl`
2. **Validate setup**: `./scripts/validate.sh --verbose`
3. **Test manually**: `xpath_query --document uce34 --xpath '//framework'`
4. **Review config**: `cat ~/.claude/settings.json | jq '.'`

## References

- **UCE34 Framework**: `/home/samuel/xml/frameworks/uce34.xml`
- **CLAUDE.md**: `/home/samuel/CLAUDE.md` (v6.0)
- **MCP Protocol**: https://modelcontextprotocol.io/
- **XPath Queries**: https://www.w3.org/TR/xpath20/

## Version History

### v1.0.0 (Nov 2025)

- Initial release
- MCP server with <10μs latency
- XPath cache with 100,000× speedup
- SessionStart preloading
- Framework query skill
- Comprehensive validation suite

## License

All code is protected under COCA (Computational Capsule) architecture. See `/home/samuel/Docs/The Computational Capsule.md` for details.
