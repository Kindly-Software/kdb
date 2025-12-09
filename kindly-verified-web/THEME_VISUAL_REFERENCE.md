# Byzantine Theme - Visual Reference Guide

**Last Updated**: November 21, 2025

---

## Color Palette

### Primary Colors
```
Byzantine Purple (Primary)
HEX: #663399 | RGB: 102, 51, 153
Usage: Primary text, accents, borders

Dark Purple (Secondary)
HEX: #4B0082 | RGB: 75, 0, 130
Usage: Gradients, hover states, depth

Light Purple (Highlight)
HEX: #8B5CF6 | RGB: 139, 92, 246
Usage: Light accents, alternative highlights
```

### Accent Colors
```
Metallic Gold (Accent)
HEX: #FFD700 | RGB: 255, 215, 0
Usage: Buttons, highlights, premium elements

Dark Gold (Darker Accent)
HEX: #DAA520 | RGB: 218, 165, 32
Usage: Gradients, hover states, shadows
```

### Background Colors
```
Deep Purple Background
HEX: #1a0033 | RGB: 26, 0, 51
Usage: Main background, dark overlays

Mid Purple Background
HEX: #2d1b4e | RGB: 45, 27, 78
Usage: Gradient end, card backgrounds
```

### Status Colors
```
Success Green: #10B981
Warning Orange: #F59E0B
Error Red: #EF4444
Neutral Gray: #9CA3AF
```

---

## Typography System

### Heading XL (Hero Title)
```
Size: 3.5rem (56px)
Weight: 800 (Extra Bold)
Line Height: 1.1
Letter Spacing: -0.02em
Effect: Gold gradient with background-clip
Usage: Main page title "Kindly Verified"
```

### Heading LG (Section Title)
```
Size: 2.5rem (40px)
Weight: 700 (Bold)
Line Height: 1.2
Color: Gold (#FFD700)
Usage: "Imperial Byzantine Tribunal", section headers
```

### Heading MD (Card/Feature Title)
```
Size: 1.75rem (28px)
Weight: 600 (Semi-Bold)
Line Height: 1.3
Color: Gold or Purple Light (alternating)
Text Transform: UPPERCASE
Letter Spacing: 0.05em
Usage: Feature card titles, subsection headers
```

### Body Text
```
Size: 1rem (16px)
Weight: 400 (Regular)
Line Height: 1.6
Color: rgba(255, 255, 255, 0.9)
Usage: Main content, descriptions, paragraphs
```

### Caption Text
```
Size: 0.875rem (14px)
Weight: 400 (Regular)
Line Height: 1.4
Color: rgba(255, 255, 255, 0.7)
Usage: Footer text, small labels, metadata
```

### Font Family
```
System Fonts: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif
Monospace: 'SF Mono', Monaco, 'Cascadia Code', 'Courier New', monospace
```

---

## Spacing System (8px Grid)

```
XS:   0.25rem (4px)    - Micro spacing
SM:   0.5rem  (8px)    - Small gaps
MD:   1rem    (16px)   - Standard gap
LG:   1.5rem  (24px)   - Medium spacing
XL:   2rem    (32px)   - Large spacing
2XL:  3rem    (48px)   - Extra large
3XL:  4rem    (64px)   - Huge spacing
4XL:  6rem    (96px)   - Section spacing
5XL:  8rem    (128px)  - Major section spacing
```

---

## Component Styles

### Buttons

**Primary Button (CTA)**
```
Background: Gold Gradient (135deg, #FFD700 → #DAA520)
Color: Dark Purple (#1a0033)
Padding: 1.5rem 4rem (vertical × horizontal)
Border Radius: 16px
Font Size: 1.25rem
Font Weight: 700 (Bold)
Text Transform: UPPERCASE
Letter Spacing: 0.05em

Shadow/Glow:
  - box-shadow: 0 0 20px rgba(255, 215, 0, 0.5),
                0 0 40px rgba(255, 215, 0, 0.3)

On Hover:
  - Transform: translateY(-4px)
  - Transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1)
```

### Cards

**Glassmorphic Card**
```
Background: rgba(102, 51, 153, 0.15)
Backdrop Filter: blur(16px)
Border: 1px solid rgba(255, 215, 0, 0.2)
Border Radius: 24px
Padding: 2rem
Box Shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37)

Border Top (Feature Cards):
  - Odd Cards: 2px solid rgba(255, 215, 0, 0.4)
  - Even Cards: 2px solid rgba(139, 92, 246, 0.4)

On Hover:
  - Box Shadow: +glow_purple()
  - Transform: translateY(-4px)
```

