use axs::{parse, stringify};

fn check_roundtrip(source: &str) {
    let original = parse(source).unwrap();
    let encoded = stringify(&original).unwrap();
    let restored = parse(&encoded).unwrap();

    assert_eq!(original, restored);
}

#[test]
fn primitive_values() {
    check_roundtrip("name = Oleh\nage = 22\nactive = true\n");
}

#[test]
fn nested_object() {
    check_roundtrip("user\n  name = Oleh\n  age = 22\n");
}

#[test]
fn array_of_strings() {
    check_roundtrip("skills\n  - Rust\n  - TypeScript\n");
}

#[test]
fn leading_zero_string() {
    check_roundtrip("zip = 00123\n");
}

#[test]
fn float_value() {
    check_roundtrip("height = 1.76\n");
}
