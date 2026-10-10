#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Identifier(String),
    Equals,
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

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();

    let start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');

    start && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn emit_entry(
    text: &str,
    line: usize,
    column: usize,
    tokens: &mut Vec<Token>,
) -> Result<(), String> {
    if let Some((raw_key, raw_value)) = text.split_once('=') {
        let key = raw_key.trim();
        let value = raw_value.trim();

        if !valid_key(key) || value.is_empty() {
            return Err(format!("Line {line}: invalid assignment"));
        }

        let key_offset = raw_key.chars().take_while(|c| c.is_whitespace()).count();

        let equals_offset = raw_key.chars().count();

        let leading_value_spaces = raw_value.chars().take_while(|c| c.is_whitespace()).count();

        let value_offset = equals_offset + 1 + leading_value_spaces;

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
    } else {
        let key = text.trim();

        if !valid_key(key) {
            return Err(format!("Line {line}: invalid key '{key}'"));
        }

        tokens.push(token(TokenKind::Identifier(key.into()), line, column));
    }

    Ok(())
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut indents = vec![0usize];

    for (index, raw) in input.lines().enumerate() {
        let line = index + 1;

        // AXS forbids tab characters.
        if raw.contains('\t') {
            return Err(format!("Line {line}: tabs are forbidden"));
        }

        let text = raw.trim();

        // Ignore blank lines and comments.
        if text.is_empty() || text.starts_with('#') {
            continue;
        }

        let spaces = raw.chars().take_while(|c| *c == ' ').count();

        if spaces % 2 != 0 {
            return Err(format!("Line {line}: indentation must use 2 spaces"));
        }

        let depth = spaces / 2;
        let current = *indents.last().unwrap();

        if depth > current {
            if depth != current + 1 {
                return Err(format!("Line {line}: invalid indent jump"));
            }

            indents.push(depth);

            tokens.push(token(TokenKind::Indent, line, 1));
        } else if depth < current {
            while *indents.last().unwrap() > depth {
                indents.pop();

                tokens.push(token(TokenKind::Dedent, line, 1));
            }

            if *indents.last().unwrap() != depth {
                return Err(format!("Line {line}: invalid dedent"));
            }
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
                return Err(format!("Line {line}: empty array item"));
            }

            // A quoted array item is always a value,
            // even when it contains '='.
            if rest.starts_with('"') {
                tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
            } else if let Some((key, _)) = rest.split_once('=') {
                // Example: - name = Oleh
                if valid_key(key.trim()) {
                    emit_entry(rest, line, value_column, &mut tokens)?;
                } else {
                    tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
                }
            } else {
                // Example: - Rust
                tokens.push(token(TokenKind::Value(rest.into()), line, value_column));
            }
        } else {
            emit_entry(text, line, column, &mut tokens)?;
        }

        tokens.push(token(TokenKind::Newline, line, raw.chars().count() + 1));
    }

    let eof_line = input.lines().count() + 1;

    while indents.len() > 1 {
        indents.pop();

        tokens.push(token(TokenKind::Dedent, eof_line, 1));
    }

    tokens.push(token(TokenKind::Eof, eof_line, 1));

    Ok(tokens)
}
