//! Runs the normative Grammar v3 fixtures in `tests/conformance/`.

use std::fs;
use std::path::Path;

use axs::{AxsObject, AxsValue};
use serde_json::Value;

fn fixtures(kind: &str, extension: &str) -> Vec<std::path::PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/conformance")
        .join(kind);
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    paths.sort();
    assert!(!paths.is_empty());
    paths
}

// A JSON number with a fraction or exponent is an AXS float (spec §16).
fn from_json(value: &Value) -> AxsValue {
    match value {
        Value::Null => AxsValue::Null,
        Value::Bool(value) => AxsValue::Boolean(*value),
        Value::Number(number) => {
            let text = number.to_string();
            if text.contains(['.', 'e', 'E']) {
                AxsValue::Float(text.parse().unwrap())
            } else {
                AxsValue::Integer(text.parse().unwrap())
            }
        }
        Value::String(text) => AxsValue::String(text.clone()),
        Value::Array(items) => AxsValue::Array(items.iter().map(from_json).collect()),
        Value::Object(entries) => AxsValue::Object(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), from_json(value)))
                .collect(),
        ),
    }
}

/// Equality that also requires the same key order and float bits.
fn same(left: &AxsValue, right: &AxsValue) -> bool {
    match (left, right) {
        (AxsValue::Float(a), AxsValue::Float(b)) => a.to_bits() == b.to_bits(),
        (AxsValue::Array(a), AxsValue::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b))
        }
        (AxsValue::Object(a), AxsValue::Object(b)) => same_object(a, b),
        _ => left == right,
    }
}

fn same_object(a: &AxsObject, b: &AxsObject) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|((ka, va), (kb, vb))| ka == kb && same(va, vb))
}

#[test]
fn valid_fixtures_parse_to_expected_json() {
    for path in fixtures("valid", "axs") {
        let source = fs::read_to_string(&path).unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(path.with_extension("json")).unwrap())
                .unwrap();
        let AxsValue::Object(expected) = from_json(&expected) else {
            panic!("{} root must be an object", path.display())
        };

        let parsed = axs::parse(&source)
            .unwrap_or_else(|error| panic!("{}\n{}", path.display(), error.render(&source, "")));
        assert!(
            same_object(&parsed, &expected),
            "{}\nparsed:   {parsed:?}\nexpected: {expected:?}",
            path.display()
        );
    }
}

#[test]
fn valid_fixtures_round_trip_canonically() {
    for path in fixtures("valid", "axs") {
        let parsed = axs::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        let canonical = axs::stringify(&parsed).unwrap();
        let restored = axs::parse(&canonical)
            .unwrap_or_else(|error| panic!("{}\n{canonical}\n{error}", path.display()));
        assert!(
            same_object(&parsed, &restored),
            "{}\n{canonical}",
            path.display()
        );
        assert_eq!(
            axs::stringify(&restored).unwrap(),
            canonical,
            "{}",
            path.display()
        );
    }
}

#[test]
fn invalid_fixtures_are_rejected() {
    for path in fixtures("invalid", "axs") {
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            source.starts_with("# error: "),
            "{} must name its expected error",
            path.display()
        );
        if let Ok(value) = axs::parse(&source) {
            panic!("{} parsed as {value:?}", path.display());
        }
    }
}
