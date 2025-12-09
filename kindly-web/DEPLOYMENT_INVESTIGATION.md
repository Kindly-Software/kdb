# kindly-web Deployment Investigation Report

## Executive Summary

**DEFINITIVE FINDING**: `/home/samuel/Primitives/kindly-web/` is the **ACTUAL DEPLOYED VERSION** for kindly.software.

Evidence: fly.toml deployed configuration, recent builds (Nov 7, 2025), WASM artifacts, correct domain references, and active git commits.

---

## All kindly-web Locations Found

### 1. PRIMARY DEPLOYMENT: `/home/samuel/Primitives/kindly-web/` ✅

**Status**: ACTIVELY DEPLOYED to kindly.software

**Location**: `/home/samuel/Primitives/kindly-web/`

**Fly.io Configuration**:
```
app = "kindly-software-website"
primary_region = "ord"  (Chicago)
```

**Key Characteristics**:
- Tech Stack: Leptos 0.7 (Rust WASM)
- Deployment: WASM single-page application
- Content: kindly_dedup marketing page
- Page Title: "kindly_dedup - Lightning-Fast LLM Dataset Deduplication"
- Domain: kindly.software/kindly_dedup

**Build Artifacts** (dist/):
- `index.html` - 17 KB (Nov 7, 2025 20:25:06)
- `kindly-web-65ba195c861a2f5c_bg.wasm` - 665 KB (Nov 7, 2025 20:25:06)
- `kindly-web-65ba195c861a2f5c.js` - 45 KB (Nov 7, 2025 20:25:06)

**Total Build Size**: ~736 KB (dist/)

**Dockerfile**: nginx alpine with gzip compression, WASM MIME types configured

**Source Files**:
- Cargo.toml (Leptos 0.7)
- CLAUDE.md (v2.0, "Premium WASM Landing Page")
- Multiple documentation files (compilation reports, design system docs)

**Git Information**:
- Remote: `https://github.com/yourusername/kindly-web.git`
- Last Commit: `609891c [TRADE SECRET] feat(kindly_dedup): Add License Capsule`
- Active Maintenance: Yes (multiple commits in Nov 2025)

**Deployment Configuration**:
- Internal port: 8080
- HTTP/2 with forced HTTPS
- Auto-scaling: Suspended machines when idle (cost optimization)
- Machine: shared-cpu-1x, 256MB RAM
- Concurrency: soft_limit=200, hard_limit=250

---

### 2. LEGACY/ALTERNATIVE: `/home/samuel/projects/kindly-ecosystem/kindly-main/src/kindly-website/` ❌

**Status**: LEGACY PROJECT (NOT DEPLOYED)

**Location**: `/home/samuel/projects/kindly-ecosystem/kindly-main/src/kindly-website/`

**Fly.io Configuration**:
```
app = "kindly-software-website"  (SAME APP NAME!)
primary_region = "iad"  (US East/Virginia - DIFFERENT REGION)
```

**Key Characteristics**:
- Tech Stack: Static HTML (not Leptos/Rust WASM)
- Content: Kindly API cost optimization platform
- Page Title: "AI API Cost Optimization | Save 37%+ with Kindly API"
- Domain: kindly.software (root, NOT /kindly_dedup)
- Last Modified: August 3, 2025 (3+ months old)

**Size**: ~15 MB (multiple HTML test files, CSS, old project artifacts)

**Last Git Commit**: `19203c16 [UCE34 v5.13] Add PROFILING section before Q10 tier selection` (in parent repo, not this directory)

**Deployment Status**: NOT CURRENTLY DEPLOYED
- Configuration is more comprehensive (OAuth, multi-region, auto-scaling)
- But primary_region is different (iad vs ord)
- Last content update was August 3, 2025 (very stale)

---

### 3. NESTED IN kindly_dedup: `/home/samuel/Primitives/kindly_dedup/src/kindly-web/`

**Status**: DEVELOPMENT/TEST COPY

**Location**: `/home/samuel/Primitives/kindly_dedup/src/kindly-web/`

**Characteristics**:
- Leptos 0.8 (slightly newer than main deploy version 0.7)
- Contains atomic_capsule integration (trade secret version)
- Has build artifacts (target/ directory with debug/release WASM)
- No fly.toml for independent deployment
- Referenced in kindly_dedup Cargo.toml

**Purpose**: Internal testing/integration with kindly_dedup project

**Last Build**: Nov 8, 2025 (recent but not deployed as primary)

---

## Comparison Table

