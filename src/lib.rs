//! Compact 8-bit Cyrillic-Latin text encoding.
//!
//! ```
//! use cl8::{decode_into, encode_into};
//!
//! let mut enc = [0u8; 64];
//! let n = encode_into("Hello, мир!", &mut enc).unwrap();
//!
//! let mut dec = [0u8; 64];
//! let m = decode_into(&enc[..n], &mut dec).unwrap();
//! assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), "Hello, мир!");
//! ```
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! let bytes = cl8::encode("Hello, мир!").unwrap();
//! assert_eq!(cl8::decode(&bytes).unwrap(), "Hello, мир!");
//! # }
//! ```

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

pub use decode::{decode_into, max_decoded_len};
pub use encode::{encode_into, max_encoded_len, CharPlan};
pub use error::{DecodeError, EncodeError};
pub use modifier::{Modifier, ModifierStack};
pub use tables::{BASE_TABLE, CAPITALIZE_TABLE, MODIFIER_TABLE};

#[cfg(feature = "alloc")]
pub use alloc_api::{decode, encode};

/// Implemented format version.
pub const SPEC_VERSION: &str = "final-2026-08";
