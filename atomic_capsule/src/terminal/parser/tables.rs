//! Constant Lookup Tables for ANSI Parser (T0 Auditable, 0ns)
//!
//! Precomputed tables for zero-runtime-cost escape sequence parsing.
//!
//! # Architecture
//! - **Tier**: T0 Auditable (compile-time const)
//! - **Cost**: 0ns runtime (all compile-time)
//! - **Coverage**: Full VT100/ANSI/Xterm sequences
//!
//! # References
//! - [VT100 User Guide](https://vt100.net/docs/vt100-ug/chapter3.html)
//! - [ANSI Control Functions](https://vt100.net/docs/vt510-rm/chapter4.html)

use crate::terminal::event::{KeyCode, KeyModifiers};

/// ESC character (0x1B)
pub const ESC: u8 = 0x1B;

/// CSI - Control Sequence Introducer (ESC [)
pub const CSI_BRACKET: u8 = b'[';

/// SS3 - Single Shift 3 (ESC O) - Function keys
pub const SS3_O: u8 = b'O';

/// OSC - Operating System Command (ESC ])
pub const OSC_BRACKET: u8 = b']';

/// DCS - Device Control String (ESC P)
pub const DCS_P: u8 = b'P';

/// Bracketed paste start marker
pub const BRACKETED_PASTE_START: &[u8] = b"\x1B[200~";

/// Bracketed paste end marker
pub const BRACKETED_PASTE_END: &[u8] = b"\x1B[201~";

/// CSI final character range (0x40-0x7E)
pub const CSI_FINAL_MIN: u8 = 0x40; // @
pub const CSI_FINAL_MAX: u8 = 0x7E; // ~

/// CSI parameter character range (0x30-0x3F)
pub const CSI_PARAM_MIN: u8 = 0x30; // 0
pub const CSI_PARAM_MAX: u8 = 0x3F; // ?

/// CSI intermediate character range (0x20-0x2F)
pub const CSI_INTERMEDIATE_MIN: u8 = 0x20; // space
pub const CSI_INTERMEDIATE_MAX: u8 = 0x2F; // /

/// Lookup table: CSI final byte to KeyCode (for simple sequences)
///
/// Maps CSI final bytes (0x40-0x7E) to KeyCode.
/// Used for arrow keys, function keys, etc.
///
/// # Layout
/// ```text
/// Index | Byte | KeyCode
/// ------|------|--------
/// 0     | 0x40 | N/A
/// 1     | 0x41 | Up (A)
/// 2     | 0x42 | Down (B)
/// 3     | 0x43 | Right (C)
/// 4     | 0x44 | Left (D)
/// 5     | 0x45 | KeypadBegin (E)
/// 6     | 0x46 | End (F)
/// 7     | 0x47 | N/A (G)
/// 8     | 0x48 | Home (H)
/// ...
/// ```
pub const CSI_FINAL_TO_KEYCODE: [Option<KeyCode>; 63] = {
    let mut table = [None; 63];

    // Arrow keys (A-D)
    table[1] = Some(KeyCode::Up);       // A
    table[2] = Some(KeyCode::Down);     // B
    table[3] = Some(KeyCode::Right);    // C
    table[4] = Some(KeyCode::Left);     // D

    // Keypad/Navigation
    table[5] = Some(KeyCode::KeypadBegin); // E (numpad 5)
    table[6] = Some(KeyCode::End);         // F
    table[8] = Some(KeyCode::Home);        // H

    table
};

/// SS3 final byte to KeyCode (function keys F1-F4)
///
/// Maps SS3 sequences (ESC O x) to function keys.
///
/// # Layout
/// ```text
/// Index | Byte | KeyCode
/// ------|------|--------
/// 0     | P    | F(1)
/// 1     | Q    | F(2)
/// 2     | R    | F(3)
/// 3     | S    | F(4)
/// ```
pub const SS3_FINAL_TO_KEYCODE: [Option<KeyCode>; 4] = [
    Some(KeyCode::F(1)),  // P
    Some(KeyCode::F(2)),  // Q
    Some(KeyCode::F(3)),  // R
    Some(KeyCode::F(4)),  // S
];

/// VT100 function key codes (ESC [ n ~)
///
/// Maps numeric codes to function keys F5-F24.
///
/// # Common Sequences
/// ```text
/// Code | Sequence   | KeyCode
/// -----|------------|--------
/// 15   | ESC[15~    | F(5)
/// 17   | ESC[17~    | F(6)
/// 18   | ESC[18~    | F(7)
/// 19   | ESC[19~    | F(8)
/// 20   | ESC[20~    | F(9)
/// 21   | ESC[21~    | F(10)
/// 23   | ESC[23~    | F(11)
/// 24   | ESC[24~    | F(12)
/// ```
pub fn vt_code_to_keycode(code: u16) -> Option<KeyCode> {
    match code {
        // Special keys
        1 => Some(KeyCode::Home),
        2 => Some(KeyCode::Insert),
        3 => Some(KeyCode::Delete),
        4 => Some(KeyCode::End),
        5 => Some(KeyCode::PageUp),
        6 => Some(KeyCode::PageDown),

        // Function keys F5-F12
        15 => Some(KeyCode::F(5)),
        17 => Some(KeyCode::F(6)),
        18 => Some(KeyCode::F(7)),
        19 => Some(KeyCode::F(8)),
        20 => Some(KeyCode::F(9)),
        21 => Some(KeyCode::F(10)),
        23 => Some(KeyCode::F(11)),
        24 => Some(KeyCode::F(12)),

        // Function keys F13-F24 (extended)
        25 => Some(KeyCode::F(13)),
        26 => Some(KeyCode::F(14)),
        28 => Some(KeyCode::F(15)),
        29 => Some(KeyCode::F(16)),
        31 => Some(KeyCode::F(17)),
        32 => Some(KeyCode::F(18)),
        33 => Some(KeyCode::F(19)),
        34 => Some(KeyCode::F(20)),

        _ => None,
    }
}