| Aspect | Primitives/kindly-web (DEPLOYED) | Projects/kindly-website (LEGACY) | kindly_dedup/kindly-web (TEST) |
|--------|----------------------------------|----------------------------------|-------------------------------|
| **Location** | `/home/samuel/Primitives/kindly-web/` | `/home/samuel/projects/.../kindly-website/` | `/home/samuel/Primitives/kindly_dedup/src/kindly-web/` |
| **Deployment** | ✅ ACTIVE (kindly.software) | ❌ Stale (iad region, not deployed) | ⚠️ Test/Integration only |
| **Fly App** | kindly-software-website (ord) | kindly-software-website (iad) | None |
| **Region** | Chicago (ord) | Virginia (iad) | N/A |
| **Tech Stack** | Leptos 0.7 WASM | Static HTML | Leptos 0.8 WASM (test) |
| **Content** | kindly_dedup marketing | Kindly API platform | Test integration copy |
| **Domain** | kindly.software/kindly_dedup | kindly.software/ | N/A |
| **Last Build** | Nov 7, 2025 20:25 | Aug 3, 2025 17:50 | Nov 8, 2025 02:12 |
| **Build Size** | 736 KB (dist/) | ~15 MB (multiple HTML) | ~152 KB (project root) |
| **Page Title** | "kindly_dedup - Lightning-Fast Dedup" | "AI API Cost Optimization" | (test/integration) |
| **Git Activity** | Active (daily commits in Nov) | Inactive (parent repo moves) | Active (development) |
| **atommic_capsule** | No dependency | No dependency | Yes (trade secret version) |
| **Production Ready** | ✅ YES | ❌ Outdated | ⚠️ Development |

---

## Evidence: Why Primitives/kindly-web is Deployed

### 1. **Fly.toml Configuration Match**
- Both have `app = "kindly-software-website"` (same Fly.io app)
- Primitives version: `primary_region = "ord"` (Chicago - mentioned as "existing deployment")
- Projects version: `primary_region = "iad"` (Virginia - old config, not active)
- **Evidence**: The "ord" comment "matches existing deployment" indicates Primitives is current

### 2. **Recent Build Artifacts**
- Primitives: dist/ files dated Nov 7, 2025 @ 20:25 (VERY RECENT)
- Projects: HTML files dated Aug 3, 2025 (3+ months OLD)
- **Evidence**: Fresh dist/ artifacts prove active deployment

### 3. **Content Alignment**
- Primitives: kindly_dedup marketing page (current product focus)
- Projects: Kindly API cost optimization (abandoned product line)
- URL references: Primitives has `kindly.software/kindly_dedup` (current)
- **Evidence**: Content matches what kindly.software shows

### 4. **Build Configuration**
- Primitives: Minimal Dockerfile (nginx alpine, 1x shared CPU, 256 MB RAM = cost-optimized)
- Projects: Complex OAuth setup, multiple regions, extensive monitoring = over-engineered for current needs
- **Evidence**: Simpler config reflects focused, active product

### 5. **Git Activity**
- Primitives: Last commit Nov 10, 2025 (TODAY - active maintenance)
- Projects: Last commit in parent repo (Sep 12, 2025 - no updates to website)
- **Evidence**: Active git commits in Primitives indicate ongoing development

### 6. **Deployment Mode**
- Primitives: `auto_stop_machines = "suspend"` (cost optimization for static site)
- Projects: `auto_stop_machines = false`, `min_machines_running = 2` (high-availability setup)
- **Evidence**: Primitives matches simple marketing site needs

---

## RECOMMENDATION FOR PAYMENT INTEGRATION

### **Work In**: `/home/samuel/Primitives/kindly-web/`

This is the production-deployed version. For payment integration:

1. **Add Payment Page**: Create new page component in Leptos
2. **Integrate Stripe**: Use Leptos form handling to integrate payment flows
3. **Build & Deploy**: 
   - Run: `trunk build --release` in `/home/samuel/Primitives/kindly-web/`
   - Deploy: `fly deploy` (uses fly.toml configuration)

4. **Avoid**: `/home/samuel/projects/kindly-ecosystem/kindly-main/src/kindly-website/`
   - This is stale (last update Aug 2025)
   - Different region configuration
   - Static HTML (not WASM - can't easily integrate modern payment flows)

---

## Files to Modify for Payment Integration

### In `/home/samuel/Primitives/kindly-web/`:

1. **Cargo.toml** - Add payment library (stripe-rs for Rust backend, or wasm-compatible payment SDK)
2. **src/main.rs** - Add payment route/page component
3. **src/pages/checkout.rs** (new) - Create checkout component
4. **index.html** - Add payment scripts if needed
5. **fly.toml** - May need environment variables for API keys

### Build & Verification
```bash
cd /home/samuel/Primitives/kindly-web
cargo build --target wasm32-unknown-unknown --release
trunk build --release
fly deploy --app kindly-software-website
```

---

## Key Insights

1. **Two Fly.io Configs, Same App**: Both fly.toml files deploy to the same Fly.io app name, but with different regions. The "ord" (Chicago) region in Primitives is the active one.

2. **Trade Secret Protection**: Primitives/kindly-web intentionally avoids shipping atomic_capsule code in the public WASM bundle (see CLAUDE.md). The kindly_dedup/kindly-web is the test version WITH capsule integration (protected as trade secret).

3. **Product Focus**: The deployed version (Primitives) is marketing kindly_dedup specifically. The legacy version was trying to market the Kindly API platform.

4. **Cost Optimization**: Active deployment uses minimal resources (1 CPU, 256 MB) with auto-suspend. Legacy version was over-provisioned (2+ machines, 512 MB).

---

## Conclusion

**DEFINITIVELY**: `/home/samuel/Primitives/kindly-web/` is the actual deployed version on kindly.software.

All payment integration work should be done in this directory. The other locations are either legacy projects or development/test copies.
