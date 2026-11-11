//! AXS queries: select, filter, and project parts of a document before it
//! reaches a model. See `spec/QUERY.md`.
//!
//! ```text
//! users                      the whole array
//! users[0]  users[-1]        one element
//! users[age > 20]{name}      filter, then keep only `name`
//! users.name                 `name` of every element
//! issues[state == open and labels ~ bug]{number,title}
//! ```

use crate::error::{AxsError, ErrorKind, SourceSpan};
use crate::lexer::valid_key;
use crate::string;
use crate::value::{AxsObject, AxsValue};
use crate::value_parser::parse_value;

/// Maximum query length in characters.
pub const MAX_QUERY_CHARS: usize = 4096;
/// Maximum nesting of parentheses in a condition.
pub const MAX_CONDITION_DEPTH: usize = 32;

/// Evaluates `query` against `document`.
pub fn query(document: &AxsObject, query: &str) -> Result<AxsValue, AxsError> {
    let steps = parse(query)?;
    let mut value = AxsValue::Object(document.clone());
    for step in &steps {
        value = step.apply(value)?;
    }
    Ok(value)
}

#[derive(Debug)]
enum Step {
    Field(String, SourceSpan),
    Index(i64, SourceSpan),
    All(SourceSpan),
    Filter(Condition, SourceSpan),
    Project(Vec<String>, SourceSpan),
}

#[derive(Debug)]
enum Condition {
    Or(Box<Condition>, Box<Condition>),
    And(Box<Condition>, Box<Condition>),
    Compare(Vec<String>, Operator, AxsValue),
}

#[derive(Debug, Clone, Copy)]
enum Operator {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Contains,
}

fn query_error(message: impl Into<String>, column: usize, length: usize) -> AxsError {
    AxsError::new(ErrorKind::InvalidQuery, message).with_span(SourceSpan::new(
        1,
        column,
        length.max(1),
    ))
}

