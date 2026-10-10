use crate::string;
use crate::value::AxsValue;
use std::collections::HashMap;

pub fn parse_value(input: &str) -> Result<AxsValue, String> {
    let value = input.trim();

    if value.is_empty() {
        return Err("Empty value".into());
    }

    if value.starts_with('"') {
        return string::decode(value).map(AxsValue::String);
    }

    match value {
        "true" => return Ok(AxsValue::Boolean(true)),
        "false" => return Ok(AxsValue::Boolean(false)),
        "null" => return Ok(AxsValue::Null),
        "[]" => return Ok(AxsValue::Array(vec![])),
        "{}" => return Ok(AxsValue::Object(HashMap::new())),
        _ => {}
    }

    let digits = value.strip_prefix('-').unwrap_or(value);

    let is_integer = digits == "0"
        || (!digits.starts_with('0')
            && !digits.is_empty()
            && digits.bytes().all(|c| c.is_ascii_digit()));

    if is_integer && let Ok(n) = value.parse::<i64>() {
        return Ok(AxsValue::Integer(n));
    }

    if let Some((whole, fraction)) = digits.split_once('.') {
        let valid = (whole == "0"
            || (!whole.is_empty()
                && !whole.starts_with('0')
                && whole.bytes().all(|c| c.is_ascii_digit())))
            && !fraction.is_empty()
            && fraction.bytes().all(|c| c.is_ascii_digit());

        if valid
            && let Ok(n) = value.parse::<f64>()
            && n.is_finite()
        {
            return Ok(AxsValue::Float(n));
        }
    }

    Ok(AxsValue::String(value.to_string()))
}
