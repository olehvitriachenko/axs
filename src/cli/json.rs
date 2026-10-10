use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

use axs::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, MAX_TOKENS};
use axs::{AxsError, AxsValue, ErrorKind, SourceSpan};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use serde_json::{Number, Value};

type Object = HashMap<String, AxsValue>;

pub(super) fn from_json(source: &str) -> Result<Object, AxsError> {
    // serde_json validates borrowed fragments without recursively allocating a
    // Value tree. RawValue retains number spellings and duplicate object keys.
    let raw: &RawValue = serde_json::from_str(source).map_err(|error| json_error(error, source))?;
    let mut nodes = 0;
    match convert(raw, 0, &mut nodes)? {
        AxsValue::Object(object) => Ok(object),
        _ => Err(AxsError::new(
            ErrorKind::InvalidValue,
            "JSON document root must be an object",
        )),
    }
}

fn json_error(error: serde_json::Error, source: &str) -> AxsError {
    let byte_column = error.column().saturating_sub(1);
    let column = source
        .lines()
        .nth(error.line().saturating_sub(1))
        .map_or(1, |line| {
            line.char_indices()
                .take_while(|(byte, _)| *byte < byte_column)
                .count()
                + 1
        });
    AxsError::new(ErrorKind::SyntaxError, format!("Invalid JSON: {error}"))
        .with_span(SourceSpan::new(error.line().max(1), column, 1))
}

fn fragment<'a, T: Deserialize<'a>>(source: &'a str) -> Result<T, AxsError> {
    serde_json::from_str(source).map_err(|error| {
        // Fragment-relative offsets must not be presented as document spans.
        AxsError::new(
            ErrorKind::SyntaxError,
            format!("JSON conversion failed: {error}"),
        )
    })
}

fn convert(raw: &RawValue, depth: usize, nodes: &mut usize) -> Result<AxsValue, AxsError> {
    if *nodes >= MAX_TOKENS {
        return Err(AxsError::new(
            ErrorKind::SyntaxError,
            "Maximum JSON conversion node count exceeded",
        ));
    }
    *nodes += 1;
    let source = raw.get();
    match source.as_bytes().first() {
        Some(b'{') => {
            let RawObject(entries) = fragment(source)?;
            check_depth(depth, !entries.is_empty())?;
            let mut object = HashMap::new();
            for (key, raw) in entries {
                if object.contains_key(&key) {
                    return Err(AxsError::new(
                        ErrorKind::DuplicateKey,
                        format!("Duplicate JSON key '{key}'"),
                    ));
                }
                object.insert(key, convert(raw, depth + 1, nodes)?);
            }
            Ok(AxsValue::Object(object))
        }
        Some(b'[') => {
            let RawArray(entries) = fragment(source)?;
            check_depth(depth, !entries.is_empty())?;
            entries
                .into_iter()
                .map(|raw| convert(raw, depth + 1, nodes))
                .collect::<Result<Vec<_>, _>>()
                .map(AxsValue::Array)
        }
        Some(b'"') => fragment(source).map(AxsValue::String),
        Some(b't' | b'f') => fragment(source).map(AxsValue::Boolean),
        Some(b'n') => {
            fragment::<()>(source)?;
            Ok(AxsValue::Null)
        }
        _ => number(source),
    }
}

fn check_depth(depth: usize, nonempty: bool) -> Result<(), AxsError> {
    if nonempty && depth > MAX_NESTING_DEPTH {
        Err(AxsError::new(
            ErrorKind::InvalidIndentation,
            "Maximum nesting depth exceeded during JSON conversion",
        ))
    } else {
        Ok(())
    }
}

