# Error View Enhancement - Visual Examples

## Error Category Examples

### 1. Warning Error (⚠️ Orange)

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  ⚠️ Error                                       │
│                                                 │
│  Deduplication cancelled                        │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Visual**:
- Border: 2px solid #FF9500 (orange)
- Background: rgba(255, 149, 0, 0.1) (10% orange)
- Header: "⚠️ Error" (16px, orange)
- Message: "Deduplication cancelled" (13px, white)

**Trigger**: User clicks cancel button during processing

---

### 2. File Error (📁 Red)

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  📁 Error                                       │
│                                                 │
│  File not found: /path/to/missing.jsonl         │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Visual**:
- Border: 2px solid #FF3B30 (red)
- Background: rgba(255, 59, 48, 0.1) (10% red)
- Header: "📁 Error" (16px, red)
- Message: "File not found: /path/to/missing.jsonl" (13px, white)

**Trigger**: Invalid file path or file doesn't exist

---

### 3. Memory Error (💾 Red, Darker)

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  💾 Error                                       │
│                                                 │
│  Memory allocation failed: Out of memory        │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Visual**:
- Border: 2px solid #FF3B30 (red)
- Background: rgba(255, 59, 48, 0.15) (15% red, darker than other errors)
- Header: "💾 Error" (16px, red)
- Message: "Memory allocation failed: Out of memory" (13px, white)

**Trigger**: System runs out of memory during processing

---

### 4. Generic Error (❌ Red)

```
┌─────────────────────────────────────────────────┐
│                                                 │
│  ❌ Error                                       │
│                                                 │
│  Please select an input file first              │
│                                                 │
└─────────────────────────────────────────────────┘
```

**Visual**:
- Border: 2px solid #FF3B30 (red)
- Background: rgba(255, 59, 48, 0.1) (10% red)
- Header: "❌ Error" (16px, red)
- Message: "Please select an input file first" (13px, white)

**Trigger**: User tries to process without selecting a file

---

## Color Comparison Table

| Error Type | Icon | Border Color | Background Alpha | Severity |
|------------|------|--------------|------------------|----------|
| Warning | ⚠️ | #FF9500 (Orange) | 10% | Low |
| File Error | 📁 | #FF3B30 (Red) | 10% | Medium |
| Memory Error | 💾 | #FF3B30 (Red) | 15% | High |
| Generic Error | ❌ | #FF3B30 (Red) | 10% | Medium |

## Keyword Matching Examples

### Warning Keywords
- "cancelled" → ⚠️ Warning
- "skipped" → ⚠️ Warning
- "Deduplication cancelled" → ⚠️ Warning

### File Error Keywords
- "file" → 📁 File Error
- "i/o" → 📁 File Error
- "not found" → 📁 File Error
- "File not found: /path/to/file.jsonl" → 📁 File Error
- "I/O error reading file" → 📁 File Error

### Memory Error Keywords
- "memory" → 💾 Memory Error
- "allocation" → 💾 Memory Error
- "Memory allocation failed" → 💾 Memory Error
- "Out of memory" → 💾 Memory Error

### Generic Error (No Match)
- "Please select an input file first" → ❌ Generic Error
- "Invalid threshold value" → ❌ Generic Error
- "Unknown error occurred" → ❌ Generic Error

## Testing Scenarios

### Scenario 1: User Action (Warning)
**User Action**: Click cancel button during processing
**Expected Error**: "Deduplication cancelled"
**Expected Display**: ⚠️ Warning with orange border

### Scenario 2: File Not Found (File Error)
**User Action**: Select non-existent file or invalid path
**Expected Error**: "File not found: /path/to/file.jsonl"
**Expected Display**: 📁 File Error with red border

### Scenario 3: Out of Memory (Memory Error)
**User Action**: Process extremely large dataset (>available RAM)
**Expected Error**: "Memory allocation failed" or "Out of memory"
**Expected Display**: 💾 Memory Error with red border and darker background (15% alpha)

