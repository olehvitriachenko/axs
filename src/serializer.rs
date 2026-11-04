use crate::error::{AxsError, ErrorKind};
use crate::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, parse_field, tokenize, valid_key};
use crate::string;
use crate::value::{AxsObject, AxsValue};
use crate::value_parser::parse_value;

pub fn stringify(data: &AxsObject) -> Result<String, String> {
    stringify_typed(data).map_err(|error| error.to_string())
}

/// Like [`stringify`], but returns the structured [`AxsError`] (kind, code,
/// message) instead of its rendered string.
pub fn stringify_typed(data: &AxsObject) -> Result<String, AxsError> {
    let mut output = String::new();
    write_object(data, 0, None, &mut output)?;
    // Enforce the same token budget as the reader before returning a document.
    tokenize(&output)?;
    Ok(output)
}

/// Where a scalar is written; each position has its own quoting rules.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    /// `key: value`
    Field,
    /// `- value`
    Item,
    /// An inline array element, inline object value, or table cell.
    Element,
}

enum ArrayForm<'a> {
    Table(Vec<&'a String>),
    Inline,
    Block,
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

/// Writes an object's fields at `depth`. `first_prefix` replaces the first
/// field's indentation, so an array item can start the object on its dash line.
fn write_object(
    object: &AxsObject,
    depth: usize,
    first_prefix: Option<&str>,
    output: &mut String,
) -> Result<(), AxsError> {
    check_depth(depth)?;

    for (index, (key, value)) in object.iter().enumerate() {
        let prefix = match first_prefix {
            Some(prefix) if index == 0 => prefix.to_owned(),
            _ => indent(depth),
        };
        let key = encode_key(key)?;
        write_field(&prefix, &key, value, depth, output)?;
    }

    Ok(())
}

/// On the recursion path for every nesting level, so everything that does not
/// recurse lives in separate, never-inlined helpers to keep the frame small.
fn write_field(
    prefix: &str,
    key: &str,
    value: &AxsValue,
    depth: usize,
    output: &mut String,
) -> Result<(), AxsError> {
    match value {
        AxsValue::Object(nested) if !nested.is_empty() && !inline_object(nested) => {
            append(output, &format!("{prefix}{key}:\n"))?;
            write_object(nested, depth + 1, None, output)
        }

        AxsValue::Array(items)
            if !items.is_empty() && matches!(array_form(items), ArrayForm::Block) =>
        {
            append(output, &format!("{prefix}{key}[{}]:\n", items.len()))?;
            write_array(items, depth + 1, output)
        }

        _ => write_line_field(prefix, key, value, depth, output),
    }
}

/// A field whose value fits on its line, or a table or text block below it.
#[inline(never)]
fn write_line_field(
    prefix: &str,
    key: &str,
    value: &AxsValue,
    depth: usize,
    output: &mut String,
) -> Result<(), AxsError> {
    match value {
        AxsValue::Object(nested) if !nested.is_empty() => {
            let fields = nested
                .iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}: {}",
                        encode_key(key)?,
                        encode_scalar(value, Position::Element)?
                    ))
                })
                .collect::<Result<Vec<_>, AxsError>>()?
                .join(",");
            append(output, &format!("{prefix}{key}: {{{fields}}}\n"))
        }

        AxsValue::Array(items) if !items.is_empty() => {
            let count = items.len();
            match array_form(items) {
                ArrayForm::Table(columns) => {
                    let header = columns
                        .iter()
                        .map(|column| encode_key(column))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(",");
                    append(output, &format!("{prefix}{key}[{count}]{{{header}}}:\n"))?;
                    check_depth(depth + 1)?;
                    let row_indent = indent(depth + 1);
                    for item in items {
                        let AxsValue::Object(row) = item else {
                            unreachable!("tables contain only objects")
                        };
                        let cells = encode_elements(row.values())?;
                        append(output, &format!("{row_indent}{cells}\n"))?;
                    }
                    Ok(())
                }
                ArrayForm::Inline => {
                    let elements = encode_elements(items.iter())?;
                    append(output, &format!("{prefix}{key}[{count}]: [{elements}]\n"))
                }
                ArrayForm::Block => unreachable!("block arrays are written by write_field"),
            }
        }

        AxsValue::String(text) if text_block(text) => {
            append(output, &format!("{prefix}{key}: |\n"))?;
            write_text(text, depth + 1, output)
        }

        _ => {
            let encoded = encode_scalar(value, Position::Field)?;
            append(output, &format!("{prefix}{key}: {encoded}\n"))
        }
    }
}

fn write_array(array: &[AxsValue], depth: usize, output: &mut String) -> Result<(), AxsError> {
    check_depth(depth)?;
    let indent = indent(depth);

    for value in array {
        match value {
            AxsValue::Object(object) if !object.is_empty() => {
                write_object(object, depth + 1, Some(&format!("{indent}- ")), output)?;
            }

            AxsValue::Array(items) if !items.is_empty() => {
                if inline_array(items) {
                    let elements = encode_elements(items.iter())?;
                    append(output, &format!("{indent}- [{elements}]\n"))?;
                } else {
                    append(output, &format!("{indent}-\n"))?;
                    write_array(items, depth + 1, output)?;
                }
            }

            AxsValue::String(text) if text_block(text) => {
                append(output, &format!("{indent}- |\n"))?;
                write_text(text, depth + 1, output)?;
            }

            _ => {
                let encoded = encode_scalar(value, Position::Item)?;
                append(output, &format!("{indent}- {encoded}\n"))?;
            }
        }
    }

    Ok(())
}

