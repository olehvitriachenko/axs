use axs::{AxsValue, parse, stringify};

fn roundtrip(source: &str) {
    let original = parse(source).unwrap();
    let encoded = stringify(&original).unwrap();
    let restored = parse(&encoded).unwrap();

    assert_eq!(original, restored);
}

#[test]
fn empty_array() {
    let data = parse("tags = []").unwrap();
    assert_eq!(data["tags"], AxsValue::Array(vec![]));
    roundtrip("tags = []");
}

#[test]
fn empty_object() {
    let data = parse("settings = {}").unwrap();
    assert_eq!(data["settings"], AxsValue::Object(Default::default()));
    roundtrip("settings = {}");
}

#[test]
fn array_of_objects() {
    roundtrip(
        r#"users:
  - name = Oleh
    age = 22
  - name = Alex
    age = 25
"#,
    );
}

#[test]
fn nested_arrays() {
    roundtrip(
        r#"matrix:
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
fn nested_objects_in_arrays() {
    roundtrip(
        r#"users:
  - name = Oleh
    skills:
      - Rust
      - TypeScript
  - name = Alex
    skills:
      - Go
"#,
    );
}

#[test]
fn empty_collections_inside_array() {
    roundtrip(
        r#"items:
  - []
  - {}
  - null
"#,
    );
}

#[test]
fn rejects_inconsistent_indentation() {
    assert!(parse("user:\n    name = Oleh").is_err());
}
