//! WebAssembly bindings for the AXS core, used by the `@axs-lang/core` npm package.
//!
//! JavaScript values never cross the boundary as live objects. The TypeScript SDK
//! validates and encodes values into a compact binary payload (see [`codec`]),
//! and this crate decodes it, runs the existing AXS parser or serializer, and
//! returns another payload. Rust therefore never calls back into user JavaScript
//! (getters, proxies), and integers and floats keep their exact bits.
//!
//! Every exported function returns an envelope: one status byte (`0` success,
//! `1` AXS error) followed by the result or an encoded [`axs::AxsError`].

#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod codec;

use wasm_bindgen::prelude::wasm_bindgen;

/// Version of the binary payload format shared with the TypeScript SDK.
pub const PAYLOAD_VERSION: u32 = 1;

/// Returns [`PAYLOAD_VERSION`] so the SDK can detect mismatched build artifacts.
#[wasm_bindgen(js_name = payloadVersion)]
pub fn payload_version() -> u32 {
    PAYLOAD_VERSION
}

/// Parses AXS text with [`axs::parse`] and encodes the resulting object.
#[wasm_bindgen]
pub fn parse(source: &str) -> Vec<u8> {
    match axs::parse(source) {
        Ok(object) => codec::encode_ok_object(&object),
        Err(error) => codec::encode_error(&error),
    }
}

/// Decodes an object payload and serializes it with the canonical AXS serializer.
#[wasm_bindgen]
pub fn stringify(payload: &[u8]) -> Vec<u8> {
    let result =
        codec::decode_object(payload).and_then(|object| axs::serializer::stringify_typed(&object));
    match result {
        Ok(text) => codec::encode_ok_text(&text),
        Err(error) => codec::encode_error(&error),
    }
}
