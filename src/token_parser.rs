use std::collections::HashMap;

use crate::error::{AxsError, ErrorKind, SourceSpan};
use crate::lexer::{Token, TokenKind, tokenize};
use crate::value::AxsValue;
use crate::value_parser::parse_value;

type AxsObject = HashMap<String, AxsValue>;

pub fn parse_tokens(source: &str) -> Result<AxsObject, AxsError> {
    let tokens = tokenize(source)?;
    let mut parser = TokenParser::new(&tokens);

    let object = parser.parse_object()?;
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
            err = err.with_span(SourceSpan::new(token.span.line, token.span.column, 1));
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
            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected identifier")),
        }
    }

    fn read_value(&mut self) -> Result<AxsValue, AxsError> {
        match self.kind() {
            Some(TokenKind::Value(raw)) => {
                let value = parse_value(raw).map_err(|message| {
                    let kind = if raw.starts_with('"') {
                        ErrorKind::InvalidString
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

    fn parse_object(&mut self) -> Result<AxsObject, AxsError> {
        let mut object = HashMap::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
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

                    let value = self.parse_block()?;

                    self.expect_dedent()?;
                    value
                }

                _ => {
                    return Err(
                        self.error(ErrorKind::UnexpectedToken, "Expected '=' or nested block")
                    );
                }
            };

            self.insert_field(&mut object, key, value, key_span)?;
        }

        Ok(object)
    }

    fn parse_block(&mut self) -> Result<AxsValue, AxsError> {
        match self.kind() {
            Some(TokenKind::Dash) => Ok(AxsValue::Array(self.parse_array()?)),
            Some(TokenKind::Identifier(_)) => Ok(AxsValue::Object(self.parse_object()?)),
            _ => Err(self.error(ErrorKind::UnexpectedToken, "Expected object or array block")),
        }
    }

    fn parse_array(&mut self) -> Result<Vec<AxsValue>, AxsError> {
        let mut array = Vec::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            if !matches!(self.kind(), Some(TokenKind::Dash)) {
                return Err(self.error(ErrorKind::UnexpectedToken, "Expected '-'"));
            }

            self.advance();

            let value = match self.kind() {
                Some(TokenKind::Value(_)) => {
                    let value = self.read_value()?;
                    self.expect_newline()?;
                    value
                }

                Some(TokenKind::Newline) => {
                    self.expect_newline()?;
                    self.expect_indent()?;

                    let value = self.parse_block()?;

                    self.expect_dedent()?;
                    value
                }

                Some(TokenKind::Identifier(_)) => self.parse_inline_object()?,

                _ => {
                    return Err(self.error(ErrorKind::UnexpectedToken, "Invalid array element"));
                }
            };

            array.push(value);
        }

        Ok(array)
    }

    fn parse_inline_object(&mut self) -> Result<AxsValue, AxsError> {
        let mut object = HashMap::new();

        let key_span = self.identifier_span();
        let key = self.read_identifier()?;

        self.expect_equals()?;

        let value = self.read_value()?;
        self.expect_newline()?;

        self.insert_field(&mut object, key, value, key_span)?;

        if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.expect_indent()?;

            let extra = self.parse_object()?;
            self.expect_dedent()?;

            for (key, value) in extra {
                self.insert_field(&mut object, key, value, key_span)?;
            }
        }

        Ok(AxsValue::Object(object))
    }
}
