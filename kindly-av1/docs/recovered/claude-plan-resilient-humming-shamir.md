# kindly_term: IDE/Agentic Coding Tool Readiness Assessment

## Executive Summary

**Goal**: Assess kindly_term's readiness to build an IDE/agentic coding tool.

**Current State**: 22,789 LOC across 71 files with **55-65% infrastructure complete**
- ✅ Excellent lockfree Chaos foundation (100% mutex-free)
- ✅ Strong async runtime (io_uring, work-stealing, <500ns process spawn)
- ✅ Full network stack (HTTP/1/2/3, QUIC, WebSocket, JSON-RPC)
- 🔴 GPU rendering pipeline is **stubs only** (no actual kgpu integration)
- 🔴 Complex widgets have **empty render methods**
- 🔴 No text buffer (Rope), no LSP client, no syntax highlighting

**Verdict**: NOT IDE-ready. Requires **6-10 weeks** to reach minimum viable IDE.

---

## Completed (Phases 1-5 Terminal Architecture)

- ✅ **Phase 1**: Terminal Foundation (13 capsules, 11,191 LOC)
- ✅ **Phase 2**: GPU Pipeline Architecture (16 capsules) - **STUBS ONLY**
- ✅ **Phase 3**: Widget System (27 capsules) - **RENDER METHODS EMPTY**
- ✅ **Phase 4**: kindly-CSS Styling (8 capsules)
- ✅ **Phase 5**: Capsule-OS Integration (2 capsules)

---

## Detailed Gap Analysis

### Layer 1: What WORKS (✅ Production Ready)

| Component | Status | Evidence | Location |
|-----------|--------|----------|----------|
| **Event System** | ✅ | Keyboard, mouse, resize events fully implemented | `terminal/event/` |
| **ANSI Parser** | ✅ | SIMD-accelerated (T2), 2-8× speedup | `terminal/parser/` |
| **Async Runtime** | ✅ | io_uring, work-stealing, <500ns spawn | `runtime/` |
| **Process Management** | ✅ | PTY, signals, job control | `runtime/process.rs` |
| **Network Stack** | ✅ | HTTP/1/2/3, QUIC, WebSocket, JSON-RPC | `http/`, `quic/` |
| **File Navigation** | ✅ | FileNavigatorCapsule with tree traversal | `tui/file_navigator.rs` |
| **TextInputCapsule** | ✅ | 128-byte input, undo (8-state ring), UTF-8 | `widget/foundation/text_input.rs` |
| **Theme System** | ✅ | 24 semantic colors, 4 built-in themes | `terminal/style/theme.rs` |

### Layer 2: What's STUBBED (🟠 Architecture Only)

| Component | Problem | Evidence | Fix Effort |
|-----------|---------|----------|------------|
| **GPU Rendering** | No kgpu connection, no shaders compiled | `render/pipeline.rs` has state machine but no draw calls | 2-3 weeks |
| **TreeCapsule render()** | Empty method, no line drawing | `widget/complex/tree.rs` line ~200: `// TODO` | 2-3 days |
| **ListCapsule render()** | State tracking OK, no visual output | `widget/complex/list.rs` | 2-3 days |
| **TabsCapsule** | Placeholder types only | `widget/mod.rs` line 52-56 | 1 week |
| **TableCapsule render()** | Column logic OK, no rendering | `widget/complex/table.rs` | 3-4 days |
| **ScrollCapsule scrollbar** | Scroll state works, no scrollbar visual | Line 124: `// TODO scrollbar` | 1-2 days |

### Layer 3: What's MISSING (🔴 Not Implemented)

