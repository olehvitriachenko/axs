use std::collections::HashMap;

use crate::value::AxsValue;

/// Compatibility entry point retaining the legacy string-error signature.
/// Grammar, limits, and diagnostics are implemented by the token parser.
pub fn parse(input: &str) -> Result<HashMap<String, AxsValue>, String> {
    crate::token_parser::parse_tokens(input).map_err(|error| error.to_string())
}
