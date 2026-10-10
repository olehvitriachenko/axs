use axs::{AxsValue, ErrorKind, parse};

#[test]
fn parses_i64_minimum() {
    let result = parse("number = -9223372036854775808").unwrap();

    assert_eq!(result["number"], AxsValue::Integer(i64::MIN));
}

#[test]
fn parses_i64_maximum() {
    let result = parse("number = 9223372036854775807").unwrap();

    assert_eq!(result["number"], AxsValue::Integer(i64::MAX));
}

#[test]
fn rejects_integer_overflow() {
    let error = parse("number = 9223372036854775808").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidNumber);

    let span = error.span.unwrap();
    assert_eq!(span.line, 1);
    assert_eq!(span.column, 10);
}

#[test]
fn rejects_integer_underflow() {
    let error = parse("number = -9223372036854775809").unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidNumber);
}

#[test]
fn keeps_quoted_large_integer_as_string() {
    let result = parse("number = \"9223372036854775808\"").unwrap();

    assert_eq!(
        result["number"],
        AxsValue::String("9223372036854775808".into())
    );

    let serialized = axs::stringify(&result).unwrap();
    assert_eq!(serialized, "number = \"9223372036854775808\"\n");
    assert_eq!(axs::parse(&serialized).unwrap(), result);
}

#[test]
fn rejects_float_overflow() {
    let huge_fraction = format!("1.{}", "0".repeat(400));

    // This value is approximately 1.0, so it is finite.
    let source = format!("number = {huge_fraction}");
    assert!(parse(&source).is_ok());

    // A large decimal integer part exceeds f64::MAX.
    let huge = format!("{}.0", "9".repeat(400));
    let source = format!("number = {huge}");

    let error = parse(&source).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidNumber);
}

#[test]
fn preserves_leading_zero_string() {
    let result = parse("code = 00123").unwrap();

    assert_eq!(result["code"], AxsValue::String("00123".into()));
}

#[test]
fn rejects_excessive_nesting() {
    let mut source = String::new();

    for depth in 0..=129 {
        source.push_str(&"  ".repeat(depth));
        source.push_str("item\n");
    }

    let error = parse(&source).unwrap_err();

    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert!(error.message.contains("Maximum nesting depth"));
    let span = error.span.unwrap();
    assert_eq!(span.line, 130);
    assert_eq!(span.column, 259);
}

#[test]
fn accepts_normal_nesting() {
    let source = "user\n  profile\n    age = 22";

    let result = parse(source).unwrap();

    let AxsValue::Object(user) = &result["user"] else {
        panic!("Expected user object");
    };

    let AxsValue::Object(profile) = &user["profile"] else {
        panic!("Expected profile object");
    };

    assert_eq!(profile["age"], AxsValue::Integer(22));

    // The limit is inclusive: a scalar at depth 128 remains valid.
    let mut source = String::new();
    for depth in 0..128 {
        source.push_str(&"  ".repeat(depth));
        source.push_str("item\n");
    }
    source.push_str(&"  ".repeat(128));
    source.push_str("age = 22\n");
    assert!(parse(&source).is_ok());
}