fn parse(text: &str) -> Result<Vec<Step>, AxsError> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() > MAX_QUERY_CHARS {
        return Err(query_error(
            format!("Queries are limited to {MAX_QUERY_CHARS} characters"),
            1,
            1,
        ));
    }
    let mut parser = Parser { chars, pos: 0 };
    parser.skip_spaces();
    let mut steps = Vec::new();

    // `.` alone (or an empty query) selects the whole document.
    if parser.eat('.') {
        parser.skip_spaces();
        if !parser.done() {
            let (name, span) = parser.segment()?;
            steps.push(Step::Field(name, span));
        }
    } else if !parser.done() && !matches!(parser.peek(), Some('[' | '{')) {
        let (name, span) = parser.segment()?;
        steps.push(Step::Field(name, span));
    }

    loop {
        parser.skip_spaces();
        let start = parser.column();
        match parser.peek() {
            None => break,
            Some('.') => {
                parser.pos += 1;
                parser.skip_spaces();
                let (name, span) = parser.segment()?;
                steps.push(Step::Field(name, span));
            }
            Some('[') => {
                parser.pos += 1;
                steps.push(parser.selector(start)?);
            }
            Some('{') => {
                parser.pos += 1;
                let mut names = Vec::new();
                loop {
                    parser.skip_spaces();
                    names.push(parser.segment()?.0);
                    parser.skip_spaces();
                    if parser.eat(',') {
                        continue;
                    }
                    if parser.eat('}') {
                        break;
                    }
                    return Err(parser.error("Expected ',' or '}' in a field list"));
                }
                let span = SourceSpan::new(1, start, parser.column() - start);
                steps.push(Step::Project(names, span));
            }
            Some(_) => {
                return Err(parser
                    .error("Expected '.', '[', or '{'")
                    .with_help("Chain steps as in users[age > 20]{name}"));
            }
        }
    }
    Ok(steps)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn done(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn column(&self) -> usize {
        self.pos + 1
    }

    fn eat(&mut self, ch: char) -> bool {
        if self.peek() == Some(ch) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|c| c == ' ') {
            self.pos += 1;
        }
    }

    fn error(&self, message: &str) -> AxsError {
        query_error(message, self.column(), 1)
    }

    /// A quoted string starting at the current `"`.
    fn quoted(&mut self) -> Result<String, AxsError> {
        let start = self.pos;
        let mut escaped = false;
        self.pos += 1;
        while let Some(ch) = self.peek() {
            self.pos += 1;
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                let raw: String = self.chars[start..self.pos].iter().collect();
                return string::decode(&raw)
                    .map_err(|message| query_error(message, start + 1, self.pos - start));
            }
        }
        Err(query_error("Unterminated quoted string", start + 1, 1))
    }

    /// A field name: a key without `.`, or any quoted string.
    fn segment(&mut self) -> Result<(String, SourceSpan), AxsError> {
        let start = self.pos;
        let name =
            if self.peek() == Some('"') {
                self.quoted()?
            } else {
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    self.pos += 1;
                }
                let name: String = self.chars[start..self.pos].iter().collect();
                if !valid_key(&name) {
                    return Err(query_error("Expected a field name", start + 1, 1).with_help(
                    "Field names start with a letter or '_'; quote other names, as in \"@id\"",
                ));
                }
                name
            };
        Ok((name, SourceSpan::new(1, start + 1, self.pos - start)))
    }

    /// The inside of `[...]`, after the opening bracket.
    fn selector(&mut self, start: usize) -> Result<Step, AxsError> {
        self.skip_spaces();
        let inner_start = self.pos;
        let step = if self.eat('*') {
            Step::All(SourceSpan::new(1, start, 1))
        } else if let Some(index) = self.index() {
            index
        } else {
            self.pos = inner_start;
            let condition = self.condition(0)?;
            Step::Filter(condition, SourceSpan::new(1, start, 1))
        };
        self.skip_spaces();
        if !self.eat(']') {
            return Err(self.error("Expected ']'"));
        }
        Ok(match step {
            Step::All(_) => Step::All(SourceSpan::new(1, start, self.column() - start)),
            Step::Index(index, _) => {
                Step::Index(index, SourceSpan::new(1, start, self.column() - start))
            }
            Step::Filter(condition, _) => {
                Step::Filter(condition, SourceSpan::new(1, start, self.column() - start))
            }
            other => other,
        })
    }

    /// `[3]` or `[-1]`; anything else is left for the condition parser.
    fn index(&mut self) -> Option<Step> {
        let start = self.pos;
        self.eat('-');
        let digits = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let after = self.pos;
        self.skip_spaces();
        if after == digits || self.peek() != Some(']') {
            self.pos = start;
            return None;
        }
        let text: String = self.chars[start..after].iter().collect();
        let index = text.parse().ok()?;
        Some(Step::Index(
            index,
            SourceSpan::new(1, start + 1, after - start),
        ))
    }

    fn keyword(&mut self, word: &str) -> bool {
        let end = self.pos + word.len();
        let matches = self.chars.len() >= end
            && self.chars[self.pos..end].iter().copied().eq(word.chars())
            && self.chars.get(end).is_none_or(|c| matches!(c, ' ' | '('));
        if matches {
            self.pos = end;
        }
        matches
    }

    fn condition(&mut self, depth: usize) -> Result<Condition, AxsError> {
        if depth > MAX_CONDITION_DEPTH {
            return Err(self.error("Conditions are nested too deeply"));
        }
        let mut left = self.conjunction(depth)?;
        loop {
            self.skip_spaces();
            if !self.keyword("or") {
                return Ok(left);
            }
            let right = self.conjunction(depth)?;
            left = Condition::Or(Box::new(left), Box::new(right));
        }
    }

    fn conjunction(&mut self, depth: usize) -> Result<Condition, AxsError> {
        let mut left = self.comparison(depth)?;
        loop {
            self.skip_spaces();
            if !self.keyword("and") {
                return Ok(left);
            }
            let right = self.comparison(depth)?;
            left = Condition::And(Box::new(left), Box::new(right));
        }
    }

    fn comparison(&mut self, depth: usize) -> Result<Condition, AxsError> {
        self.skip_spaces();
        if self.eat('(') {
            let inner = self.condition(depth + 1)?;
            self.skip_spaces();
            if !self.eat(')') {
                return Err(self.error("Expected ')'"));
            }
            return Ok(inner);
        }

        // `@` is the element itself; otherwise a field path of the element.
        let mut path = Vec::new();
        if !self.eat('@') {
            path.push(self.segment()?.0);
            while self.eat('.') {
                path.push(self.segment()?.0);
            }
        }

        self.skip_spaces();
        let operator = self.operator()?;
        self.skip_spaces();
        let literal = self.literal()?;
        Ok(Condition::Compare(path, operator, literal))
    }

    fn operator(&mut self) -> Result<Operator, AxsError> {
        let two: String = self.chars[self.pos..(self.pos + 2).min(self.chars.len())]
            .iter()
            .collect();
        let (operator, length) = match two.as_str() {
            "==" => (Operator::Equal, 2),
            "!=" => (Operator::NotEqual, 2),
            ">=" => (Operator::GreaterOrEqual, 2),
            "<=" => (Operator::LessOrEqual, 2),
            _ => match self.peek() {
                Some('>') => (Operator::Greater, 1),
                Some('<') => (Operator::Less, 1),
                Some('~') => (Operator::Contains, 1),
                _ => {
                    return Err(self
                        .error("Expected a comparison operator")
                        .with_help("Use ==, !=, >, >=, <, <=, or ~ (contains)"));
                }
            },
        };
        self.pos += length;
        Ok(operator)
    }

    /// A quoted string, or a bare word read like an AXS scalar.
    fn literal(&mut self) -> Result<AxsValue, AxsError> {
        if self.peek() == Some('"') {
            return self.quoted().map(AxsValue::String);
        }
        let start = self.pos;
        while self.peek().is_some_and(|c| !matches!(c, ' ' | ')' | ']')) {
            self.pos += 1;
        }
        let raw: String = self.chars[start..self.pos].iter().collect();
        if raw.is_empty() {
            return Err(self.error("Expected a value to compare with"));
        }
        parse_value(&raw).map_err(|message| query_error(message, start + 1, self.pos - start))
    }
}

