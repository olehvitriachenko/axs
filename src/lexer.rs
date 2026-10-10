use crate::error::{AxsError, ErrorKind, SourceSpan};

/// Maximum block depth; root fields are at depth zero.
pub const MAX_NESTING_DEPTH: usize = 128;
/// Maximum UTF-8 document size, also enforced on serializer output.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum emitted token count, including structural tokens and EOF.
pub const MAX_TOKENS: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    Equals,
    Colon,
    Dash,
    Value(String),
    Indent,
    Dedent,
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

fn token(kind: TokenKind, line: usize, column: usize) -> Token {
    Token {
        kind,
        span: Span { line, column },
    }
}

fn error(kind: ErrorKind, message: &str, line: usize, column: usize, length: usize) -> AxsError {
    AxsError::new(kind, message).with_span(SourceSpan::new(line, column, length))
}

pub(crate) fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();

    let start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');

    start && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

// Only the first delimiter is structural; assignment values remain opaque.
pub(crate) fn entry_delimiter(text: &str) -> Option<(usize, char)> {
    text.char_indices().find(|(_, ch)| matches!(ch, '=' | ':'))
}

fn emit_entry(
    text: &str,
    line: usize,
    column: usize,
    tokens: &mut Vec<Token>,
) -> Result<(), AxsError> {
    if let Some((offset, '=')) = entry_delimiter(text) {
        let (raw_key, remainder) = text.split_at(offset);
        let raw_value = &remainder[1..];
        let key = raw_key.trim();
        let value = raw_value.trim();

        let key_offset = raw_key.chars().take_while(|c| c.is_whitespace()).count();

        let equals_offset = raw_key.chars().count();

        let leading_value_spaces = raw_value.chars().take_while(|c| c.is_whitespace()).count();

        let value_offset = equals_offset + 1 + leading_value_spaces;

        if !valid_key(key) {
            return Err(error(
                ErrorKind::InvalidKey,
                &format!("Invalid key '{key}'"),
                line,
                column + key_offset,
                key.chars().count().max(1),
            )
            .with_help("Keys must start with a letter or underscore"));
        }

        if value.is_empty() {
            return Err(error(
                ErrorKind::InvalidValue,
                "Expected value after '='",
                line,
                column + equals_offset + 1,
                1,
            )
            .with_help("Provide a value after '='"));
        }

        tokens.push(token(
            TokenKind::Identifier(key.into()),
            line,
            column + key_offset,
        ));

        tokens.push(token(TokenKind::Equals, line, column + equals_offset));

        tokens.push(token(
            TokenKind::Value(value.into()),
            line,
            column + value_offset,
        ));
    } else if let Some((offset, ':')) = entry_delimiter(text) {
        let key = text[..offset].trim_end();
        if !valid_key(key) {
            return Err(error(
                ErrorKind::InvalidKey,
                &format!("Invalid key '{key}'"),
                line,
                column,
                key.chars().count().max(1),
            ));
        }
        let colon_column = column + text[..offset].chars().count();
        if !text[offset + 1..].trim().is_empty() {
            return Err(error(
                ErrorKind::UnexpectedToken,
                "Expected end of line after ':'",
                line,
                colon_column,
                text[offset..].chars().count(),
            )
            .with_help("Use 'key:' for a nested collection or 'key = value' for an assignment"));
        }
        tokens.push(token(TokenKind::Identifier(key.into()), line, column));
        tokens.push(token(TokenKind::Colon, line, colon_column));
    } else {
        let key = text.trim();
        let (kind, message) = if valid_key(key) {
            (
                ErrorKind::UnexpectedToken,
                "Bare block keys are invalid; expected ':' or '='".to_owned(),
            )
        } else {
            (ErrorKind::InvalidKey, format!("Invalid key '{key}'"))
        };
        return Err(
            error(kind, &message, line, column, key.chars().count().max(1))
                .with_help("Use 'key:' for a nested collection or 'key = value' for an assignment"),
        );
    }

    Ok(())
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, AxsError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(error(
            ErrorKind::SyntaxError,
            "Maximum input size exceeded",
            1,
            1,
            1,
        )
        .with_help(format!(
            "AXS supports at most {MAX_INPUT_BYTES} input bytes"
        )));
    }
    let mut tokens: Vec<Token> = Vec::new();
    let mut indents = vec![0usize];

    for (index, raw) in input.lines().enumerate() {
        let line = index + 1;

        if let Some(tab_position) = raw.chars().position(|c| c == '\t') {
            return Err(error(
                ErrorKind::InvalidIndentation,
                "Tabs are forbidden",
                line,
                tab_position + 1,
                1,
            )
            .with_help("Use two spaces per indentation level"));
        }

        let text = raw.trim();

        if text.is_empty() || text.starts_with('#') {
            continue;
        }

        let spaces = raw.chars().take_while(|c| *c == ' ').count();

        if raw[spaces..].starts_with(char::is_whitespace) {
            return Err(error(
                ErrorKind::InvalidIndentation,
                "Indentation must use ASCII spaces",
                line,
                spaces + 1,
                1,
            ));
        }

        if spaces % 2 != 0 {
            return Err(error(
                ErrorKind::InvalidIndentation,
                "Indentation must use multiples of two spaces",
                line,
                spaces + 1,
                1,
            )
            .with_help("Use two spaces per indentation level"));
        }

        let depth = spaces / 2;
        if depth > MAX_NESTING_DEPTH {
            return Err(error(
                ErrorKind::InvalidIndentation,
                "Maximum nesting depth exceeded",
                line,
                spaces + 1,
                1,
            )
            .with_help(format!(
                "AXS supports at most {MAX_NESTING_DEPTH} nesting levels"
            )));
        }
        let current = *indents.last().unwrap();

        match depth.cmp(&current) {
            std::cmp::Ordering::Greater => {
                // An inline named block has an implicit object level at the
                // dash's continuation indentation and a child collection level.
                let inline_named_block = tokens.len() >= 4
                    && matches!(tokens[tokens.len() - 4].kind, TokenKind::Dash)
                    && matches!(tokens[tokens.len() - 3].kind, TokenKind::Identifier(_))
                    && matches!(tokens[tokens.len() - 2].kind, TokenKind::Colon)
                    && matches!(tokens[tokens.len() - 1].kind, TokenKind::Newline);
                if depth != current + 1 && !(inline_named_block && depth == current + 2) {
                    return Err(error(
                        ErrorKind::InvalidIndentation,
                        "Invalid indentation jump",
                        line,
                        spaces + 1,
                        1,
                    )
                    .with_help("Increase indentation by one level at a time"));
                }

                for level in current + 1..=depth {
                    indents.push(level);
                    tokens.push(token(TokenKind::Indent, line, level * 2 + 1));
                }
            }
            std::cmp::Ordering::Less => {
                if !indents.contains(&depth) {
                    return Err(error(
                        ErrorKind::InvalidIndentation,
                        "Invalid dedentation",
                        line,
                        spaces + 1,
                        1,
                    ));
                }
                while *indents.last().unwrap() > depth {
                    indents.pop();
                    tokens.push(token(TokenKind::Dedent, line, spaces + 1));
                }
            }

            std::cmp::Ordering::Equal => {}
        }

        let column = spaces + 1;

        if text == "-" {
            tokens.push(token(TokenKind::Dash, line, column));
        } else if let Some(after_dash) = text.strip_prefix("- ") {
            tokens.push(token(TokenKind::Dash, line, column));

            let extra_spaces = after_dash.chars().take_while(|c| c.is_whitespace()).count();

            let value_column = column + 2 + extra_spaces;
            let rest = after_dash.trim();

            if rest.is_empty() {
                return Err(error(
                    ErrorKind::InvalidValue,
                    "Empty array item",
                    line,
                    value_column,
                    1,
                ));
            }

            if rest.starts_with('"') {
                tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
            } else if let Some((offset, _)) = entry_delimiter(rest) {
                if valid_key(rest[..offset].trim()) {
                    emit_entry(rest, line, value_column, &mut tokens)?;
                } else {
                    tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
                }
            } else {
                tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
            }
        } else {
            emit_entry(text, line, column, &mut tokens)?;
        }

        tokens.push(token(TokenKind::Newline, line, raw.chars().count() + 1));
        if tokens.len() >= MAX_TOKENS {
            return Err(error(
                ErrorKind::SyntaxError,
                "Maximum token count exceeded",
                line,
                column,
                1,
            )
            .with_help(format!(
                "AXS supports at most {MAX_TOKENS} tokens including EOF"
            )));
        }
    }

    let eof_line = input.lines().count() + 1;

    while indents.len() > 1 {
        indents.pop();
        tokens.push(token(TokenKind::Dedent, eof_line, 1));
    }

    tokens.push(token(TokenKind::Eof, eof_line, 1));
    if tokens.len() > MAX_TOKENS {
        return Err(error(
            ErrorKind::SyntaxError,
            "Maximum token count exceeded",
            eof_line,
            1,
            1,
        ));
    }

    Ok(tokens)
}
