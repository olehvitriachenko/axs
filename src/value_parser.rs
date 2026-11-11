use crate::string;
use crate::value::{AxsObject, AxsValue};

fn unsigned_integer_syntax(value: &str) -> bool {
    value == "0"
        || (!value.is_empty()
            && !value.starts_with('0')
            && value.bytes().all(|c| c.is_ascii_digit()))
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}

fn number_parts(value: &str) -> &str {
    value.strip_prefix('-').unwrap_or(value)
}

fn integer_syntax(value: &str) -> bool {
    unsigned_integer_syntax(number_parts(value))
}

// whole[.fraction][(e|E)[+|-]digits], with a fraction or an exponent present.
fn float_syntax(value: &str) -> bool {
    let unsigned = number_parts(value);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (
            mantissa,
            Some(exponent.strip_prefix(['+', '-']).unwrap_or(exponent)),
        ),
        None => (unsigned, None),
    };
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (mantissa, None),
    };

    (fraction.is_some() || exponent.is_some())
        && unsigned_integer_syntax(whole)
        && fraction.is_none_or(digits)
        && exponent.is_none_or(digits)
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
        "{}" => return Ok(AxsValue::Object(AxsObject::new())),
        _ => {}
    }

    if value.starts_with(['[', '{']) {
        return Err(format!(
            "Strings starting with '{}' must be quoted",
            &value[..1]
        ));
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
