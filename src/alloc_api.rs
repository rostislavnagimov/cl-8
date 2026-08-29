//! Allocating convenience wrappers — available under `feature = "alloc"`.
//!
//! Thin adapter layer over `*_into`: computes guaranteed upper bound capacities,
//! allocates the buffer, invokes the core engine, and truncates to actual size.

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::decode::max_decoded_len;
use crate::encode::max_encoded_len;
use crate::error::{DecodeError, EncodeError};

/// Encodes input text into a newly allocated `Vec<u8>`.
///
/// # Errors
///
/// Propagates errors from [`crate::encode::encode_into`] (such as [`EncodeError::NotNfcNormalized`]).
pub fn encode(input: &str) -> Result<Vec<u8>, EncodeError> {
    let mut buf = vec![0u8; max_encoded_len(input.len())];
    let n = crate::encode::encode_into(input, &mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

/// Decodes a CL-8 byte stream into a newly allocated `String`.
///
/// # Errors
///
/// Propagates errors from [`crate::decode::decode_into`].
pub fn decode(input: &[u8]) -> Result<String, DecodeError> {
    let mut buf = vec![0u8; max_decoded_len(input.len())];
    let n = crate::decode::decode_into(input, &mut buf)?;
    buf.truncate(n);
    String::from_utf8(buf).map_err(|e| DecodeError::InvalidUtf8InUnicodeMode {
        position: e.utf8_error().valid_up_to(),
    })
}
