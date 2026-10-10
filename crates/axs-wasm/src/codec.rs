//! Binary payload shared with `packages/axs/src/core.ts`.
//!
//! All integers are little-endian. A value is one tag byte followed by its body:
//!
//! | Tag | Value   | Body                                                   |
//! |-----|---------|--------------------------------------------------------|
//! | 0   | null    | none                                                   |
//! | 1   | false   | none                                                   |
//! | 2   | true    | none                                                   |
//! | 3   | integer | `i64`                                                  |
//! | 4   | float   | `f64` bits                                             |
//! | 5   | string  | `u32` byte length, UTF-8 bytes                         |
//! | 6   | array   | `u32` item count, items                                |
//! | 7   | object  | `u32` entry count, entries of (`u32` key length, UTF-8 key, value) |
//!
//! A successful parse returns `0` followed by an object value, a successful
//! stringify returns `0` followed by UTF-8 text, and a failure returns `1`
//! followed by: code, kind, and message strings; a span flag byte and, when set,
//! `u32` line, column, and length; a help flag byte and, when set, a help string.
//! Strings in the error body use the `u32` length prefix.

use std::collections::HashMap;

use axs::lexer::MAX_NESTING_DEPTH;
use axs::{AxsError, AxsValue, ErrorKind};

const TAG_NULL: u8 = 0;
const TAG_FALSE: u8 = 1;
const TAG_TRUE: u8 = 2;
const TAG_INTEGER: u8 = 3;
const TAG_FLOAT: u8 = 4;
const TAG_STRING: u8 = 5;
const TAG_ARRAY: u8 = 6;
const TAG_OBJECT: u8 = 7;

const STATUS_OK: u8 = 0;
const STATUS_ERROR: u8 = 1;

/// The root object is depth 0; an empty collection may sit one level below the
/// deepest block the serializer accepts. Anything deeper is rejected while
/// decoding so recursion stays bounded even for malformed payloads.
const MAX_PAYLOAD_DEPTH: usize = MAX_NESTING_DEPTH + 1;

type AxsObject = HashMap<String, AxsValue>;

pub fn encode_ok_object(object: &AxsObject) -> Vec<u8> {
    let mut output = vec![STATUS_OK];
    write_object(object, &mut output);
    output
}

pub fn encode_ok_text(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len() + 1);
    output.push(STATUS_OK);
    output.extend_from_slice(text.as_bytes());
    output
}

pub fn encode_error(error: &AxsError) -> Vec<u8> {
    let mut output = vec![STATUS_ERROR];
    write_str(error.kind.code(), &mut output);
    write_str(kind_name(error.kind), &mut output);
    write_str(&error.message, &mut output);
    match error.span {
        Some(span) => {
            output.push(1);
            write_u32(saturate(span.line), &mut output);
            write_u32(saturate(span.column), &mut output);
            write_u32(saturate(span.length), &mut output);
        }
        None => output.push(0),
    }
    match &error.help {
        Some(help) => {
            output.push(1);
            write_str(help, &mut output);
        }
        None => output.push(0),
    }
    output
}

/// Decodes a payload whose single root value must be an object.
pub fn decode_object(payload: &[u8]) -> Result<AxsObject, AxsError> {
    let mut reader = Reader {
        bytes: payload,
        offset: 0,
    };
    let object = match reader.u8()? {
        TAG_OBJECT => reader.object(0)?,
        _ => return Err(malformed("root value is not an object")),
    };
    if reader.offset != payload.len() {
        return Err(malformed("trailing bytes"));
    }
    Ok(object)
}

pub fn kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::UnexpectedToken => "UnexpectedToken",
        ErrorKind::InvalidIndentation => "InvalidIndentation",
        ErrorKind::InvalidKey => "InvalidKey",
        ErrorKind::InvalidValue => "InvalidValue",
        ErrorKind::DuplicateKey => "DuplicateKey",
        ErrorKind::UnexpectedEof => "UnexpectedEof",
        ErrorKind::InvalidString => "InvalidString",
        ErrorKind::InvalidNumber => "InvalidNumber",
        ErrorKind::SyntaxError => "SyntaxError",
    }
}

fn saturate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn write_u32(value: u32, output: &mut Vec<u8>) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_str(text: &str, output: &mut Vec<u8>) {
    // AXS documents and diagnostics are bounded far below 4 GiB.
    write_u32(saturate(text.len()), output);
    output.extend_from_slice(text.as_bytes());
}

