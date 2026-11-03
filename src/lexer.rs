use crate::error::{AxsError, ErrorKind, SourceSpan};
use crate::string;

/// Maximum block depth; root fields are at depth zero.
pub const MAX_NESTING_DEPTH: usize = 128;
/// Maximum UTF-8 document size, also enforced on serializer output.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum token count, including structural tokens, inline array elements,
/// table cells, and EOF.
pub const MAX_TOKENS: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A decoded key (bare or quoted).
    Identifier(String),
    /// A declared array length: `key[N]`.
    Length(usize),
    /// Decoded table field names: `key{a, b}`.
    Columns(Vec<String>),
    Colon,
    Dash,
    /// A raw scalar value, including `[]` handled as an empty inline array.
    Value(String),
    /// Raw inline array elements or table row cells, split on top-level commas.
    Items(Vec<String>),
    /// The decoded content of a `|` text block.
    Text(String),
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

fn key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')
}

pub(crate) fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();

    let start = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');

    start && chars.all(key_char)
}

/// Returns the byte index just past the closing quote of a string starting
/// at `text[0] == '"'`, honoring backslash escapes.
pub(crate) fn scan_quoted(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in text.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(index + 1);
        }
    }
    None
}

/// Splits on commas outside quoted strings. Only a `"` that begins a part
/// (after spaces) opens a quoted string. Fails on an unterminated quote.
pub(crate) fn split_top_level(text: &str) -> Option<Vec<&str>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut at_start = true;
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == ',' {
            parts.push(&text[start..index]);
            start = index + 1;
            at_start = true;
        } else if ch == '"' && at_start {
            quoted = true;
            at_start = false;
        } else if ch != ' ' {
            at_start = false;
        }
    }
    if quoted {
        return None;
    }
    parts.push(&text[start..]);
    Some(parts)
}

/// A field line: `key[N]{a, b}: value`, with optional length and columns.
#[derive(Debug)]
pub(crate) struct Field<'a> {
    key: String,
    key_chars: usize,
    length: Option<usize>,
    columns: Option<Vec<String>>,
    /// Character offset of `:` within the field text.
    colon: usize,
    /// The trimmed value and its character offset; `None` for a block header.
    value: Option<(&'a str, usize)>,
}

#[derive(Debug)]
pub(crate) struct FieldError {
    kind: ErrorKind,
    message: String,
    help: &'static str,
}

fn field_error(kind: ErrorKind, message: impl Into<String>, help: &'static str) -> FieldError {
    FieldError {
        kind,
        message: message.into(),
        help,
    }
}

fn decode_key(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('"') {
        string::decode(raw).ok()
    } else if valid_key(raw) {
        Some(raw.to_owned())
    } else {
        None
    }
}

const FIELD_HELP: &str = "Write fields as 'key: value', or 'key:' followed by an indented block";

pub(crate) fn parse_field(text: &str) -> Result<Field<'_>, FieldError> {
    let (key, mut rest) = if text.starts_with('"') {
        let end = scan_quoted(text).ok_or_else(|| {
            field_error(
                ErrorKind::InvalidString,
                "Unterminated quoted key",
                "Close the key with '\"'",
            )
        })?;
        let key = string::decode(&text[..end]).map_err(|message| {
            field_error(ErrorKind::InvalidString, message, "Fix the quoted key")
        })?;
        (key, &text[end..])
    } else {
        let end = text.find(|c: char| !key_char(c)).unwrap_or(text.len());
        let key = &text[..end];
        if !valid_key(key) {
            let shown = text.split([':', ' ']).next().unwrap_or(text);
            return Err(field_error(
                ErrorKind::InvalidKey,
                format!("Invalid key '{shown}'"),
                "Keys start with a letter or '_'; quote any other key, as in \"@id\": value",
            ));
        }
        (key.to_owned(), &text[end..])
    };
    let key_chars = text.chars().count() - rest.chars().count();

    let mut length = None;
    if let Some(after) = rest.strip_prefix('[') {
        let close = after.find(']').ok_or_else(|| {
            field_error(
                ErrorKind::InvalidCollection,
                "Unclosed array length",
                "Write the length as 'key[3]:'",
            )
        })?;
        let digits = after[..close].trim();
        let valid = digits == "0"
            || (!digits.starts_with('0')
                && !digits.is_empty()
                && digits.bytes().all(|c| c.is_ascii_digit()));
        length = Some(
            digits
                .parse::<usize>()
                .ok()
                .filter(|_| valid)
                .ok_or_else(|| {
                    field_error(
                        ErrorKind::InvalidCollection,
                        format!("Invalid array length '{digits}'"),
                        "A length is a decimal integer without leading zeros",
                    )
                })?,
        );
        rest = &after[close + 1..];
    }

    let mut columns = None;
    if let Some(after) = rest.strip_prefix('{') {
        let malformed = || {
            field_error(
                ErrorKind::InvalidCollection,
                "Malformed table header",
                "Write a table header as 'key{a, b}:'",
            )
        };
        let mut close = None;
        let mut index = 0;
        while index < after.len() {
            let tail = &after[index..];
            if tail.starts_with('"') {
                index += scan_quoted(tail).ok_or_else(malformed)?;
            } else if tail.starts_with('}') {
                close = Some(index);
                break;
            } else {
                index += tail.chars().next().map_or(1, char::len_utf8);
            }
        }
        let close = close.ok_or_else(malformed)?;
        let names = split_top_level(&after[..close])
            .ok_or_else(malformed)?
            .into_iter()
            .map(decode_key)
            .collect::<Option<Vec<_>>>()
            .ok_or_else(malformed)?;
        columns = Some(names);
        rest = &after[close + 1..];
    }

    let spaced = rest.trim_start_matches(' ');
    let Some(after_colon) = spaced.strip_prefix(':') else {
        let (message, help) = if spaced.starts_with('=') {
            (
                "Expected ':' after key; '=' is not AXS syntax",
                "Write 'key: value'",
            )
        } else {
            ("Expected ':' after key", FIELD_HELP)
        };
        return Err(field_error(ErrorKind::UnexpectedToken, message, help));
    };
    let colon = text.chars().count() - spaced.chars().count();

    let value = if after_colon.trim().is_empty() {
        None
    } else if after_colon.starts_with(' ') {
        let trimmed = after_colon.trim();
        let leading = after_colon.chars().take_while(|c| *c == ' ').count();
        Some((trimmed, colon + 1 + leading))
    } else {
        return Err(field_error(
            ErrorKind::UnexpectedToken,
            "Expected a space after ':'",
            "Write 'key: value'",
        ));
    };

    Ok(Field {
        key,
        key_chars,
        length,
        columns,
        colon,
        value,
    })
}