| Component | Why Critical for IDE | Tier | Effort |
|-----------|---------------------|------|--------|
| **Text Buffer (Rope)** | Can't edit 100K+ line files | T4 Batch | 2-3 weeks |
| **Multi-line Editor** | TextInputCapsule max 128 bytes | T1+T5 | 2 weeks |
| **LSP Client** | No code intelligence (autocomplete, diagnostics) | T8 Network | 2-3 weeks |
| **Syntax Highlighting** | No treesitter, no tokenizer | T2 SIMD | 2 weeks |
| **Chat Panel** | No AI conversation UI | T5 Streaming | 1-2 weeks |
| **Inline Suggestions** | No ghost text/AI suggestions | T1 | 1 week |
| **Diff Renderer** | No side-by-side or unified diffs | T2+T4 | 2 weeks |
| **Command Palette** | No quick command search | T1+T5 | 1 week |
| **Minimap** | No code thumbnail | T7 GPU | 1 week |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    kindly_term Architecture                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                kindly-CSS Styling Layer                   │   │
│  │  StyleSheetCapsule | ThemeCapsule | ComputedStyleCapsule │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              ↓                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                   Widget System Layer                     │   │
│  │  Button | Input | List | Tree | Modal | Tabs | Dropdown  │   │
│  │  LayoutSolver | FocusManager | AnimationCapsule          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              ↓                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                 GPU Rendering Pipeline                    │   │
│  │  GlyphAtlas | CellBuffer | ShaderPipeline | FrameSync   │   │
│  │  Backend: kgpu (Vulkan/Metal/DX12) | WASM (WebGPU)      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              ↓                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Terminal Foundation (Phase 1 ✅)             │   │
│  │  Events | Parser | Mode | Output | Platform | Signal     │   │
│  │  TerminalMetacapsule (T6, 1024B) - 13 capsules done     │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 2: GPU Rendering Pipeline

**Goal**: GPU-native terminal rendering with custom shaders on kgpu

### Architecture: Hybrid Atlas + Compute

| Component | Approach | Rationale |
|-----------|----------|-----------|
| **Text Rendering** | Glyph Atlas (95%) | Instanced quads, 2 draw calls/frame |
| **Effects** | Compute Shaders | Post-processing, ligatures, CRT effects |
| **Sync** | DualAtomicU64 | Lockfree CPU-GPU coordination |

### GPU Capsule Inventory (16 total)

**New Capsules (8)**:

| Capsule | Tier | Size | Purpose |
|---------|------|------|---------|
| TerminalAtlasCapsule | T7 | 512B | 4K glyph texture atlas with subpixel variants |
| TerminalCellBufferCapsule | T1+T4 | 256B | Double-buffered cell grid (80×24 → 4K cells) |
| GlyphCacheCapsule | T4+T5 | 512B | LRU glyph cache, batch rasterization |
| GpuFrameSyncCapsule | T1 | 128B | Fence-based frame synchronization |
| TerminalPipelineManager | T6 | 256B | Pipeline state coordination |
| TerminalRenderPassCapsule | T6 | 512B | Render pass orchestration |
| TerminalUniformsCapsule | T1 | 64B | Per-frame uniform buffer |
| TerminalEffectsCapsule | T7 | 256B | Post-processing (CRT, bloom) |

**Reused kgpu Capsules (8)**:
- KgpuTextureCapsule, KgpuBufferCapsule, KgpuRenderPipelineCapsule
- KgpuComputePipelineCapsule, KgpuShaderCacheCapsule, KgpuBindGroupCapsule
- KgpuRenderPassCapsule, RenderBufferCapsule

### Shader Architecture

```
┌─────────────────────────────────────────────────┐
│           Terminal Shader Pipeline              │
├─────────────────────────────────────────────────┤
│                                                  │
│  ┌────────────────┐   ┌────────────────┐        │
│  │ Vertex Shader  │ → │ Fragment Shader│        │
│  │ (quad_vert)    │   │ (glyph_frag)   │        │
│  │ - Instance pos │   │ - Atlas sample │        │
│  │ - Cell coords  │   │ - FG/BG color  │        │
│  │ - UV mapping   │   │ - Bold/Italic  │        │
│  └────────────────┘   │ - Underline    │        │
│                       │ - Strikethrough│        │
│                       └────────────────┘        │
│                              ↓                  │
│  ┌────────────────────────────────────────┐    │
│  │        Compute Shader (Optional)       │    │
│  │ - CRT scanlines  - Bloom effect        │    │
│  │ - Color grading  - Ligature rendering  │    │
│  └────────────────────────────────────────┘    │
│                                                  │
└─────────────────────────────────────────────────┘
```

### Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Frame time | <16ms | 60 FPS guaranteed |
| Cell update | <100ns | Single atomic write |
| Buffer swap | <10ns | DualAtomicU64 swap |
| Atlas lookup | <50ns | O(1) hash |
| GPU memory | ~20MB | Atlas + double-buffer |

---

## Phase 3: Widget System

**Goal**: GUI-quality widget system with 100% lockfree Chaos architecture

### Widget Trait Design

```rust
/// Core widget trait - NO Box<dyn>, typed composition
pub trait Widget: Send + Sync + Sized + 'static {
    type State: Copy + Default;
    const TYPE_ID: u64;

    fn layout(&self, bounds: Rect, state: &WidgetStateCapsule<Self::State>) -> Rect;
    fn handle_event(&self, event: &Event, state: &WidgetStateCapsule<Self::State>) -> bool;
    fn render(&self, area: Rect, state: &WidgetStateCapsule<Self::State>, cmd: &mut RenderCommandBuffer);

    fn focusable(&self) -> bool { false }
    fn tab_index(&self) -> u16 { 0 }
}
```

### Widget Capsule Inventory (27 total over 3 phases)

**Foundation Widgets (Phase 3.1 - 8 capsules)**:

| Widget | Tier | Size | Features |
|--------|------|------|----------|
| ButtonCapsule | T1+T3 | 256B | Press animation, ripple effect |
| TextInputCapsule | T1+T5 | 512B | Cursor blink, selection, streaming |
| LabelCapsule | T1 | 128B | Static text, truncation |
| CheckboxCapsule | T1+T3 | 128B | Toggle animation, tristate |
| RadioCapsule | T1+T3 | 128B | Group coordination |
| ProgressCapsule | T1+T3 | 128B | Smooth animation, indeterminate |
| SpacerCapsule | T0 | 64B | Flex grow layout |
| DividerCapsule | T0 | 64B | Visual separator |

**Container Widgets (Phase 3.2 - 6 capsules)**:

| Widget | Tier | Size | Features |
|--------|------|------|----------|
| FlexContainerCapsule | T4+T6 | 1024B | Flexbox layout, constraint solver |
| GridContainerCapsule | T4+T6 | 1024B | CSS Grid, auto-placement |
| ScrollContainerCapsule | T1+T5 | 512B | Momentum scrolling, virtualization |
| ModalContainerCapsule | T1 | 256B | Focus trap, backdrop |
| PanelCapsule | T1 | 256B | Borders, shadows, background |
| SplitPaneCapsule | T1+T3 | 256B | Resizable split |

**Complex Widgets (Phase 3.3 - 8 capsules)**:

| Widget | Tier | Size | Features |
|--------|------|------|----------|
| DropdownCapsule | T1+T5 | 512B | Popup overlay, search filter |
| ListCapsule | T4+T5 | 512B | Virtual scrolling, 100K+ items |
| TreeCapsule | T4+T5 | 512B | Expand/collapse, lazy loading |
| TableCapsule | T4+T5 | 1024B | Sort, filter, column resize |
| TabsCapsule | T1+T3 | 256B | Animated tab indicator |
| TooltipCapsule | T1+T3 | 128B | Delay, positioning |
| MenuCapsule | T1+T5 | 512B | Submenus, keyboard nav |
| SliderCapsule | T1+T3 | 128B | Continuous/discrete, dual thumb |

**Infrastructure Capsules (5)**:

