use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl SourceSpan {
    pub fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    UnexpectedToken,
    InvalidIndentation,
    InvalidKey,
    InvalidValue,
    DuplicateKey,
    UnexpectedEof,
    InvalidString,
    InvalidNumber,
    SyntaxError,
}

impl ErrorKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::UnexpectedToken => "AXS001",
            Self::InvalidIndentation => "AXS002",
            Self::InvalidKey => "AXS003",
            Self::InvalidValue => "AXS004",
            Self::DuplicateKey => "AXS005",
            Self::UnexpectedEof => "AXS006",
            Self::InvalidString => "AXS007",
            Self::InvalidNumber => "AXS008",
            Self::SyntaxError => "AXS009",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxsError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub help: Option<String>,
}

impl AxsError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            span: None,
            help: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn render(&self, source: &str, filename: &str) -> String {
        let mut output = format!(
            "error[{}]: {}",
            self.kind.code(),
            clipped(&self.message, 1024)
        );

        if let Some(span) = self.span {
            let line_number = span.line;
            let column = span.column.max(1);

            output.push_str(&format!(
                "\n  --> {}:{line_number}:{column}",
                clipped(filename, 256)
            ));

            if source.len() > crate::lexer::MAX_INPUT_BYTES {
                output.push_str("\n  (source excerpt omitted: document exceeds 8 MiB)");
            } else if let Some(line) = source.lines().nth(line_number.saturating_sub(1)) {
                let gutter_width = line_number.to_string().len();

                output.push_str(&format!("\n{:width$} |", "", width = gutter_width + 1));

                // Crop around the marker without slicing through a UTF-8 character.
                let line_length = line.chars().count();
                let offset = (column - 1).min(line_length);
                let start = offset.saturating_sub(100);
                let end = offset.saturating_add(100).min(line_length);
                let excerpt: String = line.chars().skip(start).take(end - start).collect();
                let prefix = if start > 0 { "... " } else { "" };
                let suffix = if end < line_length { " ..." } else { "" };
                output.push_str(&format!(
                    "\n{line_number} | {prefix}{}{suffix}",
                    escaped(&excerpt)
                ));
                let before: String = excerpt.chars().take(offset - start).collect();
                let padding = " ".repeat(prefix.len() + escaped(&before).chars().count());
                let highlighted: String = excerpt
                    .chars()
                    .skip(offset - start)
                    .take(span.length.max(1))
                    .collect();
                let carets = "^".repeat(escaped(&highlighted).chars().count().max(1));

                output.push_str(&format!(
                    "\n{:width$} | {padding}{carets}",
                    "",
                    width = gutter_width
                ));
            }
        }

        if let Some(help) = &self.help {
            output.push_str(&format!("\nhelp: {}", clipped(help, 1024)));
        }

        output
    }
}

fn clipped(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((end, _)) => format!("{}...", escaped(&text[..end])),
        None => escaped(text),
    }
}

fn escaped(text: &str) -> String {
    let mut output = String::new();
    for character in text.chars() {
        if character.is_control() {
            output.extend(character.escape_default());
        } else {
            output.push(character);
        }
    }
    output
}

impl fmt::Display for AxsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "error[{}]: {}",
            self.kind.code(),
            clipped(&self.message, 1024)
        )
    }
}

impl Error for AxsError {}

impl serde::ser::Error for AxsError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::new(ErrorKind::InvalidValue, message.to_string())
    }
}

impl serde::de::Error for AxsError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::new(ErrorKind::InvalidValue, message.to_string())
    }
}