fn evaluation_error(message: impl Into<String>, span: SourceSpan) -> AxsError {
    AxsError::new(ErrorKind::InvalidQuery, message).with_span(span)
}

fn type_name(value: &AxsValue) -> &'static str {
    match value {
        AxsValue::String(_) => "a string",
        AxsValue::Integer(_) | AxsValue::Float(_) => "a number",
        AxsValue::Boolean(_) => "a boolean",
        AxsValue::Null => "null",
        AxsValue::Array(_) => "an array",
        AxsValue::Object(_) => "an object",
    }
}

fn available_fields(object: &AxsObject) -> String {
    let mut names: Vec<&str> = object.keys().map(String::as_str).take(20).collect();
    if object.len() > names.len() {
        names.push("...");
    }
    format!("Available fields: {}", names.join(", "))
}

impl Step {
    fn apply(&self, value: AxsValue) -> Result<AxsValue, AxsError> {
        match (self, value) {
            (Step::Field(name, span), AxsValue::Object(mut object)) => {
                match object.shift_remove(name) {
                    Some(value) => Ok(value),
                    None => Err(evaluation_error(format!("No field '{name}'"), *span)
                        .with_help(available_fields(&object))),
                }
            }
            // On an array, `.field` and `{...}` apply to every object element.
            (Step::Field(name, _), AxsValue::Array(items)) => Ok(AxsValue::Array(
                items
                    .into_iter()
                    .filter_map(|item| field_of(item, name))
                    .collect(),
            )),
            (Step::Project(names, _), AxsValue::Object(object)) => {
                Ok(AxsValue::Object(project(object, names)))
            }
            (Step::Project(names, _), AxsValue::Array(items)) => Ok(AxsValue::Array(
                items
                    .into_iter()
                    .map(|item| match item {
                        AxsValue::Object(object) => AxsValue::Object(project(object, names)),
                        other => other,
                    })
                    .collect(),
            )),
            (Step::Index(index, span), AxsValue::Array(mut items)) => {
                let length = items.len();
                let position = if *index < 0 {
                    usize::try_from(index.unsigned_abs())
                        .ok()
                        .and_then(|back| length.checked_sub(back))
                } else {
                    usize::try_from(*index).ok().filter(|i| *i < length)
                };
                match position {
                    Some(position) => Ok(items.swap_remove(position)),
                    None => Err(evaluation_error(
                        format!("Index {index} is out of range for {length} elements"),
                        *span,
                    )),
                }
            }
            (Step::All(_), AxsValue::Array(items)) => Ok(AxsValue::Array(items)),
            (Step::All(_), AxsValue::Object(object)) => {
                Ok(AxsValue::Array(object.into_values().collect()))
            }
            (Step::Filter(condition, _), AxsValue::Array(items)) => Ok(AxsValue::Array(
                items
                    .into_iter()
                    .filter(|item| condition.matches(item))
                    .collect(),
            )),
            (step, value) => {
                let (span, expected) = match step {
                    Step::Field(_, span) | Step::Project(_, span) => (span, "an object or array"),
                    Step::Index(_, span) | Step::Filter(_, span) => (span, "an array"),
                    Step::All(span) => (span, "an array or object"),
                };
                Err(evaluation_error(
                    format!(
                        "This step needs {expected}, but the value is {}",
                        type_name(&value)
                    ),
                    *span,
                ))
            }
        }
    }
}