| Capsule | Tier | Size | Purpose |
|---------|------|------|---------|
| WidgetTreeCapsule | T4 | 4KB | Arena allocation, 60 nodes |
| LayoutSolverCapsule | T4+T6 | 512B | Constraint solver, SIMD-accelerated |
| FocusManagerCapsule | T1 | 256B | Tab navigation, hotkeys |
| AnimationCapsule | T3+T4 | 1024B | Q16.16 easing, 60 slots |
| RenderCommandBuffer | T5 | 4KB | GPU command stream |

### Layout Algorithm (3-Phase)

1. **Measure** (T4 Batch): Collect min/preferred/max sizes in parallel
2. **Solve** (T2 SIMD): Distribute space via flex factors, <500μs for 100 widgets
3. **Apply** (T4 Batch): Write final positions to widget state capsules

---

## Phase 4: kindly-CSS Styling System

**Goal**: CSS-like syntax that compiles to ANSI AND GPU uniforms

### CSS Property Specification

**Core Properties**:

| Property | ANSI | GPU Uniform |
|----------|------|-------------|
| `color` | SGR 38;2;R;G;B | `vec4 u_fg_color` |
| `background-color` | SGR 48;2;R;G;B | `vec4 u_bg_color` |
| `font-weight` | SGR 1 (bold) | `float u_font_weight` |
| `font-style` | SGR 3 (italic) | `float u_font_style` |
| `text-decoration` | SGR 4/9 | `uint u_decoration_flags` |
| `padding` | Space chars | `vec4 u_padding` |
| `border` | Box-drawing | SDF lines |
| `border-radius` | Arc chars | SDF rounded rect |
| `box-shadow` | Dithered gradient | Gaussian blur |

### kindly-CSS Syntax Example

```css
@theme "byzantine-dark";

Button {
    background-color: theme.primary;
    color: theme.text-inverse;
    padding: 1 2;
    border: solid theme.primary;
    border-radius: 1;
}

Button:hover {
    background-color: theme.primary-hover;
    box-shadow: 2 2 4 theme.shadow;
}

Button.secondary:disabled {
    opacity: 0.5;
    background-color: theme.muted;
}

Panel {
    background-color: theme.bg-elevated;
    border: solid theme.border-default;
    border-radius: 2;
    box-shadow: 2 2 4 theme.shadow;
}
```

### Styling Capsule Inventory (8 total)

| Capsule | Tier | Size | Purpose |
|---------|------|------|---------|
| StyleSheetCapsule | T0+T1 | 1024B | Parsed rules, compile-time macro |
| ThemeColorsCapsule | T1 | 256B | 24 semantic colors |
| ThemeComponentsCapsule | T1 | 512B | Component-specific colors |
| ComputedStyleCapsule | T3 | 128B | Resolved values (Q8.8) |
| StyleUniformsCapsule | T7 | 256B | GPU shader uniforms |
| StyleCacheCapsule | T1 | 1024B | LRU cache (64 entries) |
| AnimationCapsule | T1+T3 | 128B | CSS transitions |
| IconAtlasCapsule | T7 | 512B | Unicode + custom glyphs |

### Style Resolution Pipeline

```
Parse (T0)  →  Cascade (T1)  →  Compute (T3)  →  Render
   ↓              ↓               ↓              ↓
Compile-time   LRU cache      Q8.8 fixed    ANSI or GPU
 <1ms parse   <100ns lookup   <50ns calc    <100ns emit
```

---

## Module Structure (Future)

