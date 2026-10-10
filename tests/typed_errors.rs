use axs::{ErrorKind, parse, tokenize};

#[test]
fn reports_missing_value() {
    let error = parse("age =").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidValue);

    let span = error.span.unwrap();
    assert_eq!(span.line, 1);
    assert_eq!(span.column, 6);
}

#[test]
fn reports_duplicate_key() {
    let error = parse("name = Oleh\nname = Alex").unwrap_err();

    assert_eq!(error.kind, ErrorKind::DuplicateKey);

    let span = error.span.unwrap();
    assert_eq!(span.line, 2);
    assert_eq!(span.column, 1);
}

#[test]
fn reports_invalid_key() {
    let error = parse("123name = Oleh").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidKey);
    assert_eq!(error.span.unwrap().line, 1);
}

#[test]
fn reports_invalid_indentation() {
    let error = parse("user:\n   name = Oleh").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert_eq!(error.span.unwrap().line, 2);
}

#[test]
fn reports_tabs() {
    let error = tokenize("name\t= Oleh").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert_eq!(error.span.unwrap().column, 5);
}

#[test]
fn reports_missing_nested_block() {
    let error = parse("user:").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert!(error.span.is_some());
}

#[test]
fn renders_real_parser_error() {
    let source = "name = Oleh\nage =";

    let error = parse(source).unwrap_err();
    let output = error.render(source, "profile.axs");

    assert!(output.contains("AXS004"));
    assert!(output.contains("profile.axs:2:6"));
    assert!(output.contains("2 | age ="));
    assert!(output.contains("^"));
}

#[test]
fn reports_invalid_quoted_string() {
    let error = parse("name = \"unterminated").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidString);
    assert_eq!(error.span.unwrap().column, 8);
}

#[test]
fn typed_stringify_reports_structured_errors() {
    use std::collections::HashMap;

    let mut object = HashMap::new();
    object.insert("bad key".to_string(), axs::AxsValue::Null);
    let error = axs::serializer::stringify_typed(&object).unwrap_err();
    assert_eq!(error.kind, axs::ErrorKind::InvalidKey);
    assert_eq!(axs::stringify(&object).unwrap_err(), error.to_string());

    object.clear();
    object.insert("pi".to_string(), axs::AxsValue::Float(f64::NAN));
    let error = axs::serializer::stringify_typed(&object).unwrap_err();
    assert_eq!(error.kind, axs::ErrorKind::InvalidNumber);
}
