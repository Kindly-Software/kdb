# CCPM Installer - Rust CLI Tool

**Automatically install CCPM (Claude Code Project Manager) across all your Rust projects.**

## 🚀 Quick Start

```bash
# Build and install
cd ~/Primitives/tools/ccpm-installer
cargo build --release
cp target/release/ccpm-install ~/bin/

# List all projects
ccpm-install list

# Dry run (see what would be installed)
ccpm-install install-all --dry-run

# Install CCPM in all projects (skip those that already have .claude/)
ccpm-install install-all --skip-existing

# Install in all projects (merge with existing .claude/)
ccpm-install install-all

# Install in specific project
ccpm-install install ~/Primitives/atomic_capsule
```

---

## 📋 Commands

### `list` - List All Projects

Scans for Rust projects (directories with `Cargo.toml`) and shows CCPM status.

```bash
# Default: ~/Primitives
ccpm-install list

# Custom directory
ccpm-install list --root ~/my-projects
```

**Output**:
```
Found 75 projects:
  atomic_capsule - ✓ CCPM installed
  kindly_dedup - ✓ CCPM installed
  ai_image_detector - ○ No CCPM
  ...
```

---

### `install-all` - Install in All Projects

Scans and installs CCPM in all detected Rust projects.

```bash
# Dry run (show what would happen)
ccpm-install install-all --dry-run

# Install, skip projects with existing .claude/
ccpm-install install-all --skip-existing

# Install, merge with existing .claude/ directories
ccpm-install install-all

# Custom root directory
ccpm-install install-all --root ~/my-projects --skip-existing
```

**Options**:
- `--dry-run` - Show what would be installed without making changes
- `--skip-existing` - Skip projects that already have `.claude/` directory
- `--root <DIR>` - Root directory to scan (default: `~/Primitives`)

**Example Output**:
```
🔍 Scanning for projects in: /home/samuel/Primitives
Found 75 projects

📦 Downloading CCPM from GitHub...
✓ CCPM downloaded to: /tmp/.tmp3xYZ/ccpm

  ✓ atomic_capsule
  ✓ kindly_dedup
  ⏭ atomic_hedge_capsule (skipped)
  ✓ ai_image_detector
  ...

Summary:
  Installed: 60
  Skipped: 15

Next steps:
  For each project, run in Claude Code:
    /pm:init
```

---

### `install` - Install in Specific Project

Install CCPM in a single project.

```bash
# Install in specific project
ccpm-install install ~/Primitives/atomic_capsule

# Force install (overwrite existing .claude/)
ccpm-install install ~/Primitives/atomic_capsule --force
```

**Options**:
- `--force` - Overwrite existing `.claude/` directory

---

### `verify` - Verify Installation

Check if CCPM is properly installed in a project.

```bash
ccpm-install verify ~/Primitives/atomic_capsule
```

**Output**:
```
🔍 Verifying CCPM in: /home/samuel/Primitives/atomic_capsule
  ✓ agents/
  ✓ commands/
  ✓ context/
  ✓ prds/
  ✓ epics/

✓ CCPM installation verified!
```

---

### `download` - Download CCPM Repository

Download CCPM repository to a specific location (for manual installation or inspection).

```bash
# Default: /tmp/ccpm-master
ccpm-install download

# Custom location
ccpm-install download --output ~/ccpm-backup
```

---

## 🎯 Typical Workflows

### Workflow 1: Install CCPM in All Projects (First Time)

```bash
# 1. See what would be installed
ccpm-install install-all --dry-run

# 2. Install in all projects
ccpm-install install-all

# 3. For each project, initialize in Claude Code
# cd ~/Primitives/atomic_capsule
# claude
# /pm:init
```

### Workflow 2: Add CCPM to New Projects Only

```bash
# Skip projects that already have CCPM
ccpm-install install-all --skip-existing
```

### Workflow 3: Install in One Project

```bash
# Single project install
ccpm-install install ~/Primitives/my-new-project

# Then in Claude Code:
# /pm:init
```

### Workflow 4: Verify Multiple Installations

```bash
# List all projects and their CCPM status
ccpm-install list
```

---

## 🛠️ How It Works

1. **Scans** for Rust projects (directories containing `Cargo.toml`)
2. **Downloads** CCPM from https://github.com/automazeio/ccpm
3. **Copies** `.claude/` directory into each project
4. **Skips** `target/` directories and hidden directories

### Directory Structure Created

```
your-project/
├── Cargo.toml
├── src/
└── .claude/                 ← Installed by ccpm-install
    ├── agents/
    ├── commands/
    ├── context/
    ├── prds/
    ├── epics/
    └── ...
```

---

## 📊 Projects Detected

The tool finds **75 projects** in `~/Primitives`:

**Already have CCPM**:
- Primitives (root)
- atomic_capsule
- kindly_dedup
- kindly_hft
- kindly-web
- ... (15 total)

**Would be installed** (60 projects):
- atomic_breaker, atomic_capsule_derive, ai_image_detector
- kindly_bench, kindly_dash, kindly_inference
- fqbit, kiang, clapi_core
- ... and 51 more

---

## 🔧 Building from Source

```bash
cd ~/Primitives/tools/ccpm-installer
cargo build --release
cp target/release/ccpm-install ~/bin/
```

### Dependencies

- `clap` - CLI argument parsing
- `walkdir` - Directory traversal
- `colored` - Terminal colors
- `anyhow` - Error handling
- `reqwest` - HTTP (for potential future use)
- `tempfile` - Temporary directories
- `serde` + `serde_json` - JSON handling

---

## ⚙️ Configuration

### Scan Depth

Currently scans **3 levels deep** from root directory. Modify `max_depth(3)` in `find_projects()` if needed.

### Excluded Directories

Automatically skips:
- `target/` directories
- Hidden directories (starting with `.`)

---

## 🐛 Troubleshooting

### "Directory does not exist"
Check the path:
```bash
ccpm-install list --root /correct/path
```

### "Git clone failed"
Ensure `git` is installed:
```bash
git --version
```

### ".claude directory already exists"
Use `--force` to overwrite or `--skip-existing` to skip:
```bash
ccpm-install install ~/path/to/project --force
```

### "Permission denied"
Ensure binary is executable:
```bash
chmod +x ~/bin/ccpm-install
```

---

## 📚 After Installation

Once CCPM is installed in your projects:

1. **Navigate to project**:
   ```bash
   cd ~/Primitives/atomic_capsule
   ```

2. **Open Claude Code**:
   ```bash
   claude
   ```

3. **Initialize CCPM**:
   ```
   /pm:init
   ```

4. **Start using CCPM**:
   ```
   /pm:prd-new my-feature
   /pm:prd-parse my-feature
   /pm:epic-oneshot my-feature
   /pm:issue-start 123
   ```

---

## 🎯 See Also

- **CCPM GitHub**: https://github.com/automazeio/ccpm
- **CCPM Quick Start**: `~/CCPM_QUICKSTART.md`
- **CCPM Installation Summary**: `~/CCPM_INSTALLATION_SUMMARY.md`
- **tmux Setup**: `~/TMUX_CLAUDE_QUICKSTART.md`

---

## 📝 License

MIT (matches CCPM license)

---

## 🤝 Contributing

This is a tool for automating CCPM installation across multiple projects. To contribute:

1. Test on your projects
2. Report issues
3. Submit PRs for improvements

---

**Happy vibing with CCPM across all your projects!** 🎸