```
atomic_capsule/src/terminal/
├── [Phase 1 - DONE] ─────────────────────────────
│   ├── mod.rs, error.rs, metacapsule.rs
│   ├── event/, parser/, mode/, output/, platform/, signal/
│
├── [Phase 2 - GPU] ──────────────────────────────
│   └── render/
│       ├── mod.rs              # Render coordination
│       ├── atlas.rs            # TerminalAtlasCapsule (T7)
│       ├── cell_buffer.rs      # TerminalCellBufferCapsule (T1+T4)
│       ├── glyph_cache.rs      # GlyphCacheCapsule (T4+T5)
│       ├── frame_sync.rs       # GpuFrameSyncCapsule (T1)
│       ├── pipeline.rs         # TerminalPipelineManager (T6)
│       ├── effects.rs          # TerminalEffectsCapsule (T7)
│       └── shaders/
│           ├── quad.vert       # Vertex shader (GLSL)
│           ├── glyph.frag      # Fragment shader (GLSL)
│           └── effects.comp    # Compute shader (GLSL)
│
├── [Phase 3 - Widgets] ──────────────────────────
│   └── widget/
│       ├── mod.rs              # Widget trait, prelude
│       ├── tree.rs             # WidgetTreeCapsule (T4)
│       ├── layout.rs           # LayoutSolverCapsule (T4+T6)
│       ├── focus.rs            # FocusManagerCapsule (T1)
│       ├── animation.rs        # AnimationCapsule (T3+T4)
│       ├── command_buffer.rs   # RenderCommandBuffer (T5)
│       ├── foundation/         # Button, Input, Checkbox, etc.
│       ├── container/          # Flex, Grid, Scroll, Modal
│       └── complex/            # List, Tree, Dropdown, Menu
│
└── [Phase 4 - kindly-CSS] ───────────────────────
    └── style/
        ├── mod.rs              # Public API
        ├── sheet.rs            # StyleSheetCapsule (T0+T1)
        ├── theme.rs            # ThemeColorsCapsule (T1)
        ├── computed.rs         # ComputedStyleCapsule (T3)
        ├── uniforms.rs         # StyleUniformsCapsule (T7)
        ├── cache.rs            # StyleCacheCapsule (T1)
        └── parser.rs           # kindly-css! macro (T0)
```

---

## Implementation Timeline

| Phase | Duration | Capsules | Tests | Deliverable |
|-------|----------|----------|-------|-------------|
| **2: GPU Pipeline** | 4 weeks | 16 | ~200 | 60 FPS GPU rendering |
| **3.1: Foundation Widgets** | 2 weeks | 8 | ~140 | Button, Input, Checkbox |
| **3.2: Containers** | 2 weeks | 6 | ~155 | Flex, Grid, Scroll |
| **3.3: Complex Widgets** | 2 weeks | 8 | ~155 | List, Tree, Dropdown |
| **4: kindly-CSS** | 3 weeks | 8 | ~200 | Full styling system |
| **5: Integration** | 2 weeks | 2 | ~120 | Capsule-OS ready |
| **Total** | ~15 weeks | 51 new | ~970 | GUI-quality terminal |

---

## Performance Targets (B32)

| Layer | Operation | Target |
|-------|-----------|--------|
| **GPU** | Frame render | <16ms (60 FPS) |
| **GPU** | Cell update | <100ns |
| **GPU** | Atlas lookup | <50ns |
| **Widget** | State read | <5ns |
| **Widget** | Layout (100) | <500μs |
| **Widget** | Event dispatch | <200ns |
| **Style** | Rule lookup | <100ns |
| **Style** | Cascade | <500ns |
| **Style** | ANSI emit | <100ns |

---

## Critical Files for Implementation

**GPU Pipeline**:
- `/home/samuel/Primitives/atomic_capsule/src/gpu/kgpu/texture.rs` - Type-state texture
- `/home/samuel/Primitives/atomic_capsule/src/gpu/kgpu/buffer.rs` - Buffer management
- `/home/samuel/Primitives/atomic_capsule/src/gpu/kgpu/pipeline.rs` - Pipeline patterns

**Widget System**:
- `/home/samuel/Primitives/atomic_capsule/src/terminal/metacapsule.rs` - T6 orchestration
- `/home/samuel/Primitives/atomic_capsule/src/patterns/dual_atomic.rs` - State coordination
- `/home/samuel/Primitives/atomic_capsule/src/tui/render_buffer.rs` - Frame timing

**kindly-CSS**:
- `/home/samuel/Primitives/clapi_core/src/tui/colors.rs` - ColorThemeCapsule pattern
- `/home/samuel/Primitives/atomic_capsule/src/tui/screen_state.rs` - SWeMR pattern
- `/home/samuel/Primitives/atomic_capsule/src/primitives/fixed_point.rs` - Q8.8/Q16.16

