//! Lookup tables.

include!(concat!(env!("OUT_DIR"), "/tables.rs"));

use crate::consts::MODIFIER_FIRST;

/// Codepoint for a CL-8 code, or `None` if unassigned.
#[must_use]
#[inline]
pub fn codepoint_of(code: u8) -> Option<u32> {
    BASE_TABLE.get(code as usize).copied().map(u32::from)
}

/// CL-8 code for a codepoint.
#[must_use]
#[inline]
#[allow(clippy::cast_possible_truncation)]
pub fn code_of(codepoint: u32) -> Option<u8> {
    match codepoint {
        0x0030..=0x0039 => return Some((codepoint - 0x0030) as u8),
        0x0061..=0x007A => return Some((codepoint - 0x0061 + 10) as u8),
        0x0430..=0x044F => {
            let offset = codepoint - 0x0430;
            return Some(if offset < 6 {
                (36 + offset) as u8
            } else {
                (36 + offset + 1) as u8
            });
        }
        0x0451 => return Some(42),
        0x0041..=0x005A => return Some((codepoint - 0x0041 + 161) as u8),
        0x0410..=0x042F => {
            let offset = codepoint - 0x0410;
            return Some(if offset < 6 {
                (187 + offset) as u8
            } else {
                (187 + offset + 1) as u8
            });
        }
        0x0401 => return Some(193),
        _ => {}
    }

    code_of_indexed(codepoint)
}

/// Binary search over [`BASE_BY_CP`] for non-arithmetic codes.
#[inline(never)]
fn code_of_indexed(codepoint: u32) -> Option<u8> {
    let Ok(needle) = u16::try_from(codepoint) else {
        return None;
    };
    if needle > BASE_CP_MAX {
        return None;
    }

    let mut lo = 0usize;
    let mut hi = BASE_BY_CP.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let code = BASE_BY_CP[mid];
        match BASE_TABLE[code as usize].cmp(&needle) {
            core::cmp::Ordering::Equal => return Some(code),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

/// Applies a diacritic modifier to a base code.
#[must_use]
#[inline]
pub fn apply_modifier(modifier: Modifier, base: u8) -> Option<u32> {
    let group = (modifier as u8).wrapping_sub(MODIFIER_FIRST) as usize;
    if group + 1 >= MOD_OFFSETS.len() {
        return None;
    }
    let mut lo = MOD_OFFSETS[group] as usize;
    let mut hi = MOD_OFFSETS[group + 1] as usize;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (_, b, result) = MODIFIER_TABLE[mid];
        match b.cmp(&base) {
            core::cmp::Ordering::Equal => return Some(u32::from(result)),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

/// Uppercase form of a codepoint.
#[must_use]
#[inline]
pub fn to_upper(codepoint: u32) -> Option<u32> {
    if matches!(codepoint, 0x0061..=0x007A | 0x0430..=0x044F) {
        return Some(codepoint - 0x20);
    }
    if codepoint == 0x0451 {
        return Some(0x0401);
    }
    let needle = u16::try_from(codepoint).ok()?;
    let mut lo = 0usize;
    let mut hi = CAPITALIZE_TABLE.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (lower, upper) = CAPITALIZE_TABLE[mid];
        match lower.cmp(&needle) {
            core::cmp::Ordering::Equal => return Some(u32::from(upper)),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

/// Returns `(Modifier, base)` pair for a composite codepoint.
#[must_use]
#[inline]
pub fn find_modifier_pair_for_codepoint(codepoint: u32) -> Option<(Modifier, u8)> {
    let Ok(needle) = u16::try_from(codepoint) else {
        return None;
    };
    if !(MOD_RESULT_MIN..=MOD_RESULT_MAX).contains(&needle) {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = MOD_BY_RESULT.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (modifier, base, result) = MODIFIER_TABLE[MOD_BY_RESULT[mid] as usize];
        match result.cmp(&needle) {
            core::cmp::Ordering::Equal => return Some((modifier, base)),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

/// Lowercase form of an uppercase codepoint.
#[must_use]
#[inline]
pub fn from_upper_to_lower(codepoint: u32) -> Option<u32> {
    if matches!(codepoint, 0x0041..=0x005A | 0x0410..=0x042F) {
        return Some(codepoint + 0x20);
    }
    if codepoint == 0x0401 {
        return Some(0x0451);
    }
    if codepoint == 0x0130 {
        return Some(0x0069);
    }

    let needle = u16::try_from(codepoint).ok()?;
    let mut lo = 0usize;
    let mut hi = REVERSE_CAPITALIZE_TABLE.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (upper, lower) = REVERSE_CAPITALIZE_TABLE[mid];
        match upper.cmp(&needle) {
            core::cmp::Ordering::Equal => return Some(u32::from(lower)),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}
