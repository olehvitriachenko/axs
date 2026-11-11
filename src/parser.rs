use crate::value::AxsObject;

/// Compatibility entry point retaining the legacy string-error signature.
/// Grammar, limits, and diagnostics are implemented by the token parser.
pub fn parse(input: &str) -> Result<AxsObject, String> {
    crate::token_parser::parse_tokens(input).map_err(|error| error.to_string())
}
