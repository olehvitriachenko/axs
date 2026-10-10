use axs::{AxsValue, parse, stringify};

#[test]
fn quoted_number_stays_string() {
    let data = parse(r#"age = "22""#).unwrap();

    assert_eq!(data.get("age"), Some(&AxsValue::String("22".into())));
}

#[test]
fn escaped_quotes_roundtrip() {
    let source = r#"message = "He said \"Hello\"""#;

    let original = parse(source).unwrap();
    let encoded = stringify(&original).unwrap();
    let restored = parse(&encoded).unwrap();

    assert_eq!(original, restored);
}

#[test]
fn empty_string_roundtrip() {
    let original = parse("name = \"\"").unwrap();
    let encoded = stringify(&original).unwrap();
    let restored = parse(&encoded).unwrap();

    assert_eq!(original, restored);
}
