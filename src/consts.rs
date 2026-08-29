//! Control and service bytes of the CL-8 encoding specification.

/// Space character, U+0020.
pub const SPACE: u8 = 232;

/// Line feed / newline, U+000A.
pub const ENTER: u8 = 233;

/// Horizontal tab, U+0009.
pub const TAB: u8 = 234;

/// Backspace — removes the last emitted character.
///
/// A streaming protocol control instruction rather than a printable character.
pub const BACKSPACE: u8 = 235;

/// Enter Unicode fallback mode.
pub const UNI_ON: u8 = 236;

/// Capitalize the following character (Section 8.2).
pub const CASE_SHIFT: u8 = 237;

/// First diacritic modifier code (inclusive).
pub const MODIFIER_FIRST: u8 = 238;

/// Last diacritic modifier code (inclusive).
pub const MODIFIER_LAST: u8 = 251;

/// Enable uppercase mode until `CAPS_OFF` (Section 9.1).
pub const CAPS_ON: u8 = 252;

/// Disable uppercase mode.
pub const CAPS_OFF: u8 = 253;

/// End of message.
pub const EOM: u8 = 254;

/// Cancel entire message.
pub const CANCEL: u8 = 255;

/// Minimum consecutive uppercase run length for switching from `CASE_SHIFT` prefixes to `CAPS_ON`/`CAPS_OFF`.
///
/// Runs of 1 letter prefer prefix (1 extra byte), 2 letters are equivalent,
/// 3 or more letters prefer toggle (2 bytes overhead for the whole run).
pub const CAPS_TOGGLE_THRESHOLD: usize = 3;

/// Checks whether a byte is a valid Unicode fallback mode terminator.
///
/// Set `{0xC0, 0xC1, 0xF5..=0xFF}` (Section 10.2). These values cannot occur as
/// the first byte of a valid canonical UTF-8 sequence (`0xC0`/`0xC1` would be overlong,
/// `0xF5..=0xFF` exceed `U+10FFFF`). Therefore, they act as unambiguous inline terminators.
#[must_use]
#[inline]
pub const fn is_unicode_terminator(b: u8) -> bool {
    matches!(b, 0xC0 | 0xC1 | 0xF5..=0xFF)
}