fn write_text(text: &str, depth: usize, output: &mut String) -> Result<(), AxsError> {
    check_depth(depth)?;
    let indent = indent(depth);
    for line in text.split('\n') {
        if line.is_empty() {
            append(output, "\n")?;
        } else {
            append(output, &format!("{indent}{line}\n"))?;
        }
    }
    Ok(())
}

fn primitive(value: &AxsValue) -> bool {
    match value {
        AxsValue::Array(_) | AxsValue::Object(_) => false,
        AxsValue::String(text) => !text_block(text),
        _ => true,
    }
}

fn inline_array(items: &[AxsValue]) -> bool {
    items.iter().all(primitive)
}

fn array_form(items: &[AxsValue]) -> ArrayForm<'_> {
    let table = match items.first() {
        Some(AxsValue::Object(first)) if !first.is_empty() => items.iter().all(|item| {
            matches!(item, AxsValue::Object(row)
                if row.len() == first.len()
                    && row.keys().eq(first.keys())
                    && row.values().all(primitive))
        }),
        _ => false,
    };

    if table {
        let AxsValue::Object(first) = &items[0] else {
            unreachable!("checked above")
        };
        ArrayForm::Table(first.keys().collect())
    } else if inline_array(items) {
        ArrayForm::Inline
    } else {
        ArrayForm::Block
    }
}

fn encode_elements<'v>(values: impl Iterator<Item = &'v AxsValue>) -> Result<String, AxsError> {
    Ok(values
        .map(|value| encode_scalar(value, Position::Element))
        .collect::<Result<Vec<_>, _>>()?
        .join(","))
}

/// Objects written on one line: a few fields, all primitive.
const INLINE_OBJECT_FIELDS: usize = 4;

fn inline_object(object: &AxsObject) -> bool {
    !object.is_empty() && object.len() <= INLINE_OBJECT_FIELDS && object.values().all(primitive)
}

/// Strings written as `|` text blocks: multiline, and representable exactly.
fn text_block(text: &str) -> bool {
    text.contains('\n')
        && !text.starts_with('\n')
        && !text.ends_with('\n')
        && !text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        && text
            .split('\n')
            .all(|line| line.is_empty() || !line.trim().is_empty())
}

fn encode_key(key: &str) -> Result<String, AxsError> {
    if key.len() > MAX_INPUT_BYTES {
        return Err(AxsError::new(
            ErrorKind::SyntaxError,
            "Maximum output size exceeded",
        ));
    }
    if valid_key(key) {
        Ok(key.to_owned())
    } else {
        Ok(string::encode(key))
    }
}

fn encode_float(number: f64) -> Result<String, AxsError> {
    if !number.is_finite() {
        return Err(AxsError::new(
            ErrorKind::InvalidNumber,
            "Non-finite float is not supported",
        ));
    }

    // Shortest round-trip digits; exponent notation for very large and very
    // small magnitudes, as in JavaScript.
    let magnitude = number.abs();
    let text = if magnitude != 0.0 && !(1e-6..1e21).contains(&magnitude) {
        format!("{number:e}")
    } else {
        let mut text = number.to_string();
        if !text.contains('.') {
            text.push_str(".0");
        }
        text
    };

    // Verify grammar and exact bits rather than ever returning an encoding
    // that changes the type or value.
    match parse_value(&text) {
        Ok(AxsValue::Float(restored)) if restored.to_bits() == number.to_bits() => Ok(text),
        _ => Err(AxsError::new(
            ErrorKind::InvalidNumber,
            "Float cannot be represented in AXS syntax",
        )),
    }
}

fn encode_scalar(value: &AxsValue, position: Position) -> Result<String, AxsError> {
    match value {
        AxsValue::String(text) => {
            if text.len() > MAX_INPUT_BYTES {
                return Err(AxsError::new(
                    ErrorKind::SyntaxError,
                    "Maximum output size exceeded",
                ));
            }
            if is_safe_unquoted(text, position) {
                Ok(text.clone())
            } else {
                Ok(string::encode(text))
            }
        }

        AxsValue::Integer(n) => Ok(n.to_string()),
        AxsValue::Float(n) => encode_float(*n),
        AxsValue::Boolean(b) => Ok(b.to_string()),
        AxsValue::Null => Ok("null".into()),

        AxsValue::Array(items) if items.is_empty() && position != Position::Element => {
            Ok("[]".into())
        }

        AxsValue::Object(object) if object.is_empty() && position != Position::Element => {
            Ok("{}".into())
        }

        _ => Err(AxsError::new(
            ErrorKind::InvalidValue,
            "Nested collection requires a block",
        )),
    }
}

fn is_safe_unquoted(value: &str, position: Position) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value == "|"
        || value.starts_with(['"', '[', '{'])
        || value.chars().any(char::is_control)
    {
        return false;
    }

    let structural = match position {
        Position::Field => false,
        // Item strings must not look like inline object fields.
        Position::Item => parse_field(value).is_ok(),
        // Table rows must not start like a comment or a dash item.
        Position::Element => {
            value.contains([',', '[', ']', '{', '}'])
                || value.starts_with('#')
                || value == "-"
                || value.starts_with("- ")
        }
    };

    !structural
        && matches!(
            parse_value(value),
            Ok(AxsValue::String(ref parsed)) if parsed == value
        )
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
