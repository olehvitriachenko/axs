use crate::error::{AxsError, ErrorKind, SourceSpan};
use crate::lexer::{MAX_NESTING_DEPTH, Token, TokenKind, tokenize};
use crate::value::{AxsObject, AxsValue};
use crate::value_parser::{is_numeric_literal, parse_value};

pub fn parse_tokens(source: &str) -> Result<AxsObject, AxsError> {
    let tokens = tokenize(source)?;
    let mut parser = TokenParser::new(&tokens);

    let object = parser.parse_object(0)?;
    parser.expect_eof()?;

    Ok(object)
}

const PRIMITIVES_ONLY: &str =
    "Inline arrays, inline objects, and table cells hold only primitive values";

/// The optional `[N]` and `{a, b}` parts of a field header.
struct Header {
    length: Option<(usize, SourceSpan)>,
    columns: Option<(Vec<String>, SourceSpan)>,
    colon: SourceSpan,
}

struct TokenParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> TokenParser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn kind(&self) -> Option<&'a TokenKind> {
        self.current().map(|token| &token.kind)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn span(&self) -> SourceSpan {
        match self.current() {
            Some(token) => SourceSpan::new(
                token.span.line,
                token.span.column,
                match &token.kind {
                    TokenKind::Identifier(raw) | TokenKind::Value(raw) => {
                        raw.chars().count().max(1)
                    }
                    _ => 1,
                },
            ),
            None => SourceSpan::new(1, 1, 1),
        }
    }

    #[cold]
    #[inline(never)]
    fn error(&self, kind: ErrorKind, message: &str) -> AxsError {
        let mut err = AxsError::new(kind, message);

        if self.current().is_some() {
            err = err.with_span(self.span());
        }

        err
    }

    fn expect_newline(&mut self) -> Result<(), AxsError> {
        if matches!(self.kind(), Some(TokenKind::Newline)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(ErrorKind::UnexpectedToken, "Expected newline"))
        }
    }

    fn expect_indent(&mut self) -> Result<(), AxsError> {
        if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.advance();
            Ok(())
        } else {
            Err(self
                .error(ErrorKind::InvalidIndentation, "Expected nested block")
                .with_help("Indent the nested content by two spaces; write empty collections as 'key: {}' or 'key: []'"))
        }
    }

    fn expect_dedent(&mut self) -> Result<(), AxsError> {
        if matches!(self.kind(), Some(TokenKind::Dedent)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(ErrorKind::InvalidIndentation, "Expected dedentation"))
        }
    }

    fn expect_eof(&mut self) -> Result<(), AxsError> {
        if matches!(self.kind(), Some(TokenKind::Eof)) {
            Ok(())
        } else {
            Err(self.error(ErrorKind::UnexpectedToken, "Expected end of document"))
        }
    }

    fn read_identifier(&mut self) -> Result<String, AxsError> {
        match self.kind() {
            Some(TokenKind::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            Some(TokenKind::Eof) | None => {
                Err(self.error(ErrorKind::UnexpectedEof, "Expected identifier"))
            }
            Some(TokenKind::Indent) => {
                Err(self.error(ErrorKind::InvalidIndentation, "Unexpected indentation"))
            }
            Some(TokenKind::Dash) => Err(self.error(
                ErrorKind::UnexpectedToken,
                "Cannot mix array items and object fields in one block",
            )),
            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected identifier")),
        }
    }

    fn scalar(&self, raw: &str) -> Result<AxsValue, AxsError> {
        parse_value(raw).map_err(|message| {
            let kind = if raw.starts_with('"') {
                ErrorKind::InvalidString
            } else if is_numeric_literal(raw) {
                ErrorKind::InvalidNumber
            } else {
                ErrorKind::InvalidValue
            };

            self.error(kind, &message)
        })
    }

    /// Inline array elements and table cells: primitives only.
    fn elements(&self, raw: &[String]) -> Result<Vec<AxsValue>, AxsError> {
        raw.iter()
            .map(|item| {
                if item.is_empty() {
                    return Err(self
                        .error(ErrorKind::InvalidCollection, "Empty element")
                        .with_help("Remove the extra ',' or write an empty string as \"\""));
                }
                if !item.starts_with('"') && item.contains(['[', ']', '{', '}']) {
                    return Err(self
                        .error(ErrorKind::InvalidCollection, PRIMITIVES_ONLY)
                        .with_help("Use a block for nested collections, or quote the string"));
                }
                match self.scalar(item)? {
                    AxsValue::Array(_) | AxsValue::Object(_) => {
                        Err(self.error(ErrorKind::InvalidCollection, PRIMITIVES_ONLY))
                    }
                    value => Ok(value),
                }
            })
            .collect()
    }

    fn inline_object(&self, fields: &[(String, String)]) -> Result<AxsObject, AxsError> {
        let raw: Vec<String> = fields.iter().map(|(_, value)| value.clone()).collect();
        let values = self.elements(&raw)?;
        let mut object = AxsObject::new();
        for ((key, _), value) in fields.iter().zip(values) {
            if object.contains_key(key) {
                return Err(self.error(
                    ErrorKind::DuplicateKey,
                    &format!("Duplicate key '{key}' in inline object"),
                ));
            }
            object.insert(key.clone(), value);
        }
        Ok(object)
    }

    /// A scalar, inline array, inline object, or text block value followed by
    /// its newline.
    fn read_value(&mut self) -> Result<AxsValue, AxsError> {
        let value = match self.kind() {
            Some(TokenKind::Value(raw)) => self.scalar(raw)?,
            Some(TokenKind::Items(items)) => AxsValue::Array(self.elements(items)?),
            Some(TokenKind::Fields(fields)) => AxsValue::Object(self.inline_object(fields)?),
            Some(TokenKind::Text(text)) => AxsValue::String(text.clone()),
            Some(TokenKind::Eof) | None => {
                return Err(self.error(ErrorKind::UnexpectedEof, "Expected value"));
            }
            _ => return Err(self.error(ErrorKind::InvalidValue, "Expected value")),
        };
        self.advance();
        self.expect_newline()?;
        Ok(value)
    }

    #[inline(never)]
    fn insert_field(
        &self,
        object: &mut AxsObject,
        key: String,
        value: AxsValue,
        key_span: SourceSpan,
    ) -> Result<(), AxsError> {
        if object.contains_key(&key) {
            return Err(
                AxsError::new(ErrorKind::DuplicateKey, format!("Duplicate key '{key}'"))
                    .with_span(key_span)
                    .with_help("Each key must be unique within the same object"),
            );
        }

        object.insert(key, value);
        Ok(())
    }

    fn check_depth(&self, depth: usize) -> Result<(), AxsError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(self.error(ErrorKind::InvalidIndentation, "Maximum nesting depth exceeded")
                .with_help(format!("AXS supports at most {MAX_NESTING_DEPTH} nesting levels, including inline objects")));
        }
        Ok(())
    }

    fn parse_object(&mut self, depth: usize) -> Result<AxsObject, AxsError> {
        let mut object = AxsObject::new();
        self.parse_object_into(&mut object, depth)?;
        Ok(object)
    }

    // Parse fields directly into an existing object.
    // This preserves source positions for duplicate keys.
    fn parse_object_into(&mut self, object: &mut AxsObject, depth: usize) -> Result<(), AxsError> {
        self.check_depth(depth)?;
        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            if matches!(self.kind(), Some(TokenKind::Indent)) {
                return Err(self.error(ErrorKind::InvalidIndentation, "Unexpected indentation"));
            }
            let key_span = self.span();
            let key = self.read_identifier()?;
            let (value, _) = self.parse_field_value(depth, false)?;
            self.insert_field(object, key, value, key_span)?;
        }

        Ok(())
    }

    #[inline(never)]
    fn read_header(&mut self) -> Result<Header, AxsError> {
        let mut header = Header {
            length: None,
            columns: None,
            colon: SourceSpan::new(1, 1, 1),
        };
        if let Some(TokenKind::Length(length)) = self.kind() {
            header.length = Some((*length, self.span()));
            self.advance();
        }
        if let Some(TokenKind::Columns(columns)) = self.kind() {
            header.columns = Some((columns.clone(), self.span()));
            self.advance();
        }
        if !matches!(self.kind(), Some(TokenKind::Colon)) {
            return Err(self.error(ErrorKind::UnexpectedToken, "Expected ':'"));
        }
        header.colon = self.span();
        self.advance();
        Ok(header)
    }

    /// Parses everything after a field's key. `inline` marks the first field
    /// of an object written on a dash line, whose block content sits two levels
    /// deeper. Returns the value and whether it was a block header.
    ///
    /// This is on the recursion path for every nesting level, so all
    /// non-recursive work lives in separate, never-inlined helpers to keep
    /// the frame small.
    fn parse_field_value(
        &mut self,
        depth: usize,
        inline: bool,
    ) -> Result<(AxsValue, bool), AxsError> {
        let header = self.read_header()?;

        if !matches!(self.kind(), Some(TokenKind::Newline)) {
            return self.parse_line_value(&header).map(|value| (value, false));
        }

        self.open_block(&header, inline)?;
        let value = match &header.columns {
            Some((columns, _)) => AxsValue::Array(self.parse_rows(columns, depth + 1)?),
            None => self.parse_block(depth + 1)?,
        };
        self.close_block(&header, &value)?;
        Ok((value, true))
    }

    /// A value on the field's own line: scalar, inline array, or text block.
    #[inline(never)]
    fn parse_line_value(&mut self, header: &Header) -> Result<AxsValue, AxsError> {
        if let Some((_, span)) = header.columns {
            return Err(AxsError::new(
                ErrorKind::InvalidCollection,
                "A table header must be followed by indented rows",
            )
            .with_span(span));
        }
        let length_allowed = matches!(self.kind(), Some(TokenKind::Items(_)));
        if let (Some((_, span)), false) = (header.length, length_allowed) {
            return Err(AxsError::new(
                ErrorKind::InvalidCollection,
                "An array length requires an array value",
            )
            .with_span(span));
        }
        let value = self.read_value()?;
        check_length(header, &value)?;
        Ok(value)
    }

    /// Consumes the header line's newline and the block's indentation.
    #[inline(never)]
    fn open_block(&mut self, header: &Header, inline: bool) -> Result<(), AxsError> {
        self.expect_newline()?;
        if !matches!(self.kind(), Some(TokenKind::Indent)) {
            return Err(AxsError::new(
                ErrorKind::InvalidValue,
                "Expected a value or an indented block after ':'",
            )
            .with_span(header.colon)
            .with_help("Write 'key: value', or indent the nested content by two spaces; write empty collections as 'key: {}' or 'key: []'"));
        }
        self.expect_indent()?;
        if inline {
            // The inline object's fields live one level below the dash; its
            // first named block must live one further level below that.
            if !matches!(self.kind(), Some(TokenKind::Indent)) {
                return Err(self.error(
                    ErrorKind::InvalidIndentation,
                    "Inline named block content must be four spaces deeper than '-'",
                ).with_help("Inline object fields are two spaces below '-', and their named block contents are four spaces below '-'"));
            }
            self.expect_indent()?;
        }

        if let Some((columns, span)) = &header.columns {
            for (index, column) in columns.iter().enumerate() {
                if columns[..index].contains(column) {
                    return Err(AxsError::new(
                        ErrorKind::InvalidCollection,
                        format!("Duplicate table field '{column}'"),
                    )
                    .with_span(*span));
                }
            }
        } else if header.length.is_some() && !matches!(self.kind(), Some(TokenKind::Dash)) {
            return Err(self
                .error(
                    ErrorKind::InvalidCollection,
                    "An array length requires an array block",
                )
                .with_help("Remove the length from an object header"));
        }
        Ok(())
    }

    #[inline(never)]
    fn close_block(&mut self, header: &Header, value: &AxsValue) -> Result<(), AxsError> {
        self.expect_dedent()?;
        check_length(header, value)
    }

    fn parse_block(&mut self, depth: usize) -> Result<AxsValue, AxsError> {
        match self.kind() {
            Some(TokenKind::Dash) => Ok(AxsValue::Array(self.parse_array(depth)?)),

            Some(TokenKind::Identifier(_)) => Ok(AxsValue::Object(self.parse_object(depth)?)),

            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected object or array block")),
        }
    }

    fn parse_rows(&mut self, columns: &[String], depth: usize) -> Result<Vec<AxsValue>, AxsError> {
        self.check_depth(depth)?;
        let mut rows = Vec::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            let Some(TokenKind::Items(cells)) = self.kind() else {
                return Err(self.error(ErrorKind::InvalidCollection, "Expected a table row"));
            };
            if cells.len() != columns.len() {
                return Err(self.error(
                    ErrorKind::InvalidCollection,
                    &format!(
                        "Table row has {} cells, but the header lists {} fields",
                        cells.len(),
                        columns.len()
                    ),
                ));
            }
            let values = self.elements(cells)?;
            self.advance();
            self.expect_newline()?;

            rows.push(AxsValue::Object(
                columns.iter().cloned().zip(values).collect(),
            ));
        }

        Ok(rows)
    }

    fn parse_array(&mut self, depth: usize) -> Result<Vec<AxsValue>, AxsError> {
        self.check_depth(depth)?;
        let mut array = Vec::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            if !matches!(self.kind(), Some(TokenKind::Dash)) {
                let (kind, message) = match self.kind() {
                    Some(TokenKind::Indent) => (
                        ErrorKind::InvalidIndentation,
                        "Expected '-' at the array indentation level",
                    ),
                    Some(TokenKind::Identifier(_)) => (
                        ErrorKind::UnexpectedToken,
                        "Cannot mix array items and object fields in one block",
                    ),
                    _ => (
                        ErrorKind::UnexpectedToken,
                        "Expected '-' at the array indentation level",
                    ),
                };
                return Err(self.error(kind, message));
            }

            self.advance();

            let value = match self.kind() {
                // - Rust, - [a, b], - {a: 1}, - |
                Some(
                    TokenKind::Value(_)
                    | TokenKind::Items(_)
                    | TokenKind::Fields(_)
                    | TokenKind::Text(_),
                ) => self.read_value()?,

                // -
                //   - Rust
                //   - Go
                Some(TokenKind::Newline) => {
                    self.expect_newline()?;
                    self.expect_indent()?;

                    let value = self.parse_block(depth + 1)?;

                    self.expect_dedent()?;
                    value
                }

                // - name: Oleh
                //   age: 22
                Some(TokenKind::Identifier(_)) => self.parse_inline_object(depth + 1)?,

                _ => {
                    return Err(self.error(ErrorKind::UnexpectedToken, "Invalid array element"));
                }
            };

            array.push(value);
        }

        Ok(array)
    }

    fn parse_inline_object(&mut self, depth: usize) -> Result<AxsValue, AxsError> {
        self.check_depth(depth)?;
        let mut object = AxsObject::new();

        let key_span = self.span();
        let key = self.read_identifier()?;
        let (value, header) = self.parse_field_value(depth, true)?;
        self.insert_field(&mut object, key, value, key_span)?;

        // A block header leaves the implicit object level open; otherwise the
        // continuation fields, if any, open it now. Fields go directly into
        // the same object to preserve duplicate spans.
        if header {
            self.parse_object_into(&mut object, depth)?;
            self.expect_dedent()?;
        } else if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.expect_indent()?;
            self.parse_object_into(&mut object, depth)?;
            self.expect_dedent()?;
        }

        Ok(AxsValue::Object(object))
    }
}

fn check_length(header: &Header, value: &AxsValue) -> Result<(), AxsError> {
    match (header.length, value) {
        (Some((expected, span)), AxsValue::Array(items)) if items.len() != expected => {
            Err(AxsError::new(
                ErrorKind::LengthMismatch,
                format!(
                    "Declared length {expected}, but the array has {} elements",
                    items.len()
                ),
            )
            .with_span(span)
            .with_help("The output may be truncated; fix the length or the elements"))
        }
        _ => Ok(()),
    }
}
