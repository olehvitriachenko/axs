use crate::parser::parse_value;
use crate::string;
use crate::value::AxsValue;
use std::collections::HashMap;

type AxsObject = HashMap<String, AxsValue>;

pub fn stringify(data: &AxsObject) -> Result<String, String> {
    let mut output = String::new();
    write_object(data, 0, &mut output)?;
    Ok(output)
}

fn write_object(object: &AxsObject, depth: usize, output: &mut String) -> Result<(), String> {
    let mut entries: Vec<_> = object.iter().collect();
    entries.sort_by_key(|(key, _)| *key);

    let indent = "  ".repeat(depth);

    for (key, value) in entries {
        validate_key(key)?;

        match value {
            AxsValue::Object(nested) if !nested.is_empty() => {
                output.push_str(&format!("{indent}{key}\n"));
                write_object(nested, depth + 1, output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                output.push_str(&format!("{indent}{key}\n"));
                write_array(items, depth + 1, output)?;
            }

            _ => {
                let encoded = encode_inline(value)?;
                output.push_str(&format!("{indent}{key} = {encoded}\n"));
            }
        }
    }

    Ok(())
}

fn write_array(array: &[AxsValue], depth: usize, output: &mut String) -> Result<(), String> {
    let indent = "  ".repeat(depth);

    for value in array {
        match value {
            AxsValue::Object(object) if !object.is_empty() => {
                output.push_str(&format!("{indent}-\n"));
                write_object(object, depth + 1, output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                output.push_str(&format!("{indent}-\n"));
                write_array(items, depth + 1, output)?;
            }

            _ => {
                let encoded = encode_inline(value)?;
                output.push_str(&format!("{indent}- {encoded}\n"));
            }
        }
    }

    Ok(())
}

fn encode_inline(value: &AxsValue) -> Result<String, String> {
    match value {
        AxsValue::String(text) => {
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

            Ok(text)
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
    {
        return false;
    }

    matches!(
        parse_value(value),
        Ok(AxsValue::String(ref parsed)) if parsed == value
    )
}

fn validate_key(key: &str) -> Result<(), String> {
    let mut chars = key.chars();

    let valid_start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');

    let valid_rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

    if !valid_start || !valid_rest {
        return Err(format!("Invalid AXS key: '{key}'"));
    }

    Ok(())
}
