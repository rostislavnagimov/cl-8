# CL-8

[![Crates.io](https://img.shields.io/crates/v/cl-8)](https://crates.io/crates/cl-8)
[![Documentation](https://docs.rs/cl-8/badge.svg)](https://docs.rs/cl-8)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/rostislavnagimov/cl-8)

**Compact 8-bit text encoding for Cyrillic and Latin** — `no_std`, zero-allocation, zero dependencies.

Every base character encodes into exactly one byte (codes 0–219). Diacritics use two bytes
(modifier + base), and anything outside the tables (emoji, CJK, Arabic) streams through a
transparent UTF-8 fallback. Cyrillic text shrinks by ~45% compared to UTF-8.

## Key Features

* **~45% smaller Cyrillic:** UTF-8 spends 2 bytes per Cyrillic letter; CL-8 spends exactly 1 —
  for both lowercase `а-я` and uppercase `А-Я`, `Ё` (direct-mapped, no prefix bytes).
* **Direct uppercase:** Latin `A-Z` (codes 161–186) and Cyrillic `А-Я`, `Ё` (codes 187–219)
  are single-byte with no case-shift overhead.
* **41 languages via 14 diacritic modifiers:** `é`, `č`, `š`, `ž`, `ü`, `ø`, `å`, `ñ`, `ç`, `ł`,
  `ş`, `ș`, `ő`, `ż` and the rest encode as 2 bytes (`modifier + base`).
* **CAPS-toggle compression:** where 3 or more uppercase letters would each need a case-shift
  prefix (e.g. `ЄЇЂ`, `ČŠŽ`), they are wrapped in `CAPS_ON … CAPS_OFF` instead — 2 bytes for
  the whole run rather than 1 byte per letter. Spaces, digits, punctuation and direct
  capitals pass through a run unchanged, so one toggle pair can span several words.
* **Transparent Unicode fallback:** unsupported characters (ZWJ emoji sequences, CJK, Arabic)
  pass through byte-for-byte inside an inline `UNI_ON` + UTF-8 mode.
* **Zero-allocation & `no_std`:** the core writes into caller-provided slices only. No heap,
  no allocator, no `std` — suitable for embedded targets.
* **Zero runtime dependencies** and `#![forbid(unsafe_code)]`.

## Quickstart

### Core API (`no_std`, default — zero allocation)

```rust
use cl8::{encode_into, decode_into};

let text = "Привет, World!";

// Encode into a caller-provided buffer.
let mut enc_buf = [0u8; 64];
let n = encode_into(text, &mut enc_buf).unwrap();

// Decode back into UTF-8.
let mut dec_buf = [0u8; 64];
let m = decode_into(&enc_buf[..n], &mut dec_buf).unwrap();

assert_eq!(core::str::from_utf8(&dec_buf[..m]).unwrap(), text);
```

Buffer sizing is guaranteed by `max_encoded_len(input.len())` and `max_decoded_len(input.len())`.

### Allocating API (`feature = "alloc"` or `"std"`)

```toml
[dependencies]
cl-8 = { version = "0.1", features = ["alloc"] }
```

```rust
let bytes: Vec<u8> = cl8::encode("Привет, мир!").unwrap();
let text: String = cl8::decode(&bytes).unwrap();
assert_eq!(text, "Привет, мир!");
```

## Compression

Measured by `cargo bench` (the size report prints before the throughput benchmarks):

| Dataset | UTF-8 bytes | CL-8 bytes | Savings |
|---|---|---|---|
| Russian prose | 545 | 301 | **44.8%** |
| Russian ALL-CAPS | 412 | 225 | **45.4%** |
| Mixed chat (RU/EN, emoji, digits) | 202 | 173 | **14.4%** |
| European diacritics (de/fr/pl/cs/tr) | 352 | 346 | 1.7% |
| CJK-heavy (fallback mode) | 221 | 155 | 29.9% |
| English ASCII | 315 | 315 | 0.0% |

English never gets worse than UTF-8; Cyrillic approaches the theoretical 50% bound
(spaces and punctuation are 1 byte in both encodings).

## Performance

Run the built-in Criterion benchmarks to measure on your hardware:

```bash
cargo bench            # full run: compression report + encode/decode throughput
cargo bench -- --quick # fast pass
```

Encoding resolves >95% of standard text (digits, `a-z`, `A-Z`, `а-я`, `А-Я`) through O(1)
arithmetic with no table scans; decoding is a single-pass state machine over direct table
lookups. Hundreds of MiB/s on commodity hardware for both directions.

## Feature Flags

| Feature | Description |
|---|---|
| *(default)* | Zero-allocation `no_std` core: `encode_into` / `decode_into` over `&mut [u8]` |
| `alloc` | Adds `encode(&str) -> Vec<u8>` and `decode(&[u8]) -> String` (needs an allocator, not `std`) |
| `std` | Enables `alloc` and implements `std::error::Error` for the error types |

## Byte Map

| Range | Meaning |
|---|---|
| 0–9 | Digits `0-9` |
| 10–35 | Latin `a-z` |
| 36–68 | Cyrillic `а-я`, `ё` |
| 69–96 | Supplementary Cyrillic (`є`, `ї`, `ђ`, `ґ`, …) and non-decomposable letters (`ß`, `æ`, …) |
| 97–160 | Punctuation, currency, special symbols |
| 161–186 | Latin `A-Z` (direct uppercase) |
| 187–219 | Cyrillic `А-Я`, `Ё` (direct uppercase) |
| 220–231 | Reserved |
| 232–236 | Space, newline, tab, backspace, `UNI_ON` (Unicode fallback) |
| 237 | `CASE_SHIFT` — capitalize the next character |
| 238–251 | 14 diacritic modifiers (acute, caron, umlaut, …) |
| 252–255 | `CAPS_ON`, `CAPS_OFF`, `EOM`, `CANCEL` |

## Guarantees

- `#![forbid(unsafe_code)]` — no unsafe anywhere, including tests.
- No heap allocations in the default configuration (there is no allocator to allocate from).
- Decoder never panics on arbitrary input bytes (property-tested) and always emits valid UTF-8.
- Round-trip `decode(encode(s)) == s` holds for any NFC-normalized input (property-tested with
  10,000 cases per strategy).
- Input must be NFC-normalized; standalone combining marks are rejected with
  `EncodeError::NotNfcNormalized` instead of being silently mangled.

## Building and Testing

```bash
cargo build --release                                  # optimized build
cargo build --no-default-features                      # pure no_std core
cargo test --all-features                              # full suite (139 tests)
cargo clippy --all-targets --all-features -- -D warnings
cargo bench                                            # compression + throughput metrics
cargo run --example demo                               # 73 round-trip cases across 38 languages
```

The `demo` example prints the encoded bytes, size and savings for each sample, flags which
cases used the CAPS toggle or the Unicode fallback, and exits non-zero if any round-trip
is inexact.

Lookup tables are generated at compile time by `build.rs` from the CSV sources in `tables/`,
with build-time validation of all structural invariants (sequential codes, unique codepoints,
reachable capitalization pairs).

## License

Licensed under either of:

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