### Headers

**Sticky Navigation Header**
```
Background: rgba(102, 51, 153, 0.2)
Backdrop Filter: blur(24px)
Padding: 1.5rem 3rem
Border Bottom: 2px solid rgba(255, 215, 0, 0.15)
Position: sticky
Top: 0
Z-Index: 100

Logo:
  - Font Size: 1.75rem
  - Font Weight: 600
  - Icon: 👑
  - Text: Gold gradient background-clip

Buttons:
  - Border: 2px solid #FFD700
  - Background: transparent
  - Color: white
  - On Hover: Fill with gold
```

### Footers

**Imperial Footer**
```
Background: rgba(102, 51, 153, 0.1)
Backdrop Filter: blur(16px)
Padding: 4rem 3rem
Border Top: 2px solid rgba(255, 215, 0, 0.15)
Text Align: center

Content Layers:
  1. Fleur-de-lis (⚜️) + main message
  2. Technical details (small)
  3. Copyright notice (gold accent)
```

---

## Effects & Animations

### Glassmorphism Effect
```
background: rgba(102, 51, 153, 0.15)
backdrop-filter: blur(16px)
-webkit-backdrop-filter: blur(16px)
border: 1px solid rgba(255, 215, 0, 0.2)
box-shadow: 0 8px 32px 0 rgba(31, 38, 135, 0.37)
```

### Gold Glow (3-Layer)
```
box-shadow:
  0 0 20px rgba(255, 215, 0, 0.5),
  0 0 40px rgba(255, 215, 0, 0.3),
  0 0 60px rgba(255, 215, 0, 0.1)
```

### Purple Glow (3-Layer)
```
box-shadow:
  0 0 20px rgba(102, 51, 153, 0.5),
  0 0 40px rgba(102, 51, 153, 0.3),
  0 0 60px rgba(102, 51, 153, 0.1)
```

### Hover Lift
```
transform: translateY(-4px)
transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1)
```

### Gradient Backgrounds

**Hero Gradient (135deg)**
```
linear-gradient(135deg, #1a0033 0%, #2d1b4e 100%)
Diagonal slope: top-left to bottom-right
Creates depth perception
```

**Gold Gradient (135deg)**
```
linear-gradient(135deg, #FFD700 0%, #DAA520 100%)
Premium metallic effect
Used for buttons and accents
```

**Purple Shimmer (135deg, 3-Stop)**
```
linear-gradient(135deg, #8B5CF6 0%, #663399 50%, #4B0082 100%)
Light → Primary → Dark
Creates depth and dimension
Used for subtitles and emphasis
```

### Animation Keyframes

**Crown Glow (2s infinite)**
```css
0%, 100% {
  filter: drop-shadow(0 0 20px rgba(255, 215, 0, 0.5));
  transform: scale(1);
}
50% {
  filter: drop-shadow(0 0 40px rgba(255, 215, 0, 0.8));
  transform: scale(1.05);
}
```

**Slide Up (Staggered 0.1s intervals)**
```css
0% {
  opacity: 0;
  transform: translateY(30px);
}
100% {
  opacity: 1;
  transform: translateY(0);
}
Duration: 0.6s ease-out
Delays: 0.1s, 0.2s, 0.3s, 0.4s, 0.5s, 0.6s per card
```

**Progress Bar (2s infinite)**
```css
0% { width: 0%; }
50% { width: 70%; }
100% { width: 100%; }
Creates illusion of continuous loading
```

**Spinner (1s linear infinite)**
```css
0% { transform: rotate(0deg); }
100% { transform: rotate(360deg); }
Smooth continuous rotation
```

---

## Component Examples

### Feature Card Structure
```
┌─────────────────────────────┐  ← Border top (gold/purple)
│      ⚖️  (4rem icon)        │
│                             │
│  IMPERIAL FORENSIC TRIBUNAL │  ← Gold/Purple heading
│                             │
│ 10 forensic detectors: PRNU,│  ← Body text
│ Benford's Law, etc.         │
└─────────────────────────────┘
```

### Button States
```
NORMAL:
  Background: Gold gradient
  Shadow: 3-layer gold glow

HOVER:
  Transform: translateY(-4px)
  Shadow: Intensified glow
  Cursor: pointer

ACTIVE:
  Background: Dark gold
  Transform: translateY(0)
```