fn write_object(object: &AxsObject, output: &mut Vec<u8>) {
    output.push(TAG_OBJECT);
    write_u32(saturate(object.len()), output);
    // HashMap order is unspecified; sort so JavaScript sees deterministic keys.
    let mut entries: Vec<_> = object.iter().collect();
    entries.sort_by_key(|(key, _)| *key);
    for (key, value) in entries {
        write_str(key, output);
        write_value(value, output);
    }
}

fn write_value(value: &AxsValue, output: &mut Vec<u8>) {
    match value {
        AxsValue::Null => output.push(TAG_NULL),
        AxsValue::Boolean(false) => output.push(TAG_FALSE),
        AxsValue::Boolean(true) => output.push(TAG_TRUE),
        AxsValue::Integer(number) => {
            output.push(TAG_INTEGER);
            output.extend_from_slice(&number.to_le_bytes());
        }
        AxsValue::Float(number) => {
            output.push(TAG_FLOAT);
            output.extend_from_slice(&number.to_bits().to_le_bytes());
        }
        AxsValue::String(text) => {
            output.push(TAG_STRING);
            write_str(text, output);
        }
        AxsValue::Array(items) => {
            output.push(TAG_ARRAY);
            write_u32(saturate(items.len()), output);
            for item in items {
                write_value(item, output);
            }
        }
        AxsValue::Object(object) => write_object(object, output),
    }
}

fn malformed(detail: &str) -> AxsError {
    AxsError::new(
        ErrorKind::InvalidValue,
        format!("Malformed SDK payload: {detail}"),
    )
    .with_help("The JavaScript and WebAssembly parts of the AXS SDK are out of sync")
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Reader<'_> {
    fn take(&mut self, count: usize) -> Result<&[u8], AxsError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| malformed("length overflow"))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| malformed("unexpected end"))?;
        self.offset = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], AxsError> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    fn u8(&mut self) -> Result<u8, AxsError> {
        Ok(self.array::<1>()?[0])
    }

    fn u32(&mut self) -> Result<usize, AxsError> {
        usize::try_from(u32::from_le_bytes(self.array()?)).map_err(|_| malformed("length overflow"))
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    /// Reads an element count, bounded by the bytes left so a forged count
    /// cannot trigger a large allocation (every element needs at least one byte).
    fn count(&mut self) -> Result<usize, AxsError> {
        let count = self.u32()?;
        if count > self.remaining() {
            return Err(malformed("count exceeds payload"));
        }
        Ok(count)
    }

    fn string(&mut self) -> Result<String, AxsError> {
        let length = self.u32()?;
        let bytes = self.take(length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| malformed("invalid UTF-8"))
    }

    fn object(&mut self, depth: usize) -> Result<AxsObject, AxsError> {
        if depth > MAX_PAYLOAD_DEPTH {
            return Err(depth_error());
        }
        let count = self.count()?;
        let mut object = HashMap::with_capacity(count);
        for _ in 0..count {
            let key = self.string()?;
            let value = self.value(depth + 1)?;
            if object.contains_key(&key) {
                return Err(malformed("duplicate object key"));
            }
            object.insert(key, value);
        }
        Ok(object)
    }

    fn value(&mut self, depth: usize) -> Result<AxsValue, AxsError> {
        Ok(match self.u8()? {
            TAG_NULL => AxsValue::Null,
            TAG_FALSE => AxsValue::Boolean(false),
            TAG_TRUE => AxsValue::Boolean(true),
            TAG_INTEGER => AxsValue::Integer(i64::from_le_bytes(self.array()?)),
            TAG_FLOAT => AxsValue::Float(f64::from_bits(u64::from_le_bytes(self.array()?))),
            TAG_STRING => AxsValue::String(self.string()?),
            TAG_ARRAY => {
                if depth > MAX_PAYLOAD_DEPTH {
                    return Err(depth_error());
                }
                let count = self.count()?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.value(depth + 1)?);
                }
                AxsValue::Array(items)
            }
            TAG_OBJECT => AxsValue::Object(self.object(depth)?),
            _ => return Err(malformed("unknown value tag")),
        })
    }
}

