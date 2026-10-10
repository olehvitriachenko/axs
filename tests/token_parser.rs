use axs::{AxsValue, parse_legacy, parse_tokens, stringify};

fn assert_compatible(source: &str) {
    let old_result = parse_legacy(source).unwrap();
    let new_result = parse_tokens(source).unwrap();

    assert_eq!(old_result, new_result);

    let encoded = stringify(&new_result).unwrap();
    let restored = parse_tokens(&encoded).unwrap();

    assert_eq!(new_result, restored);
}

#[test]
fn parses_primitives() {
    assert_compatible("name = Oleh\nage = 22\nactive = true\n");
}

#[test]
fn parses_nested_object() {
    assert_compatible("user\n  name = Oleh\n  age = 22\n");
}

#[test]
fn parses_array() {
    assert_compatible("skills\n  - Rust\n  - TypeScript\n");
}

#[test]
fn parses_inline_objects() {
    assert_compatible(
        r#"users
  - name = Oleh
    age = 22
  - name = Alex
    age = 25
"#,
    );
}

#[test]
fn parses_deeply_nested_structures() {
    assert_compatible(
        r#"users
  - name = Oleh
    skills
      - Rust
      - TypeScript
  - name = Alex
    skills
      - Go
"#,
    );
}

#[test]
fn parses_nested_arrays() {
    assert_compatible(
        r#"matrix
  -
    - 1
    - 2
  -
    - 3
    - 4
"#,
    );
}

#[test]
fn parses_empty_collections() {
    assert_compatible("tags = []\nsettings = {}\n");
}

#[test]
fn parses_quoted_strings() {
    assert_compatible("name = \"123\"\nempty = \"\"\n");
}

#[test]
fn rejects_duplicate_keys() {
    assert!(parse_tokens("name = Oleh\nname = Alex").is_err());
}

#[test]
fn rejects_invalid_indentation() {
    assert!(parse_tokens("user\n    name = Oleh").is_err());
}

#[test]
fn rejects_missing_value() {
    assert!(parse_tokens("name = ").is_err());
}

#[test]
fn parses_array_objects() {
    let data = parse_tokens("users\n  - name = Oleh\n    age = 22\n").unwrap();

    let AxsValue::Array(users) = &data["users"] else {
        panic!("Expected array");
    };

    assert_eq!(users.len(), 1);

    let AxsValue::Object(user) = &users[0] else {
        panic!("Expected object");
    };

    assert_eq!(user["age"], AxsValue::Integer(22));
}
