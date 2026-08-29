//! # CL-8: Compact 8-bit Cyrillic-Latin Text Encoding
//!
//! CL-8 is a high-performance, compact 8-bit text encoding designed for mixed Cyrillic and Latin
//! text with `no_std`, zero-allocation, and zero runtime dependencies.
//!
//! Each base character is encoded into exactly one byte (0–219). Diacritic combinations are encoded
//! using 14 modifier prefixes into 2 bytes, while unsupported Unicode characters (emojis, CJK, etc.)
//! are seamlessly handled via an inline Unicode fallback mode.
//!
//! # API Levels
//!
//! Core (`default`): zero-allocation, `no_std` compatible:
//!
//! ```
//! use cl8::{encode_into, decode_into};
//!
//! let mut enc_buf = [0u8; 64];
//! let n = encode_into("Hello, мир!", &mut enc_buf).unwrap();
//!
//! let mut dec_buf = [0u8; 64];
//! let m = decode_into(&enc_buf[..n], &mut dec_buf).unwrap();
//! let text = core::str::from_utf8(&dec_buf[..m]).unwrap();
//! assert_eq!(text, "Hello, мир!");
//! ```
//!
//! Allocating API (`feature = "alloc"` or `feature = "std"`):
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! let bytes = cl8::encode("Hello, мир!").unwrap();
//! let text = cl8::decode(&bytes).unwrap();
//! assert_eq!(text, "Hello, мир!");
//! # }
//! ```
//!
//! # Input Preprocessing
//!
//! Section 10.3: Input strings must be in Unicode Normalization Form C (NFC).
//! The encoder detects standalone combining marks and returns [`EncodeError::NotNfcNormalized`].

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod caps;
pub mod consts;
pub mod decode;
pub mod encode;
pub mod error;
pub mod modifier;
pub mod tables;
pub mod unicode_fallback;

#[cfg(feature = "alloc")]
pub mod alloc_api;

// --- Flat re-exports: primary public crate surface ---

pub use decode::{decode_into, max_decoded_len};
pub use encode::{encode_into, max_encoded_len};
pub use error::{DecodeError, EncodeError};
pub use modifier::{Modifier, ModifierStack};
pub use tables::{BASE_TABLE, CAPITALIZE_TABLE, MODIFIER_TABLE};

#[cfg(feature = "alloc")]
pub use alloc_api::{decode, encode};

/// Specification version implemented by this release.
///
/// Verified by test suite: changes to the specification version must be accompanied by test audits.
pub const SPEC_VERSION: &str = "final-2026-08";