fn depth_error() -> AxsError {
    AxsError::new(
        ErrorKind::InvalidIndentation,
        format!("Maximum nesting depth exceeded (at most {MAX_NESTING_DEPTH} levels)"),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn round_trip(object: &AxsObject) -> AxsObject {
        let encoded = encode_ok_object(object);
        assert_eq!(encoded[0], STATUS_OK);
        decode_object(&encoded[1..]).unwrap()
    }

    #[test]
    fn values_survive_the_payload_exactly() {
        let source = "a = 9223372036854775807\nb = -9223372036854775808\nc = -0.0\n\
                      d = 0.1\ne = \"x\\ny\"\nf = null\ng = true\nh = []\ni = {}\n\
                      j:\n  - 1\n  -\n    k = Олег 🚀\n";
        let object = axs::parse(source).unwrap();
        let restored = round_trip(&object);
        assert_eq!(restored, object);
        match restored["c"] {
            AxsValue::Float(value) => assert!(value.is_sign_negative() && value == 0.0),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn object_keys_are_sorted_deterministically() {
        let object = axs::parse("z = 1\na = 2\nm = 3").unwrap();
        let encoded = encode_ok_object(&object);
        let first = encoded.windows(1).position(|w| w == b"a").unwrap();
        let second = encoded.windows(1).position(|w| w == b"m").unwrap();
        let third = encoded.windows(1).position(|w| w == b"z").unwrap();
        assert!(first < second && second < third);
    }

    #[test]
    fn malformed_payloads_are_errors_not_panics() {
        let valid = encode_ok_object(&axs::parse("a:\n  - x\nb = 1").unwrap());
        let body = &valid[1..];
        // Every truncation and every single-byte corruption must be handled.
        for end in 0..body.len() {
            let _ = decode_object(&body[..end]);
        }
        for index in 0..body.len() {
            for byte in [0, 1, 7, 0x7f, 0xff] {
                let mut corrupted = body.to_vec();
                corrupted[index] = byte;
                let _ = decode_object(&corrupted);
            }
        }
        assert!(decode_object(&[]).is_err());
        assert!(decode_object(&[TAG_ARRAY, 0, 0, 0, 0]).is_err());
        assert!(decode_object(&[TAG_OBJECT, 0, 0, 0, 0, 0]).is_err());
        assert!(decode_object(&[TAG_OBJECT, 0xff, 0xff, 0xff, 0xff]).is_err());
        let mut invalid_utf8 = vec![TAG_OBJECT, 1, 0, 0, 0, 1, 0, 0, 0, 0xff, TAG_NULL];
        assert!(decode_object(&invalid_utf8).is_err());
        invalid_utf8[9] = b'a';
        assert!(decode_object(&invalid_utf8).is_ok());
    }

    #[test]
    fn duplicate_keys_and_excess_depth_are_rejected() {
        let duplicate = [
            TAG_OBJECT, 2, 0, 0, 0, 1, 0, 0, 0, b'a', TAG_NULL, 1, 0, 0, 0, b'a', TAG_NULL,
        ];
        assert!(decode_object(&duplicate).is_err());

        let mut deep = vec![TAG_OBJECT, 1, 0, 0, 0, 1, 0, 0, 0, b'a'];
        for _ in 0..10_000 {
            deep.extend_from_slice(&[TAG_ARRAY, 1, 0, 0, 0]);
        }
        deep.push(TAG_NULL);
        let error = decode_object(&deep).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    }

    #[test]
    fn errors_carry_code_kind_span_and_help() {
        let error = axs::parse("a = 1\na = 2").unwrap_err();
        let encoded = encode_error(&error);
        assert_eq!(encoded[0], STATUS_ERROR);
        let mut reader = Reader {
            bytes: &encoded[1..],
            offset: 0,
        };
        assert_eq!(reader.string().unwrap(), "AXS005");
        assert_eq!(reader.string().unwrap(), "DuplicateKey");
        assert_eq!(reader.string().unwrap(), error.message);
        assert_eq!(reader.u8().unwrap(), 1);
        let span = error.span.unwrap();
        assert_eq!(reader.u32().unwrap(), span.line);
        assert_eq!(reader.u32().unwrap(), span.column);
        assert_eq!(reader.u32().unwrap(), span.length);
    }

    #[test]
    fn exported_functions_wrap_core_results() {
        let parsed = crate::parse("name = AXS");
        assert_eq!(parsed[0], STATUS_OK);
        let text = crate::stringify(&parsed[1..]);
        assert_eq!(text[0], STATUS_OK);
        assert_eq!(&text[1..], b"name = AXS\n");

        let failed = crate::parse("name");
        assert_eq!(failed[0], STATUS_ERROR);
        assert_eq!(crate::stringify(&[0xff])[0], STATUS_ERROR);
    }
}
