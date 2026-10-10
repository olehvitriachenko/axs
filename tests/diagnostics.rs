use axs::{AxsError, ErrorKind, SourceSpan};

#[test]
fn creates_error() {
    let error = AxsError::new(ErrorKind::InvalidValue, "Expected value");

    assert_eq!(error.kind, ErrorKind::InvalidValue);
    assert_eq!(error.message, "Expected value");
    assert!(error.span.is_none());
}

#[test]
fn error_codes_are_stable() {
    assert_eq!(ErrorKind::UnexpectedToken.code(), "AXS001");
    assert_eq!(ErrorKind::InvalidIndentation.code(), "AXS002");
    assert_eq!(ErrorKind::DuplicateKey.code(), "AXS005");
    assert_eq!(ErrorKind::InvalidNumber.code(), "AXS008");
}

#[test]
fn renders_source_location() {
    let source = "name = Oleh\nage = \nactive = true";

    let error = AxsError::new(ErrorKind::InvalidValue, "Expected value")
        .with_span(SourceSpan::new(2, 7, 1));

    let output = error.render(source, "profile.axs");

    assert!(output.contains("profile.axs:2:7"));
    assert!(output.contains("2 | age = "));
    assert!(output.contains("^"));
}

#[test]
fn renders_help_message() {
    let error = AxsError::new(ErrorKind::InvalidIndentation, "Invalid indentation")
        .with_span(SourceSpan::new(2, 4, 1))
        .with_help("Use exactly two spaces per level");

    let output = error.render("user\n   name = Oleh", "test.axs");

    assert!(output.contains("AXS002"));
    assert!(output.contains("help: Use exactly two spaces per level"));
}

#[test]
fn renders_error_without_span() {
    let error = AxsError::new(ErrorKind::SyntaxError, "Invalid document");

    let output = error.render("", "test.axs");

    assert_eq!(output, "error[AXS009]: Invalid document");
}

#[test]
fn implements_standard_error() {
    fn accepts_error(_error: &dyn std::error::Error) {}

    let error = AxsError::new(ErrorKind::UnexpectedEof, "Unexpected end of file");

    accepts_error(&error);

    assert_eq!(error.to_string(), "error[AXS006]: Unexpected end of file");
}
