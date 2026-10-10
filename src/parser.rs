use crate::value::AxsValue;
use std::collections::HashMap;

struct Line<'a> {
    number: usize,
    depth: usize,
    text: &'a str,
}

type AxsObject = HashMap<String, AxsValue>;

pub fn parse_value(input: &str) -> Result<AxsValue, String> {
    let value = input.trim();

    if value.is_empty() {
        return Err("Empty value".into());
    }

    if value.starts_with('"') {
        return crate::string::decode(value).map(AxsValue::String);
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
    let integer = digits == "0"
        || (!digits.starts_with('0')
            && !digits.is_empty()
            && digits.bytes().all(|c| c.is_ascii_digit()));

    if integer {
        if let Ok(n) = value.parse::<i64>() {
            return Ok(AxsValue::Integer(n));
        }
    }

    if let Some((whole, frac)) = digits.split_once('.') {
        let valid = (whole == "0"
            || (!whole.is_empty()
                && !whole.starts_with('0')
                && whole.bytes().all(|c| c.is_ascii_digit())))
            && !frac.is_empty()
            && frac.bytes().all(|c| c.is_ascii_digit());

        if valid {
            if let Ok(n) = value.parse::<f64>() {
                if n.is_finite() {
                    return Ok(AxsValue::Float(n));
                }
            }
        }
    }

    Ok(AxsValue::String(value.into()))
}

pub fn parse(input: &str) -> Result<AxsObject, String> {
    let mut lines = Vec::new();

    for (index, raw) in input.lines().enumerate() {
        let text = raw.trim();

        if text.is_empty() || text.starts_with('#') {
            continue;
        }

        if raw.contains('\t') {
            return Err(format!("Line {}: tabs are forbidden", index + 1));
        }

        let spaces = raw.len() - raw.trim_start_matches(' ').len();

        if spaces % 2 != 0 {
            return Err(format!("Line {}: invalid indentation", index + 1));
        }

        lines.push(Line {
            number: index + 1,
            depth: spaces / 2,
            text,
        });
    }

    if lines.is_empty() {
        return Ok(HashMap::new());
    }

    if lines[0].depth != 0 {
        return Err("Root must start at depth 0".into());
    }

    let mut pos = 0;
    let object = parse_object(&lines, &mut pos, 0)?;

    if pos != lines.len() {
        return Err(format!(
            "Line {}: unexpected indentation",
            lines[pos].number
        ));
    }

    Ok(object)
}

fn parse_block(lines: &[Line], pos: &mut usize, depth: usize) -> Result<AxsValue, String> {
    let line = lines.get(*pos).ok_or("Expected nested block")?;

    if line.depth != depth {
        return Err(format!("Line {}: invalid block depth", line.number));
    }

    if line.text == "-" || line.text.starts_with("- ") {
        Ok(AxsValue::Array(parse_array(lines, pos, depth)?))
    } else {
        Ok(AxsValue::Object(parse_object(lines, pos, depth)?))
    }
}

fn parse_object(lines: &[Line], pos: &mut usize, depth: usize) -> Result<AxsObject, String> {
    let mut object = HashMap::new();

    while *pos < lines.len() {
        let line = &lines[*pos];

        if line.depth < depth {
            break;
        }

        if line.depth > depth {
            return Err(format!("Line {}: unexpected indent", line.number));
        }

        if line.text == "-" || line.text.starts_with("- ") {
            return Err(format!("Line {}: expected object key", line.number));
        }

        let (key, value) = if let Some((key, raw)) = line.text.split_once('=') {
            let key = key.trim();
            validate_key(key, line.number)?;

            let value = parse_value(raw).map_err(|e| format!("Line {}: {e}", line.number))?;

            *pos += 1;
            (key, value)
        } else {
            let key = line.text;
            validate_key(key, line.number)?;

            *pos += 1;

            let next = lines
                .get(*pos)
                .ok_or_else(|| format!("Line {}: missing block", line.number))?;

            if next.depth != depth + 1 {
                return Err(format!("Line {}: expected nested block", next.number));
            }

            let value = parse_block(lines, pos, depth + 1)?;
            (key, value)
        };

        if object.insert(key.to_string(), value).is_some() {
            return Err(format!("Duplicate key: {key}"));
        }
    }

    Ok(object)
}

fn parse_array(lines: &[Line], pos: &mut usize, depth: usize) -> Result<Vec<AxsValue>, String> {
    let mut array = Vec::new();

    while *pos < lines.len() {
        let line = &lines[*pos];

        if line.depth < depth {
            break;
        }

        if line.depth != depth {
            return Err(format!("Line {}: invalid array indent", line.number));
        }

        if line.text == "-" {
            *pos += 1;

            let next = lines
                .get(*pos)
                .ok_or_else(|| format!("Line {}: missing array value", line.number))?;

            if next.depth != depth + 1 {
                return Err(format!("Line {}: expected nested block", next.number));
            }

            array.push(parse_block(lines, pos, depth + 1)?);
            continue;
        }

        let raw = line
            .text
            .strip_prefix("- ")
            .ok_or_else(|| format!("Line {}: expected '-'", line.number))?;

        if let Some((key, value)) = raw.split_once('=') {
            let key = key.trim();
            validate_key(key, line.number)?;

            let mut object = HashMap::new();
            object.insert(
                key.to_string(),
                parse_value(value).map_err(|e| format!("Line {}: {e}", line.number))?,
            );

            *pos += 1;

            // Remaining fields of the same array object
            if *pos < lines.len() && lines[*pos].depth > depth {
                if lines[*pos].depth != depth + 1 {
                    return Err(format!(
                        "Line {}: invalid object indent",
                        lines[*pos].number
                    ));
                }

                let extra = parse_object(lines, pos, depth + 1)?;

                for (k, v) in extra {
                    if object.insert(k.clone(), v).is_some() {
                        return Err(format!("Duplicate key: {k}"));
                    }
                }
            }

            array.push(AxsValue::Object(object));
        } else {
            let value = parse_value(raw).map_err(|e| format!("Line {}: {e}", line.number))?;
            *pos += 1;
            array.push(value);
        }
    }

    Ok(array)
}

fn validate_key(key: &str, line: usize) -> Result<(), String> {
    let mut chars = key.chars();

    let start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');

    let rest = chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');

    if !start || !rest {
        return Err(format!("Line {line}: invalid key '{key}'"));
    }

    Ok(())
}