struct Lexer<'a> {
    lines: Vec<&'a str>,
    next_line: usize,
    tokens: Vec<Token>,
    /// Inline array elements and table cells, counted against MAX_TOKENS.
    elements: usize,
    indents: Vec<usize>,
    /// Indentation levels whose lines are table rows.
    tables: Vec<usize>,
    /// Row level for a table header on the previous line.
    pending_table: Option<usize>,
    /// The previous line was `- key:`, which opens two levels at once.
    double_indent: bool,
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

    let mut lexer = Lexer {
        lines: input.lines().collect(),
        next_line: 0,
        tokens: Vec::new(),
        elements: 0,
        indents: vec![0],
        tables: Vec::new(),
        pending_table: None,
        double_indent: false,
    };

    while lexer.next_line < lexer.lines.len() {
        let index = lexer.next_line;
        lexer.next_line += 1;
        lexer.line(lexer.lines[index], index + 1)?;
    }

    let eof_line = lexer.lines.len() + 1;

    while lexer.indents.len() > 1 {
        lexer.indents.pop();
        lexer.tokens.push(token(TokenKind::Dedent, eof_line, 1));
    }

    lexer.tokens.push(token(TokenKind::Eof, eof_line, 1));
    if lexer.budget() > MAX_TOKENS {
        return Err(error(
            ErrorKind::SyntaxError,
            "Maximum token count exceeded",
            eof_line,
            1,
            1,
        ));
    }

    Ok(lexer.tokens)
}

