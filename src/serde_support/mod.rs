mod de;
mod ser;

use serde::{Serialize, de::DeserializeOwned};

use crate::{AxsError, AxsValue, ErrorKind};

// This is a conversion-stack guard, separate from the grammar's block depth.
// Transparent Serde wrappers can recurse without adding AXS collection levels.
const MAX_CONVERSION_DEPTH: usize = 512;

fn invalid(message: impl Into<String>) -> AxsError {
    AxsError::new(ErrorKind::InvalidValue, message)
}

fn number_error(message: impl Into<String>) -> AxsError {
    AxsError::new(ErrorKind::InvalidNumber, message)
}

fn resource_error(message: impl Into<String>) -> AxsError {
    AxsError::new(ErrorKind::SyntaxError, message)
}

/// Deserialize an AXS root object into an owned Rust value.
///
/// Syntax errors retain parser spans. Type errors carry field/index context,
/// but no source span because the intermediate tree does not retain locations.
pub fn from_str<T: DeserializeOwned>(source: &str) -> Result<T, AxsError> {
    let value = AxsValue::Object(crate::parse(source)?);
    T::deserialize(de::Deserializer::new(value))
}

/// Serialize a Rust value whose Serde representation is an AXS root object.
///
/// Scalar and sequence roots are rejected. Existing grammar and output budgets
/// are enforced; non-finite or inexact numeric conversions are errors.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, AxsError> {
    let mut budget = ser::Budget::default();
    let tree = value.serialize(ser::Serializer::new(&mut budget))?;
    match tree {
        AxsValue::Object(object) => crate::serializer::stringify_typed(&object),
        _ => Err(invalid(
            "AXS document root must serialize as an object (struct or string-keyed map)",
        )),
    }
}