fn number(raw: &str) -> Result<AxsValue, AxsError> {
    // Preserve JSON negative zero instead of erasing its sign through i64.
    if raw == "-0" {
        return Ok(AxsValue::Float(-0.0));
    }
    if !raw.contains(['.', 'e', 'E']) {
        return raw.parse::<i64>().map(AxsValue::Integer).map_err(|_| {
            AxsError::new(
                ErrorKind::InvalidNumber,
                "JSON integer is outside the AXS i64 range",
            )
        });
    }
    let fail = || {
        AxsError::new(
            ErrorKind::InvalidNumber,
            "JSON number is not a finite, decimal-round-trip-safe f64",
        )
    };
    let value = raw.parse::<f64>().map_err(|_| fail())?;
    let encoded = Number::from_f64(value).ok_or_else(fail)?.to_string();
    let original = decimal(raw);
    if original.is_none() || original != decimal(&encoded) {
        return Err(fail());
    }
    Ok(AxsValue::Float(value))
}

// Compare decimal numerical values without allocating exponent-sized strings.
// Reject decimals changed by shortest f64 JSON rendering, but accept normal
// JSON numbers such as 0.1 and 1e-200. This is not arbitrary-precision storage.
fn decimal(source: &str) -> Option<(bool, String, i64)> {
    let negative = source.starts_with('-');
    let unsigned = source.strip_prefix('-').unwrap_or(source);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
        None => (unsigned, 0),
    };
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Some((negative, String::new(), 0));
    }
    let trimmed = significant.trim_end_matches('0');
    let power = exponent
        .checked_sub(i64::try_from(fraction).ok()?)?
        .checked_add(i64::try_from(significant.len() - trimmed.len()).ok()?)?;
    Some((negative, trimmed.to_owned(), power))
}

struct RawObject<'a>(Vec<(String, &'a RawValue)>);
impl<'de> Deserialize<'de> for RawObject<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = RawObject<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut entries = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    if entries.len() >= MAX_TOKENS {
                        return Err(de::Error::custom(
                            "Maximum JSON object entry count exceeded",
                        ));
                    }
                    entries.push((key, map.next_value()?));
                }
                Ok(RawObject(entries))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

struct RawArray<'a>(Vec<&'a RawValue>);
impl<'de> Deserialize<'de> for RawArray<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArrayVisitor;
        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = RawArray<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON array")
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                let mut entries = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if entries.len() >= MAX_TOKENS {
                        return Err(de::Error::custom("Maximum JSON array entry count exceeded"));
                    }
                    entries.push(value);
                }
                Ok(RawArray(entries))
            }
        }
        deserializer.deserialize_seq(ArrayVisitor)
    }
}

pub(super) fn to_json(object: Object) -> Result<Vec<u8>, AxsError> {
    let value = json_value(AxsValue::Object(object))?;
    let mut output = BoundedOutput(Vec::new());
    serde_json::to_writer_pretty(&mut output, &value).map_err(|error| {
        AxsError::new(
            ErrorKind::SyntaxError,
            format!("JSON output failed: {error}"),
        )
    })?;
    output
        .write_all(b"\n")
        .map_err(|error| AxsError::new(ErrorKind::SyntaxError, error.to_string()))?;
    Ok(output.0)
}

fn json_value(value: AxsValue) -> Result<Value, AxsError> {
    Ok(match value {
        AxsValue::String(text) => Value::String(text),
        AxsValue::Integer(value) => Value::Number(value.into()),
        AxsValue::Float(value) => Value::Number(Number::from_f64(value).ok_or_else(|| {
            AxsError::new(ErrorKind::InvalidNumber, "Non-finite float cannot be JSON")
        })?),
        AxsValue::Boolean(value) => Value::Bool(value),
        AxsValue::Null => Value::Null,
        AxsValue::Array(values) => Value::Array(
            values
                .into_iter()
                .map(json_value)
                .collect::<Result<_, _>>()?,
        ),
        AxsValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key, json_value(value)?)))
                .collect::<Result<_, AxsError>>()?,
        ),
    })
}

struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_INPUT_BYTES - self.0.len() {
            return Err(io::Error::other(
                "Maximum JSON output size exceeded (at most 8 MiB)",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