impl Lexer<'_> {
    fn budget(&self) -> usize {
        self.tokens.len() + self.elements
    }

    fn line(&mut self, raw: &str, line: usize) -> Result<(), AxsError> {
        if let Some(tab_position) = raw.chars().position(|c| c == '\t') {
            return Err(error(
                ErrorKind::InvalidIndentation,
                "Tabs are forbidden",
                line,
                tab_position + 1,
                1,
            )
            .with_help(
                "Use two spaces per indentation level; tabs are allowed only inside text blocks",
            ));
        }

        let trimmed = raw.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
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

        self.indent(depth, line, spaces)?;

        let text = raw[spaces..].trim_end();
        let column = spaces + 1;

        if self.tables.last() == Some(&depth) {
            self.row(text, line, column)?;
        } else if text == "-" {
            self.tokens.push(token(TokenKind::Dash, line, column));
        } else if let Some(after_dash) = text.strip_prefix("- ") {
            self.tokens.push(token(TokenKind::Dash, line, column));
            let item = after_dash.trim_start();
            let item_column = column + 2 + (after_dash.len() - item.len());
            match parse_field(item) {
                Ok(field) => self.field(field, depth + 1, true, line, item_column)?,
                Err(_) => self.value(item, depth, line, item_column)?,
            }
        } else {
            let field = parse_field(text).map_err(|failure| {
                error(
                    failure.kind,
                    &failure.message,
                    line,
                    column,
                    text.chars().count().max(1),
                )
                .with_help(failure.help)
            })?;
            self.field(field, depth, false, line, column)?;
        }

        self.tokens
            .push(token(TokenKind::Newline, line, raw.chars().count() + 1));
        if self.budget() >= MAX_TOKENS {
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
        Ok(())
    }

    fn indent(&mut self, depth: usize, line: usize, spaces: usize) -> Result<(), AxsError> {
        let current = *self.indents.last().unwrap();
        let double_indent = std::mem::take(&mut self.double_indent);
        let pending_table = self.pending_table.take();

        match depth.cmp(&current) {
            std::cmp::Ordering::Greater => {
                if depth != current + 1 && !(double_indent && depth == current + 2) {
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
                    self.indents.push(level);
                    self.tokens
                        .push(token(TokenKind::Indent, line, level * 2 + 1));
                }

                if pending_table == Some(depth) {
                    self.tables.push(depth);
                }
            }
            std::cmp::Ordering::Less => {
                if !self.indents.contains(&depth) {
                    return Err(error(
                        ErrorKind::InvalidIndentation,
                        "Invalid dedentation",
                        line,
                        spaces + 1,
                        1,
                    ));
                }
                while *self.indents.last().unwrap() > depth {
                    self.indents.pop();
                    self.tokens.push(token(TokenKind::Dedent, line, spaces + 1));
                }
                while self.tables.last().is_some_and(|level| *level > depth) {
                    self.tables.pop();
                }
            }
            std::cmp::Ordering::Equal => {}
        }
        Ok(())
    }

    fn row(&mut self, text: &str, line: usize, column: usize) -> Result<(), AxsError> {
        let Some(cells) = text.strip_prefix("- ") else {
            return Err(error(
                ErrorKind::InvalidCollection,
                "Expected a table row",
                line,
                column,
                text.chars().count().max(1),
            )
            .with_help("Write table rows as '- value, value'"));
        };
        self.tokens.push(token(TokenKind::Dash, line, column));
        self.items(cells, line, column + 2)
    }

    fn items(&mut self, text: &str, line: usize, column: usize) -> Result<(), AxsError> {
        let parts = split_top_level(text).ok_or_else(|| {
            error(
                ErrorKind::InvalidString,
                "Unterminated quoted string",
                line,
                column,
                text.chars().count().max(1),
            )
        })?;
        let items: Vec<String> = parts.iter().map(|part| part.trim().to_owned()).collect();
        self.elements += items.len();
        self.tokens
            .push(token(TokenKind::Items(items), line, column));
        Ok(())
    }

    fn field(
        &mut self,
        field: Field<'_>,
        level: usize,
        dash_line: bool,
        line: usize,
        column: usize,
    ) -> Result<(), AxsError> {
        self.tokens
            .push(token(TokenKind::Identifier(field.key), line, column));
        if let Some(length) = field.length {
            self.tokens.push(token(
                TokenKind::Length(length),
                line,
                column + field.key_chars,
            ));
        }
        let is_table = field.columns.is_some();
        if let Some(columns) = field.columns {
            self.elements += columns.len();
            self.tokens.push(token(
                TokenKind::Columns(columns),
                line,
                column + field.key_chars,
            ));
        }
        self.tokens
            .push(token(TokenKind::Colon, line, column + field.colon));

        match field.value {
            None => {
                self.double_indent = dash_line;
                if is_table {
                    self.pending_table = Some(level + 1);
                }
                Ok(())
            }
            Some((value, offset)) => self.value(value, level, line, column + offset),
        }
    }

    /// A field or item value; `level` is the level of the line's owner.
    fn value(
        &mut self,
        value: &str,
        level: usize,
        line: usize,
        column: usize,
    ) -> Result<(), AxsError> {
        if value == "|" {
            let text = self.text_block(level + 1).ok_or_else(|| {
                error(ErrorKind::InvalidValue, "Empty text block", line, column, 1)
                    .with_help("Indent the text one level deeper than its key, or use \"\"")
            })?;
            self.tokens.push(token(TokenKind::Text(text), line, column));
        } else if let Some(inner) = value.strip_prefix('[') {
            let inner = inner.strip_suffix(']').ok_or_else(|| {
                error(
                    ErrorKind::InvalidCollection,
                    "Malformed inline array",
                    line,
                    column,
                    value.chars().count(),
                )
                .with_help("Close the array with ']', or quote a string starting with '['")
            })?;
            if inner.trim().is_empty() {
                self.tokens
                    .push(token(TokenKind::Items(Vec::new()), line, column));
            } else {
                self.items(inner, line, column + 1)?;
            }
        } else {
            self.tokens
                .push(token(TokenKind::Value(value.into()), line, column));
        }
        Ok(())
    }

    /// Consumes the content lines of a text block at `level`.
    fn text_block(&mut self, level: usize) -> Option<String> {
        let prefix = level * 2;
        let mut lines: Vec<&str> = Vec::new();
        let mut blanks = 0;
        while let Some(raw) = self.lines.get(self.next_line) {
            if raw.trim().is_empty() {
                blanks += 1;
            } else if raw.len() >= prefix && raw.as_bytes()[..prefix].iter().all(|b| *b == b' ') {
                lines.extend(std::iter::repeat_n("", blanks));
                blanks = 0;
                lines.push(&raw[prefix..]);
            } else {
                break;
            }
            self.next_line += 1;
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }
}