---

## Framework Compliance

| Framework | Status |
|-----------|--------|
| **UCE34** | T0-T7 multi-tier, Q10 tier selection, Q33 lockfree, Q34 audit |
| **Chaos** | 100% lockfree (no mutex/RwLock), DualAtomicU64, gen counters |
| **ASSUM** | 99.99% safe, all memory ordering documented |
| **B32** | Fair baselines, 95% CI, 1000+ iterations |
| **T28** | 5-tier (Unit/Property/Integration/Production/Determinism) |
| **I20** | 20/20, zero breaking changes to existing capsules |

---

## Feature Flags

```toml
# GPU rendering
terminal-gpu = ["terminal", "kgpu"]
terminal-gpu-vulkan = ["terminal-gpu"]
terminal-gpu-metal = ["terminal-gpu"]
terminal-gpu-wasm = ["terminal-gpu", "wgpu"]

# Widget system
terminal-widgets = ["terminal"]
terminal-widgets-full = ["terminal-widgets", "terminal-gpu"]

# kindly-CSS
terminal-css = ["terminal"]
terminal-css-compile = ["terminal-css"]  # Compile-time styles

# Presets
preset-capsule-os = ["terminal-gpu", "terminal-widgets-full", "terminal-css-compile"]
```

---

## Recommended Roadmap: GPU-Native Agentic IDE

Based on user preferences: **GPU-native rendering** + **Chat with AI first**

### Phase 6: Fix GPU Rendering (Weeks 1-3) 🔴 BLOCKING

**Goal**: Make GPU pipeline functional so widgets can actually render to screen.

| Capsule | Tier | Work Required | Priority |
|---------|------|---------------|----------|
| **TerminalAtlasCapsule** | T7 | Connect to kgpu texture system, font rasterization | P0 |
| **TerminalCellBufferCapsule** | T1+T4 | Double-buffer write path to GPU | P0 |
| **GlyphCacheCapsule** | T4+T5 | Actual LRU with GPU texture upload | P1 |
| **Shader Compilation** | T7 | Compile quad.vert, glyph.frag, effects.comp | P0 |
| **TerminalPipelineManager** | T6 | kgpu draw call submission | P0 |
| **Widget render() methods** | T1 | Implement all stubbed render() in 27 widgets | P1 |

**Critical Files**:
- `src/terminal/render/atlas.rs` - Glyph atlas (currently no texture binding)
- `src/terminal/render/pipeline.rs` - Pipeline (state machine only, no kgpu)
- `src/gpu/kgpu/` - kgpu integration point

**Deliverable**: Text renders to screen at 60 FPS via GPU.

---

### Phase 7: Chat with AI (Weeks 4-5) 🎯 FIRST MILESTONE

**Goal**: Agentic chat panel with streaming responses and inline suggestions.

| Capsule | Tier | Size | Features |
|---------|------|------|----------|
| **ChatPanelCapsule** | T5 Streaming | 1024B | Message list, streaming text, input field |
| **ChatMessageCapsule** | T1 | 256B | Role (user/assistant), content, timestamp |
| **StreamingTextRenderer** | T5 | 512B | Token-by-token display, auto-scroll |
| **InlineSuggestionsCapsule** | T1 | 128B | Ghost text below cursor, accept/reject |
| **AgentStateCapsule** | T1 | 64B | Idle/thinking/streaming/error states |

**Critical Files**:
- `src/collections/async_log.rs` - AsyncLogCapsule (T5, 20-100× speedup) - REUSE
- `src/http/mcp_transport.rs` - MCP transport (JSON-RPC) - REUSE
- `src/terminal/widget/complex/list.rs` - ListCapsule base - EXTEND

**Deliverable**: Can chat with AI, see streaming responses, accept inline suggestions.

---

### Phase 8: Text Editor Core (Weeks 6-8)