/// A field of an object element, mapped through nested arrays so that
/// `issues.labels.name` keeps one list of names per issue.
fn field_of(item: AxsValue, name: &str) -> Option<AxsValue> {
    match item {
        AxsValue::Object(mut object) => object.shift_remove(name),
        AxsValue::Array(items) => Some(AxsValue::Array(
            items
                .into_iter()
                .filter_map(|item| field_of(item, name))
                .collect(),
        )),
        _ => None,
    }
}

fn project(mut object: AxsObject, names: &[String]) -> AxsObject {
    names
        .iter()
        .filter_map(|name| object.shift_remove(name).map(|value| (name.clone(), value)))
        .collect()
}

impl Condition {
    fn matches(&self, item: &AxsValue) -> bool {
        match self {
            Condition::Or(left, right) => left.matches(item) || right.matches(item),
            Condition::And(left, right) => left.matches(item) && right.matches(item),
            Condition::Compare(path, operator, literal) => {
                resolve(item, path).is_some_and(|value| compare(&value, *operator, literal))
            }
        }
    }
}

/// Follows a condition's field path. Through an array, a field selects that
/// field of every object element, so `labels.name ~ bug` checks all labels.
fn resolve(item: &AxsValue, path: &[String]) -> Option<AxsValue> {
    let Some((name, rest)) = path.split_first() else {
        return Some(item.clone());
    };
    match item {
        AxsValue::Object(object) => resolve(object.get(name)?, rest),
        AxsValue::Array(items) => Some(AxsValue::Array(
            items
                .iter()
                .filter_map(|item| match item {
                    AxsValue::Object(object) => object.get(name).and_then(|v| resolve(v, rest)),
                    AxsValue::Array(_) => resolve(item, path),
                    _ => None,
                })
                .collect(),
        )),
        _ => None,
    }
}

fn number(value: &AxsValue) -> Option<f64> {
    match value {
        AxsValue::Integer(n) => Some(*n as f64),
        AxsValue::Float(n) => Some(*n),
        _ => None,
    }
}

fn equal(left: &AxsValue, right: &AxsValue) -> bool {
    match (left, right) {
        (AxsValue::Integer(a), AxsValue::Integer(b)) => a == b,
        _ => match (number(left), number(right)) {
            (Some(a), Some(b)) => a == b,
            _ => left == right,
        },
    }
}

fn compare(left: &AxsValue, operator: Operator, right: &AxsValue) -> bool {
    let ordering = match (left, right) {
        (AxsValue::Integer(a), AxsValue::Integer(b)) => Some(a.cmp(b)),
        (AxsValue::String(a), AxsValue::String(b)) => Some(a.cmp(b)),
        _ => match (number(left), number(right)) {
            (Some(a), Some(b)) => a.partial_cmp(&b),
            _ => None,
        },
    };
    match operator {
        Operator::Equal => equal(left, right),
        Operator::NotEqual => !equal(left, right),
        Operator::Greater => ordering.is_some_and(|o| o.is_gt()),
        Operator::GreaterOrEqual => ordering.is_some_and(|o| o.is_ge()),
        Operator::Less => ordering.is_some_and(|o| o.is_lt()),
        Operator::LessOrEqual => ordering.is_some_and(|o| o.is_le()),
        Operator::Contains => match (left, right) {
            (AxsValue::String(text), AxsValue::String(part)) => text.contains(part.as_str()),
            (AxsValue::Array(items), needle) => items.iter().any(|item| equal(item, needle)),
            _ => false,
        },
    }
}