/// Parse modifier flags from CSI parameter
///
/// Modifiers are encoded as: 1 + sum of flags
/// - Shift: +1
/// - Alt: +2
/// - Control: +4
/// - Super: +8
///
/// # Examples
/// ```text
/// Code | Modifiers
/// -----|----------
/// 1    | None
/// 2    | Shift
/// 3    | Alt
/// 4    | Shift+Alt
/// 5    | Control
/// 6    | Shift+Control
/// ```
pub fn parse_modifiers(modifier_code: u16) -> KeyModifiers {
    if modifier_code <= 1 {
        return KeyModifiers::NONE;
    }

    let flags = modifier_code - 1;
    let mut mods = KeyModifiers::NONE;

    if flags & 1 != 0 {
        // Shift
        mods = mods | KeyModifiers::SHIFT;
    }
    if flags & 2 != 0 {
        // Alt
        mods = mods | KeyModifiers::ALT;
    }
    if flags & 4 != 0 {
        // Control
        mods = mods | KeyModifiers::CONTROL;
    }
    if flags & 8 != 0 {
        // Super
        mods = mods | KeyModifiers::SUPER;
    }

    mods
}

/// Mouse button codes (SGR 1006 protocol)
pub const MOUSE_BUTTON_LEFT: u16 = 0;
pub const MOUSE_BUTTON_MIDDLE: u16 = 1;
pub const MOUSE_BUTTON_RIGHT: u16 = 2;
pub const MOUSE_SCROLL_UP: u16 = 64;
pub const MOUSE_SCROLL_DOWN: u16 = 65;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csi_arrow_keys() {
        // A = Up (index 1)
        assert_eq!(CSI_FINAL_TO_KEYCODE[1], Some(KeyCode::Up));
        // B = Down (index 2)
        assert_eq!(CSI_FINAL_TO_KEYCODE[2], Some(KeyCode::Down));
        // C = Right (index 3)
        assert_eq!(CSI_FINAL_TO_KEYCODE[3], Some(KeyCode::Right));
        // D = Left (index 4)
        assert_eq!(CSI_FINAL_TO_KEYCODE[4], Some(KeyCode::Left));
    }

    #[test]
    fn test_ss3_function_keys() {
        assert_eq!(SS3_FINAL_TO_KEYCODE[0], Some(KeyCode::F(1))); // P
        assert_eq!(SS3_FINAL_TO_KEYCODE[1], Some(KeyCode::F(2))); // Q
        assert_eq!(SS3_FINAL_TO_KEYCODE[2], Some(KeyCode::F(3))); // R
        assert_eq!(SS3_FINAL_TO_KEYCODE[3], Some(KeyCode::F(4))); // S
    }

    #[test]
    fn test_vt_function_keys() {
        assert_eq!(vt_code_to_keycode(15), Some(KeyCode::F(5)));
        assert_eq!(vt_code_to_keycode(17), Some(KeyCode::F(6)));
        assert_eq!(vt_code_to_keycode(24), Some(KeyCode::F(12)));
    }

    #[test]
    fn test_vt_special_keys() {
        assert_eq!(vt_code_to_keycode(1), Some(KeyCode::Home));
        assert_eq!(vt_code_to_keycode(2), Some(KeyCode::Insert));
        assert_eq!(vt_code_to_keycode(3), Some(KeyCode::Delete));
        assert_eq!(vt_code_to_keycode(4), Some(KeyCode::End));
        assert_eq!(vt_code_to_keycode(5), Some(KeyCode::PageUp));
        assert_eq!(vt_code_to_keycode(6), Some(KeyCode::PageDown));
    }

    #[test]
    fn test_modifier_parsing() {
        assert_eq!(parse_modifiers(1), KeyModifiers::NONE);
        assert_eq!(parse_modifiers(2), KeyModifiers::SHIFT);
        assert_eq!(parse_modifiers(3), KeyModifiers::ALT);
        assert_eq!(parse_modifiers(5), KeyModifiers::CONTROL);

        // Combined modifiers
        let ctrl_shift = parse_modifiers(6);
        assert!(ctrl_shift.contains(KeyModifiers::SHIFT));
        assert!(ctrl_shift.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_bracketed_paste_markers() {
        assert_eq!(BRACKETED_PASTE_START, b"\x1B[200~");
        assert_eq!(BRACKETED_PASTE_END, b"\x1B[201~");
    }
}
