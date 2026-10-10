use axs::{AxsValue, ErrorKind, parse};

#[test]
fn rejects_indented_root() {
    assert!(parse("  name = Oleh").is_err());
}

#[test]
fn rejects_indentation_after_scalar() {
    assert!(parse("name = Oleh\n  age = 22").is_err());
}

#[test]
fn rejects_missing_nested_block() {
    assert!(parse("user:").is_err());
}

#[test]
fn rejects_duplicate_nested_key() {
    let source = "user:\n  name = Oleh\n  name = Alex";

    let error = parse(source).unwrap_err();

    assert_eq!(error.kind, ErrorKind::DuplicateKey);
    assert_eq!(error.span.unwrap().line, 3);
}

#[test]
fn rejects_duplicate_inline_object_key() {
    let source = "users:\n  - name = Oleh\n    name = Alex";

    let error = parse(source).unwrap_err();

    assert_eq!(error.kind, ErrorKind::DuplicateKey);
}

#[test]
fn tracks_duplicate_inline_key_position() {
    let source = "users:\n  - name = Oleh\n    name = Alex";

    let error = parse(source).unwrap_err();
    let span = error.span.unwrap();

    // The duplicate is on the third line,
    // not the first field of the array object.
    assert_eq!(span.line, 3);
    assert_eq!(span.column, 5);
}

#[test]
fn preserves_quoted_equals_in_array() {
    let source = "messages:\n  - \"Hello = World\"";

    let result = parse(source).unwrap();
    let AxsValue::Array(items) = &result["messages"] else {
        panic!("Expected array");
    };

    assert_eq!(items[0], AxsValue::String("Hello = World".into()));
}

#[test]
fn preserves_unicode_strings() {
    let result = parse("city = Київ\nname = Олег").unwrap();

    assert_eq!(result["city"], AxsValue::String("Київ".into()));
    assert_eq!(result["name"], AxsValue::String("Олег".into()));
}

#[test]
fn parses_i64_boundaries() {
    let source = format!("min = {}\nmax = {}", i64::MIN, i64::MAX);

    let result = parse(&source).unwrap();

    assert_eq!(result["min"], AxsValue::Integer(i64::MIN));
    assert_eq!(result["max"], AxsValue::Integer(i64::MAX));
}

#[test]
fn preserves_leading_zero_identifier() {
    let result = parse("code = 00123").unwrap();

    assert_eq!(result["code"], AxsValue::String("00123".into()));
}

#[test]
fn rejects_unterminated_string() {
    let error = parse("name = \"Oleh").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidString);
}

#[test]
fn rejects_invalid_array_indentation() {
    let source = "skills:\n  - Rust\n      - Go";

    assert!(parse(source).is_err());
}