**Goal**: Multi-file editing with large file support.

| Capsule | Tier | Size | Purpose |
|---------|------|------|---------|
| **RopeCapsule** | T4 Batch | 512B/file | Piece table for large-file editing |
| **EditorViewportCapsule** | T1+T5 | 256B | Scroll position, visible line range |
| **LineNumberGutterCapsule** | T1 | 128B | Line number rendering |
| **SelectionCapsule** | T1 | 128B | Multi-cursor, block selection |
| **UndoStackCapsule** | T1+T5 | 1024B | Edit history with coalescing |

**Deliverable**: Can open, edit, save 100K+ line files at 60 FPS.

---

### Phase 9: Code Intelligence (Weeks 9-11)

**Goal**: LSP integration for diagnostics and autocomplete.

| Capsule | Tier | Size | Purpose |
|---------|------|------|---------|
| **LspClientCapsule** | T8 Network | 512B | JSON-RPC over stdio/TCP |
| **DiagnosticCapsule** | T1+T4 | 256B | Error/warning storage |
| **DiagnosticRendererCapsule** | T1 | 256B | Gutter icons, inline squiggles |
| **CompletionPopupCapsule** | T1+T5 | 512B | Autocomplete list, fuzzy filter |
| **HoverTooltipCapsule** | T1+T3 | 256B | Type info, documentation |

**Deliverable**: Live error checking, autocomplete, hover info.

---

### Phase 10: Diff & Review (Weeks 12-13)

**Goal**: AI change review workflow.

| Capsule | Tier | Purpose |
|---------|------|---------|
| **DiffRendererCapsule** | T2+T4 | Side-by-side and unified diff display |
| **ApprovalWorkflowCapsule** | T1 | Accept/reject/review buttons |
| **ChangePreviewCapsule** | T1+T5 | Before/after context |

**Deliverable**: Can review, accept/reject AI-proposed changes.

---

## Timeline Summary

| Phase | Weeks | Focus | Deliverable |
|-------|-------|-------|-------------|
| **6: GPU Rendering** | 1-3 | Fix stubs | Text renders via GPU |
| **7: Chat with AI** | 4-5 | Agentic core | Streaming chat, suggestions |
| **8: Text Editor** | 6-8 | Editor core | 100K+ line editing |
| **9: Code Intelligence** | 9-11 | LSP | Diagnostics, autocomplete |
| **10: Diff & Review** | 12-13 | AI workflow | Change approval UI |

**Total**: ~13 weeks for full IDE, **5 weeks** for minimum viable agentic tool (GPU + Chat).

---

## Decision Points

1. **GPU vs ANSI**: User chose GPU-native. +2-3 weeks but 60 FPS quality.
2. **First Milestone**: User chose Chat with AI. Enables agentic features early.
3. **Scope**: Assess gaps first (this document). Full IDE = 13 weeks.

---

## Critical Files to Read Before Implementation

**GPU Pipeline**:
- `/home/samuel/Primitives/atomic_capsule/src/terminal/render/pipeline.rs`
- `/home/samuel/Primitives/atomic_capsule/src/terminal/render/atlas.rs`
- `/home/samuel/Primitives/atomic_capsule/src/gpu/kgpu/` (if exists)

**Chat/Streaming**:
- `/home/samuel/Primitives/atomic_capsule/src/collections/async_log.rs`
- `/home/samuel/Primitives/atomic_capsule/src/http/mcp_transport.rs`

**Widget System**:
- `/home/samuel/Primitives/atomic_capsule/src/terminal/widget/complex/list.rs`
- `/home/samuel/Primitives/atomic_capsule/src/terminal/widget/mod.rs`

---

## Notes

- **Phases 1-5 Complete**: 48 capsules, ~970 tests, ~22K LOC
- **GPU Pipeline**: Architecture exists but no kgpu connection - MUST FIX FIRST
- **Widget Rendering**: All complex widgets have empty render() - MUST IMPLEMENT
