use std::collections::HashMap;

use crate::string;
use crate::value::AxsValue;

fn unsigned_integer_syntax(value: &str) -> bool {
    value == "0"
        || (!value.is_empty()
            && !value.starts_with('0')
            && value.bytes().all(|c| c.is_ascii_digit()))
}

fn number_parts(value: &str) -> &str {
    value.strip_prefix('-').unwrap_or(value)
}

fn integer_syntax(value: &str) -> bool {
    unsigned_integer_syntax(number_parts(value))
}

fn float_syntax(value: &str) -> bool {
    let digits = number_parts(value);

    let Some((whole, fraction)) = digits.split_once('.') else {
        return false;
    };

    unsigned_integer_syntax(whole)
        && !fraction.is_empty()
        && fraction.bytes().all(|c| c.is_ascii_digit())
}

// Used by the token parser to classify numeric errors.
pub fn is_numeric_literal(value: &str) -> bool {
    integer_syntax(value) || float_syntax(value)
}

pub fn parse_value(input: &str) -> Result<AxsValue, String> {
    let value = input.trim();

    if value.is_empty() {
        return Err("Empty value".into());
    }

    if value.starts_with('"') {
        return string::decode(value).map(AxsValue::String);
    }

    if value.chars().any(char::is_control) {
        return Err("Unescaped control character in value".into());
    }

    match value {
        "true" => return Ok(AxsValue::Boolean(true)),
        "false" => return Ok(AxsValue::Boolean(false)),
        "null" => return Ok(AxsValue::Null),
        "[]" => return Ok(AxsValue::Array(vec![])),
        "{}" => return Ok(AxsValue::Object(HashMap::new())),
        _ => {}
    }

    if integer_syntax(value) {
        return value
            .parse::<i64>()
            .map(AxsValue::Integer)
            .map_err(|_| format!("Integer '{value}' is outside the i64 range"));
    }

    if float_syntax(value) {
        let number = value
            .parse::<f64>()
            .map_err(|_| format!("Invalid floating-point number '{value}'"))?;

        if !number.is_finite() {
            return Err(format!("Floating-point number '{value}' is not finite"));
        }

        return Ok(AxsValue::Float(number));
    }

    Ok(AxsValue::String(value.to_string()))
}
