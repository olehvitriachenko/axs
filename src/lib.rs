#![doc = include_str!("../README.md")]

pub mod error;
pub mod lexer;
pub mod parser;
pub mod query;
mod serde_support;
pub mod serializer;
pub mod string;
pub mod token_parser;
pub mod value;
pub mod value_parser;

pub use serde_support::{from_str, to_string};

// Diagnostics
pub use error::{AxsError, ErrorKind, SourceSpan};

// Lexer
pub use lexer::tokenize;

// Parser
pub use token_parser::parse_tokens;
pub use token_parser::parse_tokens as parse;

// Queries
pub use query::query;

// Serializer
pub use serializer::stringify;

// Data types
pub use value::{AxsObject, AxsValue};

// Compatibility entry point with string errors
pub use parser::parse as parse_legacy;
