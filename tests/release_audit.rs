use std::collections::BTreeMap;

use axs::{AxsError, ErrorKind, SourceSpan};
use serde::{Deserialize, Serialize};

#[test]
fn long_line_diagnostics_have_bounded_output() {
    let source = format!("name = {}🚀", "x".repeat(100_000));
    let error = AxsError::new(ErrorKind::InvalidString, "bad value")
        .with_span(SourceSpan::new(1, 100_008, 100_000));
    let rendered = error.render(&source, "input.axs");
    assert!(rendered.contains("input.axs:1:100008"));
    assert!(rendered.contains('^'));
    assert!(rendered.len() < 4096, "rendered {} bytes", rendered.len());
}

#[test]
fn serialization_is_deterministic_across_map_insertion_order() {
    let mut first = std::collections::HashMap::new();
    let mut second = std::collections::HashMap::new();
    for key in ["z", "a", "m"] {
        first.insert(key, key);
    }
    for key in ["m", "a", "z"] {
        second.insert(key, key);
    }
    assert_eq!(
        axs::to_string(&first).unwrap(),
        axs::to_string(&second).unwrap()
    );
    assert_eq!(
        axs::stringify(&axs::parse("z = z\na = a\nm = m").unwrap()).unwrap(),
        "a = a\nm = m\nz = z\n"
    );
}

#[test]
fn map_and_enum_shapes_have_schema_driven_interpretation() {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum State {
        Count(i64),
    }
    let text = axs::to_string(&State::Count(2)).unwrap();
    assert_eq!(axs::from_str::<State>(&text).unwrap(), State::Count(2));
    assert_eq!(
        axs::from_str::<BTreeMap<String, i64>>(&text).unwrap()["Count"],
        2
    );
}

#[test]
fn numeric_flatten_deserialization_fails_predictably() {
    #[derive(Debug, Serialize, Deserialize)]
    struct Flat {
        name: String,
        #[serde(flatten)]
        other: BTreeMap<String, f32>,
    }
    let source = "name = example\nvalue = 0.1";
    let error = axs::from_str::<Flat>(source).unwrap_err();
    assert!(
        error
            .message
            .contains("Numeric deserialize_any is unsupported")
    );
    assert_eq!(error, axs::from_str::<Flat>(source).unwrap_err());
    let serializable = Flat {
        name: "example".into(),
        other: BTreeMap::from([("value".into(), 0.5)]),
    };
    assert!(axs::to_string(&serializable).is_ok());
}

#[test]
fn oversized_diagnostic_fields_and_sources_are_clipped() {
    let source = "x".repeat(axs::lexer::MAX_INPUT_BYTES + 1);
    let error = AxsError::new(ErrorKind::SyntaxError, "🚀".repeat(20_000))
        .with_help("🚀".repeat(20_000))
        .with_span(SourceSpan::new(1, usize::MAX, usize::MAX));
    let rendered = error.render(&source, &"🚀".repeat(20_000));
    assert!(rendered.contains("source excerpt omitted"));
    assert!(rendered.len() < 12_000);
}

#[test]
fn error_codes_cover_every_public_variant() {
    let kinds = [
        ErrorKind::UnexpectedToken,
        ErrorKind::InvalidIndentation,
        ErrorKind::InvalidKey,
        ErrorKind::InvalidValue,
        ErrorKind::DuplicateKey,
        ErrorKind::UnexpectedEof,
        ErrorKind::InvalidString,
        ErrorKind::InvalidNumber,
        ErrorKind::SyntaxError,
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        assert_eq!(kind.code(), format!("AXS{:03}", index + 1));
    }
}

#[test]
fn numeric_limits_reject_the_first_value_outside_i64() {
    for result in [
        axs::to_string(&BTreeMap::from([("v", (i64::MAX as u64) + 1)])),
        axs::to_string(&BTreeMap::from([("v", i128::from(i64::MIN) - 1)])),
        axs::to_string(&BTreeMap::from([("v", (i64::MAX as u128) + 1)])),
    ] {
        assert_eq!(result.unwrap_err().kind, ErrorKind::InvalidNumber);
    }
}