### Loading Screen Layout
```
╔════════════════════════════════════════╗
║                                        ║
║              👑 (6rem)                 ║
║          (Pulsing glow)                ║
║                                        ║
║        ◯ ← Spinner (80px)             ║
║        LOADING IMPERIAL SYSTEMS       ║
║   Powered by Byzantine Capsules       ║
║                                        ║
║      ◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾◾            ║
║      ▰▰▰▰▰▰▰▰ (progress bar)          ║
║                                        ║
║    ⚜️ Initializing Forensic Tribunal ⚜️║
║                                        ║
╚════════════════════════════════════════╝
```

---

## Responsive Breakpoints

```
XS:  0px+     (Mobile)
SM:  640px+   (Tablet landscape)
Md:  768px+   (Tablet portrait)
LG:  1024px+  (Desktop)
XL:  1280px+  (Large desktop)
```

---

## Accessibility Notes

### Color Contrast
```
Gold (#FFD700) on Dark Purple (#1a0033): ✅ WCAG AAA
Light Purple (#8B5CF6) on Dark Purple: ✅ WCAG AA
White text (0.9 alpha) on dark backgrounds: ✅ WCAG AAA
```

### Focus States
```
Buttons: Gold glow + subtle scale
Links: Underline + color change
Cards: Border highlight + glow
```

### Motion Preferences
```
All animations use CSS (GPU-accelerated)
Animations: <1s for quick interactions, 2s+ for background effects
Easing: cubic-bezier(0.4, 0, 0.2, 1) for smooth, professional feel
```

---

## Implementation Checklist

- ✅ Use color constants from `styles.rs` (no hardcoded hex values)
- ✅ Use spacing constants (SPACING_* - 8px grid)
- ✅ Use typography functions (text_heading_*, text_body(), text_caption())
- ✅ Use effect functions (glassmorphism(), glow_gold(), glow_purple(), hover_lift())
- ✅ Use gradient functions (gradient_hero(), gradient_gold(), gradient_purple_shimmer())
- ✅ Apply uppercase text-transform with letter-spacing: 0.05em on headings
- ✅ Include imperial/Byzantine terminology in copy
- ✅ Use 👑 emoji for crown icons and ⚜️ for fleur-de-lis accents
- ✅ Apply smooth animations with cubic-bezier easing
- ✅ Ensure hover states on interactive elements
- ✅ Test color contrast for WCAG compliance

---

## Quick Copy-Paste Snippets

### Gold Button
```rust
button style=format!(
    "background: {};
     color: {};
     padding: {} {};
     border-radius: 16px;
     border: none;
     cursor: pointer;
     font-weight: 700;
     text-transform: uppercase;
     letter-spacing: 0.05em;
     {}",
    gradient_gold(),
    COLOR_BG_DARK,
    SPACING_LG,
    SPACING_3XL,
    glow_gold()
)
```

### Feature Card
```rust
div style=format!(
    "{}
     border-top: 2px solid rgba(255, 215, 0, 0.4);
     text-align: center;",
    card_glass()
)
```

### Section Title
```rust
h3 style=format!(
    "{}
     text-align: center;
     margin-bottom: {};
     text-transform: uppercase;
     letter-spacing: 0.1em;",
    text_heading_lg(),
    SPACING_3XL
)
```

### Glassmorphic Container
```rust
div style=format!(
    "{}
     padding: {};
     border-radius: 24px;",
    glassmorphism(GlassBlur::Medium, 0.15),
    SPACING_XL
)
```

---

## Brand Voice

All UI copy uses imperial/Byzantine terminology:

| Generic | Imperial Alternative |
|---------|---------------------|
| Feature | Imperial Tribune |
| Fast | Byzantine Speed |
| Accurate | Imperial Precision |
| Secure | Royal Privacy Decree |
| Open | Byzantine Computational Capsules |
| Tribunal | Forensic Tribunal |
| Loading | Loading Imperial Systems |
| Detector | Forensic Sentinel |
| Analysis | Forensic Investigation |

---

## Design Philosophy

**Byzantine Royal Purple × Metallic Gold = Premium Imperial Aesthetic**

- **Purple**: Authority, intelligence, sophistication
- **Gold**: Luxury, value, precision
- **Glassmorphism**: Modern, premium, ethereal
- **Imperial Crown**: Authority, prestige, excellence
- **Fleur-de-lis**: Tradition, nobility, refinement

Every design element reinforces the positioning of Kindly Verified as:
1. **Imperial-grade forensic detection**
2. **Powered by Byzantine computational capsules**
3. **Premium AI detection technology**

---

**Version**: 1.0
**Last Updated**: November 21, 2025
**Status**: ✅ Production Complete
