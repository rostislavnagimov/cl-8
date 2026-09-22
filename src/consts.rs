//! Control and service bytes.

/// Space (U+0020).
pub const SPACE: u8 = 232;

/// Line feed (U+000A).
pub const ENTER: u8 = 233;

/// Horizontal tab (U+0009).
pub const TAB: u8 = 234;

/// Erase last character instruction.
pub const BACKSPACE: u8 = 235;

/// Enter Unicode fallback mode.
pub const UNI_ON: u8 = 236;

/// Capitalize next character.
pub const CASE_SHIFT: u8 = 237;

/// First diacritic modifier code (inclusive).
pub const MODIFIER_FIRST: u8 = 238;

/// Last diacritic modifier code (inclusive).
pub const MODIFIER_LAST: u8 = 251;

/// Enable uppercase mode.
pub const CAPS_ON: u8 = 252;

/// Disable uppercase mode.
pub const CAPS_OFF: u8 = 253;

/// End of message.
pub const EOM: u8 = 254;

/// Cancel message.
pub const CANCEL: u8 = 255;

/// Threshold to prefer `CAPS_ON`/`CAPS_OFF` toggle over `CASE_SHIFT` prefixes.
pub const CAPS_TOGGLE_THRESHOLD: usize = 3;

/// Returns `true` if `b` closes Unicode fallback mode.
#[must_use]
#[inline]
pub const fn is_unicode_terminator(b: u8) -> bool {
    matches!(b, 0xC0 | 0xC1 | 0xF5..=0xFF)
}
