use crate::error::{AxsError, ErrorKind};
use crate::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, entry_delimiter, tokenize, valid_key};
use crate::string;
use crate::value::AxsValue;
use crate::value_parser::parse_value;
use std::collections::HashMap;

type AxsObject = HashMap<String, AxsValue>;

pub fn stringify(data: &AxsObject) -> Result<String, String> {
    stringify_typed(data).map_err(|error| error.to_string())
}

/// Like [`stringify`], but returns the structured [`AxsError`] (kind, code,
/// message) instead of its rendered string.
pub fn stringify_typed(data: &AxsObject) -> Result<String, AxsError> {
    let mut output = String::new();
    write_object(data, 0, &mut output)?;
    // Enforce the same token budget as the reader before returning a document.
    tokenize(&output)?;
    Ok(output)
}

fn write_object(object: &AxsObject, depth: usize, output: &mut String) -> Result<(), AxsError> {
    check_depth(depth)?;
    let mut entries: Vec<_> = object.iter().collect();
    entries.sort_by_key(|(key, _)| *key);

    let indent = "  ".repeat(depth);

    for (key, value) in entries {
        validate_key(key)?;

        match value {
            AxsValue::Object(nested) if !nested.is_empty() => {
                append(output, &format!("{indent}{key}:\n"))?;
                write_object(nested, depth + 1, output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                append(output, &format!("{indent}{key}:\n"))?;
                write_array(items, depth + 1, output)?;
            }

            _ => {
                let encoded = encode_inline(value)?;
                append(output, &format!("{indent}{key} = {encoded}\n"))?;
            }
        }
    }

    Ok(())
}

fn write_array(array: &[AxsValue], depth: usize, output: &mut String) -> Result<(), AxsError> {
    check_depth(depth)?;
    let indent = "  ".repeat(depth);

    for value in array {
        match value {
            AxsValue::Object(object) if !object.is_empty() => {
                append(output, &format!("{indent}-\n"))?;
                write_object(object, depth + 1, output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                append(output, &format!("{indent}-\n"))?;
                write_array(items, depth + 1, output)?;
            }

            _ => {
                let encoded = encode_inline(value)?;
                append(output, &format!("{indent}- {encoded}\n"))?;
            }
        }
    }

    Ok(())
}

fn encode_inline(value: &AxsValue) -> Result<String, AxsError> {
    match value {
        AxsValue::String(text) => {
            if text.len() > MAX_INPUT_BYTES {
                return Err(AxsError::new(
                    ErrorKind::SyntaxError,
                    "Maximum output size exceeded",
                ));
            }
            if is_safe_unquoted(text) {
                Ok(text.clone())
            } else {
                Ok(string::encode(text))
            }
        }

        AxsValue::Integer(n) => Ok(n.to_string()),

        AxsValue::Float(n) => {
            if !n.is_finite() {
                return Err(AxsError::new(
                    ErrorKind::InvalidNumber,
                    "Non-finite float is not supported",
                ));
            }

            let mut text = n.to_string();

            if !text.contains('.') {
                text.push_str(".0");
            }

            // Display emits decimal notation; verify grammar and exact bits rather
            // than ever returning an encoding that changes the type or value.
            match parse_value(&text) {
                Ok(AxsValue::Float(restored)) if restored.to_bits() == n.to_bits() => Ok(text),
                _ => Err(AxsError::new(
                    ErrorKind::InvalidNumber,
                    "Float cannot be represented in AXS decimal syntax",
                )),
            }
        }

        AxsValue::Boolean(b) => Ok(b.to_string()),
        AxsValue::Null => Ok("null".into()),

        AxsValue::Array(items) if items.is_empty() => Ok("[]".into()),

        AxsValue::Object(object) if object.is_empty() => Ok("{}".into()),

        _ => Err(AxsError::new(
            ErrorKind::InvalidValue,
            "Nested collection requires a block",
        )),
    }
}

fn is_safe_unquoted(value: &str) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.chars().any(char::is_control)
        || value.starts_with('"')
        // Array strings must not look like inline assignments or named blocks.
        || entry_delimiter(value).is_some_and(|(offset, _)| valid_key(value[..offset].trim()))
    {
        return false;
    }

    matches!(
        parse_value(value),
        Ok(AxsValue::String(ref parsed)) if parsed == value
    )
}

fn validate_key(key: &str) -> Result<(), AxsError> {
    if key.len() > MAX_INPUT_BYTES {
        return Err(AxsError::new(
            ErrorKind::SyntaxError,
            "Maximum output size exceeded",
        ));
    }
    if !valid_key(key) {
        return Err(AxsError::new(
            ErrorKind::InvalidKey,
            format!("Invalid AXS key: '{key}'"),
        ));
    }
    Ok(())
}

fn check_depth(depth: usize) -> Result<(), AxsError> {
    if depth > MAX_NESTING_DEPTH {
        return Err(AxsError::new(
            ErrorKind::InvalidIndentation,
            format!("Maximum nesting depth exceeded (at most {MAX_NESTING_DEPTH} levels)"),
        ));
    }
    Ok(())
}

fn append(output: &mut String, text: &str) -> Result<(), AxsError> {
    if text.len() > MAX_INPUT_BYTES - output.len() {
        return Err(AxsError::new(
            ErrorKind::SyntaxError,
            format!("Maximum output size exceeded (at most {MAX_INPUT_BYTES} bytes)"),
        ));
    }
    output.push_str(text);
    Ok(())
}