### Scenario 4: Missing Input (Generic Error)
**User Action**: Click "Deduplicate" without selecting a file
**Expected Error**: "Please select an input file first"
**Expected Display**: ❌ Generic Error with red border

## Byzantine Theme Integration

### Color Palette Used
```rust
// From theme/colors.rs
pub const WARNING: Color = Color::from_rgb(1.0, 0.584, 0.0);    // #FF9500
pub const ERROR: Color = Color::from_rgb(1.0, 0.231, 0.188);    // #FF3B30
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.961, 0.961, 0.969);   // #F5F5F7
```

### Background Colors (with alpha)
```rust
// Warning background
with_alpha(WARNING, 0.1)  // rgba(255, 149, 0, 0.1)

// Error background (standard)
with_alpha(ERROR, 0.1)    // rgba(255, 59, 48, 0.1)

// Error background (severe)
with_alpha(ERROR, 0.15)   // rgba(255, 59, 48, 0.15)
```

### Layout Spacing
```rust
column![
    text(format!("{} Error", icon))
        .size(16)           // Header size
        .style(color),
    vertical_space(Length::Fixed(5.0)),  // 5px gap
    text(error)
        .size(13)           // Message size
        .style(TEXT_PRIMARY),
]
.spacing(0)
.padding(12)  // Inner padding
```

## Accessibility Considerations

### Color Blindness
- ⚠️ Warning uses **orange** (#FF9500) - distinguishable from red
- 📁, 💾, ❌ Errors use **red** (#FF3B30) - standard error color
- Icons provide additional visual cue beyond color

### Icon Support
- Unicode emojis (⚠️, 📁, 💾, ❌)
- Fallback: Modern Linux systems (2023+) support emoji rendering
- Alternative: ASCII symbols if emojis not supported

### Readability
- White text (TEXT_PRIMARY #F5F5F7) on colored backgrounds
- High contrast ratio (>4.5:1 for WCAG AA compliance)
- 13px minimum font size for error messages

## Implementation Details

### Categorization Function
```rust
let error_lower = error.to_lowercase();
let (icon, color, bg_alpha) = if error_lower.contains("cancelled") || error_lower.contains("skipped") {
    ("⚠️", WARNING, 0.1)  // Warning: user action
} else if error_lower.contains("file") || error_lower.contains("i/o") || error_lower.contains("not found") {
    ("📁", ERROR, 0.1)  // File error
} else if error_lower.contains("memory") || error_lower.contains("allocation") {
    ("💾", ERROR, 0.15)  // Memory error (more severe)
} else {
    ("❌", ERROR, 0.1)  // Generic error
};
```

### ErrorBoxStyle
```rust
struct ErrorBoxStyle {
    bg_color: Color,
    border_color: Color,
}

impl container::StyleSheet for ErrorBoxStyle {
    type Style = Theme;
    fn appearance(&self, _style: &Self::Style) -> container::Appearance {
        container::Appearance {
            background: Some(iced::Background::Color(self.bg_color)),
            border_radius: 8.0.into(),
            border_width: 2.0,
            border_color: self.border_color,
            text_color: Some(TEXT_PRIMARY),
        }
    }
}
```

## Future Enhancements

### Additional Error Categories
- 🌐 Network Error (network timeout, connection refused)
- 🔒 Permission Error (access denied, insufficient privileges)
- ⚙️ Configuration Error (invalid settings, missing config)
- 📊 Data Error (invalid format, corrupted data)

### Severity Levels
- **Info** (ℹ️): Informational messages (blue)
- **Warning** (⚠️): Non-critical issues (orange)
- **Error** (❌): Standard errors (red)
- **Critical** (🔥): Severe errors (dark red, 20% alpha)

### Error Actions
- **Retry Button**: For transient errors (network, file)
- **Help Link**: Context-sensitive documentation
- **Copy Error**: Copy error message to clipboard
- **Report Bug**: Submit error report

---

**Document Version**: v1.0
**Date**: 2025-11-16
**Author**: Claude (AI Assistant)
**Framework**: UCE34, ASSUM, I20
