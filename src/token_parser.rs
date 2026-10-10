use std::collections::HashMap;

use crate::lexer::{Token, TokenKind, tokenize};
use crate::value::AxsValue;
use crate::value_parser::parse_value;

type AxsObject = HashMap<String, AxsValue>;

pub fn parse_tokens(source: &str) -> Result<AxsObject, String> {
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

    fn error(&self, message: &str) -> String {
        match self.current() {
            Some(token) => format!(
                "Line {}, column {}: {}",
                token.span.line, token.span.column, message
            ),
            None => format!("Unexpected EOF: {message}"),
        }
    }

    fn expect_newline(&mut self) -> Result<(), String> {
        if matches!(self.kind(), Some(TokenKind::Newline)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error("Expected newline"))
        }
    }

    fn expect_indent(&mut self) -> Result<(), String> {
        if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error("Expected indentation"))
        }
    }

    fn expect_dedent(&mut self) -> Result<(), String> {
        if matches!(self.kind(), Some(TokenKind::Dedent)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error("Expected dedentation"))
        }
    }

    fn expect_eof(&mut self) -> Result<(), String> {
        if matches!(self.kind(), Some(TokenKind::Eof)) {
            Ok(())
        } else {
            Err(self.error("Expected end of document"))
        }
    }

    fn read_identifier(&mut self) -> Result<String, String> {
        match self.kind() {
            Some(TokenKind::Identifier(name)) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(self.error("Expected identifier")),
        }
    }

    fn read_value(&mut self) -> Result<AxsValue, String> {
        match self.kind() {
            Some(TokenKind::Value(raw)) => {
                let value = parse_value(raw).map_err(|e| self.error(&e))?;

                self.advance();
                Ok(value)
            }
            _ => Err(self.error("Expected value")),
        }
    }

    fn expect_equals(&mut self) -> Result<(), String> {
        if matches!(self.kind(), Some(TokenKind::Equals)) {
            self.advance();
            Ok(())
        } else {
            Err(self.error("Expected '='"))
        }
    }

    fn insert_field(
        &self,
        object: &mut AxsObject,
        key: String,
        value: AxsValue,
    ) -> Result<(), String> {
        if object.contains_key(&key) {
            return Err(self.error(&format!("Duplicate key '{key}'")));
        }

        object.insert(key, value);
        Ok(())
    }

    fn parse_object(&mut self) -> Result<AxsObject, String> {
        let mut object = HashMap::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
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

                _ => return Err(self.error("Expected '=' or nested block")),
            };

            self.insert_field(&mut object, key, value)?;
        }

        Ok(object)
    }

    fn parse_block(&mut self) -> Result<AxsValue, String> {
        match self.kind() {
            Some(TokenKind::Dash) => Ok(AxsValue::Array(self.parse_array()?)),
            Some(TokenKind::Identifier(_)) => Ok(AxsValue::Object(self.parse_object()?)),
            _ => Err(self.error("Expected object or array block")),
        }
    }

    fn parse_array(&mut self) -> Result<Vec<AxsValue>, String> {
        let mut array = Vec::new();

        while !matches!(self.kind(), Some(TokenKind::Dedent | TokenKind::Eof) | None) {
            if !matches!(self.kind(), Some(TokenKind::Dash)) {
                return Err(self.error("Expected '-'"));
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
                //   name = Oleh
                Some(TokenKind::Newline) => {
                    self.expect_newline()?;
                    self.expect_indent()?;

                    let value = self.parse_block()?;

                    self.expect_dedent()?;
                    value
                }

                // - name = Oleh
                //   age = 22
                Some(TokenKind::Identifier(_)) => self.parse_inline_object()?,

                _ => return Err(self.error("Invalid array element")),
            };

            array.push(value);
        }

        Ok(array)
    }

    fn parse_inline_object(&mut self) -> Result<AxsValue, String> {
        let mut object = HashMap::new();

        let key = self.read_identifier()?;
        self.expect_equals()?;

        let value = self.read_value()?;
        self.expect_newline()?;

        self.insert_field(&mut object, key, value)?;

        // Additional fields:
        //
        // - name = Oleh
        //   age = 22
        if matches!(self.kind(), Some(TokenKind::Indent)) {
            self.expect_indent()?;

            let extra = self.parse_object()?;
            self.expect_dedent()?;

            for (key, value) in extra {
                self.insert_field(&mut object, key, value)?;
            }
        }

        Ok(AxsValue::Object(object))
    }
}
