use crate::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, tokenize, valid_key};
use crate::string;
use crate::value::AxsValue;
use crate::value_parser::parse_value;
use std::collections::HashMap;

type AxsObject = HashMap<String, AxsValue>;

pub fn stringify(data: &AxsObject) -> Result<String, String> {
    let mut output = String::new();
    write_object(data, 0, &mut output)?;
    // Enforce the same token budget as the reader before returning a document.
    tokenize(&output).map_err(|error| error.to_string())?;
    Ok(output)
}

fn write_object(object: &AxsObject, depth: usize, output: &mut String) -> Result<(), String> {
    check_depth(depth)?;
    let mut entries: Vec<_> = object.iter().collect();
    entries.sort_by_key(|(key, _)| *key);

    let indent = "  ".repeat(depth);

    for (key, value) in entries {
        validate_key(key)?;

        match value {
            AxsValue::Object(nested) if !nested.is_empty() => {
                append(output, &format!("{indent}{key}\n"))?;
                write_object(nested, depth + 1, output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                append(output, &format!("{indent}{key}\n"))?;
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

fn write_array(array: &[AxsValue], depth: usize, output: &mut String) -> Result<(), String> {
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

fn encode_inline(value: &AxsValue) -> Result<String, String> {
    match value {
        AxsValue::String(text) => {
            if text.len() > MAX_INPUT_BYTES {
                return Err("Maximum output size exceeded".into());
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
                return Err("Non-finite float is not supported".into());
            }

            let mut text = n.to_string();

            if !text.contains('.') {
                text.push_str(".0");
            }

            // Display emits decimal notation; verify grammar and exact bits rather
            // than ever returning an encoding that changes the type or value.
            match parse_value(&text) {
                Ok(AxsValue::Float(restored)) if restored.to_bits() == n.to_bits() => Ok(text),
                _ => Err("Float cannot be represented in AXS decimal syntax".into()),
            }
        }

        AxsValue::Boolean(b) => Ok(b.to_string()),
        AxsValue::Null => Ok("null".into()),

        AxsValue::Array(items) if items.is_empty() => Ok("[]".into()),

        AxsValue::Object(object) if object.is_empty() => Ok("{}".into()),

        _ => Err("Nested collection requires a block".into()),
    }
}

fn is_safe_unquoted(value: &str) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.chars().any(char::is_control)
        || value.starts_with('"')
        // In an array, a valid key followed by '=' denotes an inline object.
        || value.split_once('=').is_some_and(|(key, _)| valid_key(key.trim()))
    {
        return false;
    }

    matches!(
        parse_value(value),
        Ok(AxsValue::String(ref parsed)) if parsed == value
    )
}

fn validate_key(key: &str) -> Result<(), String> {
    if key.len() > MAX_INPUT_BYTES {
        return Err("Maximum output size exceeded".into());
    }
    if !valid_key(key) {
        return Err(format!("Invalid AXS key: '{key}'"));
    }
    Ok(())
}

fn check_depth(depth: usize) -> Result<(), String> {
    if depth > MAX_NESTING_DEPTH {
        return Err(format!(
            "Maximum nesting depth exceeded (at most {MAX_NESTING_DEPTH} levels)"
        ));
    }
    Ok(())
}

fn append(output: &mut String, text: &str) -> Result<(), String> {
    if text.len() > MAX_INPUT_BYTES - output.len() {
        return Err(format!(
            "Maximum output size exceeded (at most {MAX_INPUT_BYTES} bytes)"
        ));
    }
    output.push_str(text);
    Ok(())
}
