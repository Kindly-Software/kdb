# kindly-av1 Microsoft Store Assets

## Asset Requirements

All assets must be PNG format with transparency support. Use the kindly brand colors consistently.

## Brand Colors

| Color Name | Hex Code | Usage |
|------------|----------|-------|
| Byzantine Royal Purple | `#9B59B6` | Primary background, app tiles |
| Golden Spark | `#F1C40F` | Accent, highlights, CTAs |
| White | `#FFFFFF` | Text, icons on purple background |
| Dark Purple | `#7D3C98` | Gradients, depth |

## Required Assets

### 1. StoreLogo.png (50×50)
**Purpose**: App tile in search results, small icon references

**Design**:
- Purple background (`#9B59B6`)
- White "K" monogram centered
- 4px padding on all sides
- Optional: Subtle golden accent line at bottom

**Template**:
```
┌──────────────────┐
│  ┏━━━┓           │
│  ┃ K ┃  Purple   │
│  ┗━━━┛  #9B59B6  │
│  ╾─────╼ Golden  │
└──────────────────┘
```

### 2. Square44x44Logo.png (44×44)
**Purpose**: App icon, taskbar, system tray

**Design**:
- Same as StoreLogo but scaled to 44×44
- Ensure "K" remains legible at small size
- 3px padding on all sides

### 3. Square150x150Logo.png (150×150)
**Purpose**: Start menu medium tile

**Design**:
- Purple gradient background (top: `#9B59B6`, bottom: `#7D3C98`)
- White "K" monogram (72×72 centered)
- App name "kindly-av1" below in small white text (12pt)
- Optional: Golden arc accent in top-right corner

**Template**:
```
┌────────────────────────────┐
│ #9B59B6 ╲                  │
│          ╲                 │
│    ┏━━━━━┓  ╲ Gradient    │
│    ┃  K  ┃   ╲            │
│    ┗━━━━━┛    ╲ #7D3C98   │
│  kindly-av1     ╲          │
└────────────────────────────┘
```

### 4. Wide310x150Logo.png (310×150)
**Purpose**: Wide start menu tile

**Design**:
- Horizontal layout with branding
- Left side: "K" monogram (100×100)
- Right side: "kindly-av1" text + tagline
  - Title: "kindly-av1" (24pt bold)
  - Subtitle: "GPU-Accelerated AV1 Encoder" (14pt)
- Background: Purple gradient
- Golden accent divider between left and right sections

**Template**:
```
┌──────────────────────────────────────────────────────┐
│         ┃                                             │
│  ┏━━┓   ┃  kindly-av1                                │
│  ┃K ┃   ┃  GPU-Accelerated AV1 Encoder               │
│  ┗━━┛   ┃                                             │
│         ┃                                             │
└──────────────────────────────────────────────────────┘
  Purple    Golden   White text on purple
```

### 5. LargeTile.png (310×310)
**Purpose**: Large start menu tile (optional but recommended)

**Design**:
- Full branding showcase
- Top: Large "K" monogram (150×150)
- Middle: "kindly-av1" title (28pt)
- Bottom: "World's Fastest AV1 Encoder" tagline (16pt)
- Background: Purple radial gradient (center: `#9B59B6`, edges: `#7D3C98`)
- Golden accent arc/frame around edges

**Template**:
```
┌──────────────────────────────┐
│  ╔═══════════════╗           │
│  ║   ┏━━━━━┓     ║           │
│  ║   ┃  K  ┃     ║  Radial  │
│  ║   ┗━━━━━┛     ║  Gradient│
│  ║ kindly-av1    ║           │
│  ║ World's       ║           │
│  ║ Fastest       ║           │
│  ║ AV1 Encoder   ║           │
│  ╚═══════════════╝           │
└──────────────────────────────┘
```

## Design Guidelines

### Typography
- **Title Font**: Inter Bold or SF Pro Display Bold
- **Subtitle Font**: Inter Regular or SF Pro Display Regular
- **Weights**: Bold (700), Regular (400)

### Spacing
- **Small (44×44, 50×50)**: 3-4px padding
- **Medium (150×150)**: 10-15px padding
- **Large (310×150, 310×310)**: 20-30px padding

### Iconography
- "K" monogram: Custom design or use geometric sans-serif (Montserrat, Raleway)
- Rounded corners: 8px radius for containers
- Stroke width: 4px for borders/frames

### Contrast
- Ensure 4.5:1 contrast ratio (WCAG AA) between text and background
- White text on `#9B59B6` purple: ✅ 6.1:1 contrast (passes)
- Golden `#F1C40F` on purple: ⚠️ 1.8:1 contrast (use for accents only, not text)

## Generation Tools

### Option 1: Figma (Recommended)
1. Create 310×310 canvas
2. Design assets at largest size
3. Export each tile size as PNG (2× resolution for HiDPI)

**Figma Template**: [kindly-av1 Store Assets](https://figma.com/community/templates/kindly-av1-store-assets) (create and share)

### Option 2: Adobe Illustrator
1. Create artboards for each size
2. Design in vector (scalable)
3. Export as PNG (File > Export > Export for Screens)

### Option 3: Canva (Quick)
1. Use "App Icon" template (310×310)
2. Customize with brand colors
3. Download PNG
4. Resize using ImageMagick or Photoshop

### Option 4: ImageMagick Script
```bash
# Generate placeholder assets (replace with real designs)
./create_assets.sh
```

## Validation Checklist

Before submission, verify:

- [ ] All 5 PNG files present in `Assets/` directory
- [ ] Correct dimensions (use `identify *.png` to verify)
- [ ] PNG format (not JPEG or WebP)
- [ ] Transparency supported (alpha channel)
- [ ] File sizes <500KB each (preferably <100KB)
- [ ] Color profile: sRGB
- [ ] No text aliasing or pixelation
- [ ] Consistent branding across all sizes
- [ ] Readable at smallest size (44×44) when viewed on monitor

**Validation Commands**:
```bash
# Check dimensions
identify -format "%f: %wx%h\n" *.png

# Check file sizes
ls -lh *.png

# Check color profile
identify -verbose StoreLogo.png | grep -i colorspace
```

## Asset References

- **Microsoft Guidelines**: https://learn.microsoft.com/windows/apps/design/style/app-icons-and-logos
- **Asset Generator**: https://apetools.webprofusion.com/ (automated resizing)
- **Brand Kit**: `/home/samuel/Primitives/kindly-av1/docs/brand/` (if available)

## Current Status

⚠️ **PLACEHOLDER ASSETS ONLY**

The current `Assets/` directory contains 1×1 pixel placeholders for reference.

**Action Required**: Replace with branded designs before Microsoft Store submission.

**Timeline**: Allow 2-4 hours for professional asset design and export.

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-29