struct AuditWrapper(usize);
impl Serialize for AuditWrapper {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 == 0 {
            serializer.serialize_unit()
        } else {
            serializer.serialize_newtype_struct("Wrapper", &AuditWrapper(self.0 - 1))
        }
    }
}

// A stack overflow aborts the process, so probe in a subprocess and retain the
// failure as an ordinary regression result instead of killing the full suite.
#[test]
fn one_megabyte_stack_probe() {
    if std::env::var_os("AXS_AUDIT_STACK_CHILD").is_some() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let mut source = String::new();
                for depth in 0..128 {
                    source.push_str(&format!("{}child:\n", "  ".repeat(depth)));
                }
                source.push_str(&format!("{}v = 1\n", "  ".repeat(128)));
                let tree = axs::parse(&source).unwrap();
                axs::stringify(&tree).unwrap();
                assert!(axs::to_string(&BTreeMap::from([("v", AuditWrapper(10_000))])).is_err());
            })
            .unwrap()
            .join()
            .unwrap();
        return;
    }
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "one_megabyte_stack_probe", "--nocapture"])
        .env("AXS_AUDIT_STACK_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn diagnostics_escape_untrusted_terminal_controls() {
    let error = AxsError::new(ErrorKind::InvalidKey, "bad\x1b[2J\rkey")
        .with_help("hint\x1b[31m")
        .with_span(SourceSpan::new(1, 1, 1));
    let rendered = error.render("\x1b[2Jkey", "input\x1b[2J.axs");
    assert!(!error.to_string().contains('\x1b'));
    assert!(!error.to_string().contains('\r'));
    assert!(!rendered.contains('\x1b'));
    assert!(!rendered.contains('\r'));
    assert!(rendered.contains("\\u{1b}"));
}

#[test]
fn structured_mutation_property_tests_are_reproducible() {
    let mut seed = 0x4158_535f_4155_4449u64;
    let mutations = [
        '"', '\\', '#', '=', ':', '-', ' ', '\n', '\r', '\t', '\0', '🚀',
    ];
    for case in 0..1000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let values = BTreeMap::from([(
            "values",
            vec![format!("key = {seed}"), format!("🚀\n{case}")],
        )]);
        let encoded = axs::to_string(&values).unwrap();
        let mut chars: Vec<_> = encoded.chars().collect();
        let offset = seed as usize % (chars.len() + 1);
        chars.insert(offset, mutations[(seed >> 16) as usize % mutations.len()]);
        let source: String = chars.into_iter().collect();
        let result = std::panic::catch_unwind(|| {
            let first = axs::parse(&source);
            assert_eq!(first, axs::parse(&source));
            assert_eq!(
                axs::from_str::<BTreeMap<String, Vec<String>>>(&source),
                axs::from_str::<BTreeMap<String, Vec<String>>>(&source)
            );
            match first {
                Ok(tree) => assert_eq!(axs::parse(&axs::stringify(&tree).unwrap()).unwrap(), tree),
                Err(error) => {
                    assert!(error.render(&source, "mutated.axs").len() < 4096);
                }
            }
        });
        assert!(result.is_ok(), "seed {seed}, case {case}, input {source:?}");
    }
}

#[test]
fn syntax_specification_examples_parse_with_the_real_api() {
    let spec = include_str!("../spec/AXS.md").replace("\r\n", "\n");
    let mut count = 0;
    for block in spec.split("```text\n").skip(1) {
        let (source, _) = block.split_once("```").unwrap();
        let values = axs::parse(source).unwrap_or_else(|error| panic!("{source}\n{error}"));
        assert_eq!(
            axs::parse(&axs::stringify(&values).unwrap()).unwrap(),
            values
        );
        count += 1;
    }
    assert!(count > 0);
}
