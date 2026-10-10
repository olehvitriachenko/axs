use std::collections::HashMap;

use crate::error::{AxsError, ErrorKind, SourceSpan};
use crate::lexer::{MAX_NESTING_DEPTH, Token, TokenKind, tokenize};
use crate::value::AxsValue;
use crate::value_parser::{is_numeric_literal, parse_value};

type AxsObject = HashMap<String, AxsValue>;

pub fn parse_tokens(source: &str) -> Result<AxsObject, AxsError> {
    let tokens = tokenize(source)?;
    let mut parser = TokenParser::new(&tokens);

    let object = parser.parse_object(0)?;
    parser.expect_eof()?;

    Ok(object)
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

    fn error(&self, kind: ErrorKind, message: &str) -> AxsError {
        let mut err = AxsError::new(kind, message);

        if let Some(token) = self.current() {
            err = err.with_span(SourceSpan::new(
                token.span.line,
                token.span.column,
                match &token.kind {
                    TokenKind::Identifier(raw) | TokenKind::Value(raw) => {
                        raw.chars().count().max(1)
                    }
                    _ => 1,
                },
            ));
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
                .with_help("Indent the nested content by two spaces"))
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
            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected identifier")),
        }
    }

    fn read_value(&mut self) -> Result<AxsValue, AxsError> {
        match self.kind() {
            Some(TokenKind::Value(raw)) => {
                let value = parse_value(raw).map_err(|message| {
                    let kind = if raw.starts_with('"') {
                        ErrorKind::InvalidString
                    } else if is_numeric_literal(raw) {
                        ErrorKind::InvalidNumber
                    } else {
                        ErrorKind::InvalidValue
                    };

                    self.error(kind, &message)
                })?;

                self.advance();
                Ok(value)
            }
            Some(TokenKind::Eof) | None => {
                Err(self.error(ErrorKind::UnexpectedEof, "Expected value"))
            }
            _ => Err(self.error(ErrorKind::InvalidValue, "Expected value")),
        }
    }

    fn expect_equals(&mut self) -> Result<(), AxsError> {
        if matches!(self.kind(), Some(TokenKind::Equals)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(ErrorKind::UnexpectedToken, "Expected '='"))
        }
    }

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

    fn identifier_span(&self) -> SourceSpan {
        match self.current() {
            Some(token) => {
                let length = match &token.kind {
                    TokenKind::Identifier(name) => name.chars().count(),
                    _ => 1,
                };

                SourceSpan::new(token.span.line, token.span.column, length)
            }
            None => SourceSpan::new(1, 1, 1),
        }
    }

    fn check_depth(&self, depth: usize) -> Result<(), AxsError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(self.error(ErrorKind::InvalidIndentation, "Maximum nesting depth exceeded")
                .with_help(format!("AXS supports at most {MAX_NESTING_DEPTH} nesting levels, including inline objects")));
        }
        Ok(())
    }

    fn parse_object(&mut self, depth: usize) -> Result<AxsObject, AxsError> {
        let mut object = HashMap::new();
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
            let key_span = self.identifier_span();
            let key = self.read_identifier()?;

            let value = match self.kind() {
                Some(TokenKind::Equals) => {
                    self.expect_equals()?;

                    let value = self.read_value()?;
                    self.expect_newline()?;

                    value
                }

                Some(TokenKind::Newline) => {
                    self.expect_newline()?;
                    self.expect_indent()?;

                    let value = self.parse_block(depth + 1)?;

                    self.expect_dedent()?;
                    value
                }

                _ => {
                    return Err(
                        self.error(ErrorKind::UnexpectedToken, "Expected '=' or nested block")
                    );
                }
            };

            self.insert_field(object, key, value, key_span)?;
        }

        Ok(())
    }

    fn parse_block(&mut self, depth: usize) -> Result<AxsValue, AxsError> {
        match self.kind() {
            Some(TokenKind::Dash) => Ok(AxsValue::Array(self.parse_array(depth)?)),

            Some(TokenKind::Identifier(_)) => Ok(AxsValue::Object(self.parse_object(depth)?)),

            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected object or array block")),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Vec<AxsValue>, AxsError> {
        self.check_depth(depth)?;
        let mut array = Vec::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            if !matches!(self.kind(), Some(TokenKind::Dash)) {
                let kind = if matches!(self.kind(), Some(TokenKind::Indent)) {
                    ErrorKind::InvalidIndentation
                } else {
                    ErrorKind::UnexpectedToken
                };
                return Err(self.error(kind, "Expected '-' at the array indentation level"));
            }

            self.advance();

            let value = match self.kind() {
                // - Rust
                Some(TokenKind::Value(_)) => {
                    let value = self.read_value()?;
                    self.expect_newline()?;
                    value
                }

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

                // - name = Oleh
                //   age = 22
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
        let mut object = HashMap::new();

        let key_span = self.identifier_span();
        let key = self.read_identifier()?;

        self.expect_equals()?;

        let value = self.read_value()?;
        self.expect_newline()?;

        self.insert_field(&mut object, key, value, key_span)?;

        // Read additional object fields directly into the
        // same HashMap instead of creating a temporary one.
        if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.expect_indent()?;

            self.parse_object_into(&mut object, depth)?;

            self.expect_dedent()?;
        }

        Ok(AxsValue::Object(object))
    }
}
