use std::collections::{BTreeMap, HashMap};

use axs::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, MAX_TOKENS, TokenKind};
use axs::{AxsValue, ErrorKind, SourceSpan, from_str, parse, stringify, to_string, tokenize};
use serde::{Deserialize, Serialize};

fn canonical(source: &str, expected: &str) {
    let value = parse(source).unwrap();
    assert_eq!(stringify(&value).unwrap(), expected);
    assert_eq!(parse(expected).unwrap(), value);
    assert_eq!(stringify(&parse(expected).unwrap()).unwrap(), expected);
}

#[test]
fn named_objects_and_arrays_have_explicit_headers() {
    canonical(
        "user:\n  name = Oleh\n  age = 22\nskills:\n  - Rust\n  - TypeScript\n",
        "skills:\n  - Rust\n  - TypeScript\nuser:\n  age = 22\n  name = Oleh\n",
    );
}

#[test]
fn inline_named_array_expands_canonically() {
    canonical(
        "categories:\n  - UI:\n      - MUI\n      - Tailwind\n",
        "categories:\n  -\n    UI:\n      - MUI\n      - Tailwind\n",
    );
}

#[test]
fn inline_named_object_and_sibling_fields_share_one_object() {
    canonical(
        "items:\n  - config:\n      enabled = true\n    name = first\n    tags:\n      - Rust\n  - config:\n      enabled = false\n",
        "items:\n  -\n    config:\n      enabled = true\n    name = first\n    tags:\n      - Rust\n  -\n    config:\n      enabled = false\n",
    );
}

#[test]
fn inline_assignment_can_continue_with_named_blocks() {
    canonical(
        "users:\n  - name = Oleh\n    skills:\n      - Rust\n    age = 22\n",
        "users:\n  -\n    age = 22\n    name = Oleh\n    skills:\n      - Rust\n",
    );
}

#[test]
fn nested_inline_blocks_and_arrays_roundtrip() {
    canonical(
        "items:\n  - outer:\n      - inner:\n          -\n            - 1\n            - {}\n        extra = []\n    name = end\n",
        "items:\n  -\n    name = end\n    outer:\n      -\n        extra = []\n        inner:\n          -\n            - 1\n            - {}\n",
    );
}

#[test]
fn comments_blank_lines_and_crlf_preserve_inline_indentation() {
    canonical(
        "categories : \r\n  - UI :\r\n# comment\r\n\r\n      - MUI\r\n        # ignored\r\n    name = literal # suffix\r\n",
        "categories:\n  -\n    UI:\n      - MUI\n    name = literal # suffix\n",
    );
}

#[test]
fn bare_keys_and_mixed_legacy_blocks_are_rejected_by_every_entry_point() {
    for source in [
        "user",
        "user\n  age = 22",
        "user:\n  profile\n    age = 22",
        "users:\n  - name = Oleh\n    skills\n      - Rust",
        "user:\n  age = 22\nskills\n  - Rust",
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.kind, ErrorKind::UnexpectedToken, "{source}");
        assert!(error.message.contains("Bare block keys"));
        assert!(tokenize(source).is_err());
        assert!(axs::parse_legacy(source).is_err());
        assert!(from_str::<BTreeMap<String, String>>(source).is_err());
    }
}

#[test]
fn malformed_colon_headers_are_rejected() {
    for source in [
        "key::",
        "key: value",
        "key: []",
        "key: {}",
        "key: = 1",
        "key: # inline comment",
        ":\n  x = 1",
        "bad key:\n  x = 1",
        "items:\n  - key::",
        "items:\n  - key: value",
        "items:\n  - key: = 1",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn colon_requires_a_nonempty_indented_collection() {
    for source in [
        "key:",
        "key:\n# comment\n",
        "key:\nx = 1",
        "key:\n  {}",
        "key:\n  []",
        "key:\n  value",
        "key:\n  -",
        "items:\n  - key:",
        "items:\n  - key:\n  - next",
        "key:\n  x = 1\n  - 2",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert_eq!(
        parse("key:").unwrap_err().kind,
        ErrorKind::InvalidIndentation
    );
}

#[test]
fn inline_named_blocks_require_two_levels_below_the_dash() {
    for spaces in [0, 2, 4, 8, 10] {
        let source = format!("items:\n  - UI:\n{}- MUI", " ".repeat(spaces));
        assert_eq!(
            parse(&source).unwrap_err().kind,
            ErrorKind::InvalidIndentation,
            "{source}"
        );
    }
    let error = parse("items:\n  - UI:\n    - MUI").unwrap_err();
    assert!(error.message.contains("four spaces"));
    assert_eq!(error.span, Some(SourceSpan::new(3, 5, 1)));
    for source in ["items:\n  -\n      x = 1", "root:\n    x = 1"] {
        assert!(parse(source).is_err(), "{source}");
    }
}

#[test]
fn duplicate_named_fields_keep_duplicate_source_spans() {
    for (source, span) in [
        ("key:\n  x = 1\nkey = []", SourceSpan::new(3, 1, 3)),
        ("key = {}\nkey:\n  x = 1", SourceSpan::new(2, 1, 3)),
        (
            "items:\n  - UI:\n      - MUI\n    UI = []",
            SourceSpan::new(4, 5, 2),
        ),
        (
            "items:\n  - UI:\n      - MUI\n    UI:\n      - Rust",
            SourceSpan::new(4, 5, 2),
        ),
        (
            "items:\n  - UI:\n      x = 1\n      x = 2",
            SourceSpan::new(4, 7, 1),
        ),
    ] {
        let error = parse(source).unwrap_err();
        assert_eq!(error.kind, ErrorKind::DuplicateKey, "{source}");
        assert_eq!(error.span, Some(span));
    }
}

#[test]
fn colon_tokens_and_synthetic_inline_indents_have_precise_positions() {
    let tokens = tokenize("items :\n  - UI :\n      - \"🚀:= -\"").unwrap();
    let colons: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Colon)
        .map(|t| (t.span.line, t.span.column))
        .collect();
    assert_eq!(colons, [(1, 7), (2, 8)]);
    let indents: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Indent)
        .map(|t| (t.span.line, t.span.column))
        .collect();
    assert_eq!(indents, [(2, 3), (3, 5), (3, 7)]);
    let error = parse("items:\n  - UI: 🚀").unwrap_err();
    assert_eq!(error.span, Some(SourceSpan::new(2, 7, 3)));
    let error = parse("items:\n  - UI:\n      x = \"🚀\\q\"").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidString);
    assert_eq!(error.span, Some(SourceSpan::new(3, 11, 5)));
    assert_eq!(
        parse("user").unwrap_err().span,
        Some(SourceSpan::new(1, 1, 4))
    );
}

#[test]
fn special_strings_are_quoted_when_array_classification_requires_it() {
    for text in [
        "UI:",
        "UI :",
        "UI: value",
        "key::",
        "http://host",
        "key: = text",
        "a=b:c - d",
        "a:b=c",
        ":",
        "-",
        "- UI:",
        "bad key:",
        "🚀: = -",
        "",
    ] {
        let values = HashMap::from([
            ("scalar".to_owned(), AxsValue::String(text.into())),
            (
                "items".to_owned(),
                AxsValue::Array(vec![AxsValue::String(text.into())]),
            ),
        ]);
        let encoded = stringify(&values).unwrap();
        assert_eq!(parse(&encoded).unwrap(), values, "{text:?}\n{encoded}");
        if text.starts_with("UI") || text.starts_with("key:") || text.starts_with("http:") {
            assert!(encoded.contains("  - \""), "{encoded}");
        }
    }
    assert_eq!(
        parse("x = UI:").unwrap()["x"],
        AxsValue::String("UI:".into())
    );
    assert_eq!(
        parse("items:\n  - \"UI: = -\"").unwrap()["items"],
        AxsValue::Array(vec![AxsValue::String("UI: = -".into())])
    );
}

#[test]
fn empty_collections_remain_assignments_and_roots_remain_objects() {
    canonical(
        "items:\n  - UI = []\n    config = {}\nempty = []\n",
        "empty = []\nitems:\n  -\n    UI = []\n    config = {}\n",
    );
    for source in ["- 1", "[]", "{}", "42", "\"root\""] {
        assert!(parse(source).is_err(), "{source}");
    }
    assert!(parse("").unwrap().is_empty());
}

fn inline_at_depth(array_depth: usize, child: &str) -> String {
    let mut source = String::from("items:\n");
    for depth in 1..array_depth {
        source.push_str(&format!("{}-\n", "  ".repeat(depth)));
    }
    source.push_str(&format!(
        "{}- key:\n{}{child}\n",
        "  ".repeat(array_depth),
        "  ".repeat(array_depth + 2)
    ));
    source
}

#[test]
fn inline_named_collections_reach_but_never_exceed_depth_128() {
    for child in ["- 1", "value = {}"] {
        let source = inline_at_depth(MAX_NESTING_DEPTH - 2, child);
        let tree = parse(&source).unwrap();
        assert_eq!(parse(&stringify(&tree).unwrap()).unwrap(), tree);
        let error = parse(&inline_at_depth(MAX_NESTING_DEPTH - 1, child)).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidIndentation);
        assert!(error.message.contains("Maximum nesting depth"));
    }
}

#[test]
fn colon_structural_tokens_count_toward_the_existing_budget() {
    // Header(3), indent(1), assignment(4), dedent(1), plus final EOF.
    let source = "key:\n  v = 1\n".repeat(MAX_TOKENS / 9);
    assert_eq!(tokenize(&source).unwrap().len(), (MAX_TOKENS / 9) * 9 + 1);
    let error = tokenize(&(source + "key:\n  v = 1\n")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::SyntaxError);
    assert!(error.message.contains("token count"));
}

#[test]
fn colon_output_bytes_count_toward_the_existing_document_budget() {
    let prefix = "items:\n  - ";
    let source = format!(
        "{prefix}{}\n",
        "x".repeat(MAX_INPUT_BYTES - prefix.len() - 1)
    );
    let tree = parse(&source).unwrap();
    assert_eq!(stringify(&tree).unwrap(), source);
    assert_eq!(
        parse(&(source + "\n")).unwrap_err().kind,
        ErrorKind::SyntaxError
    );
    let oversized = HashMap::from([(
        "items".into(),
        AxsValue::Array(vec![AxsValue::String(
            "x".repeat(MAX_INPUT_BYTES - prefix.len()),
        )]),
    )]);
    assert!(stringify(&oversized).unwrap_err().contains("output size"));
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Categories {
    categories: Vec<BTreeMap<String, Vec<String>>>,
}

#[test]
fn serde_and_demo_use_the_shared_v2_parser_and_serializer() {
    let source = "categories:\n  - UI:\n      - MUI\n      - Tailwind\n";
    let value: Categories = from_str(source).unwrap();
    assert_eq!(value.categories[0]["UI"], ["MUI", "Tailwind"]);
    assert_eq!(
        to_string(&value).unwrap(),
        "categories:\n  -\n    UI:\n      - MUI\n      - Tailwind\n"
    );
    assert_eq!(
        from_str::<Categories>(&to_string(&value).unwrap()).unwrap(),
        value
    );
    let demo = include_str!("../profile.axs");
    let tree = parse(demo).unwrap();
    assert_eq!(parse(&stringify(&tree).unwrap()).unwrap(), tree);
    assert!(from_str::<Categories>("categories\n  - UI:\n      - MUI").is_err());
}

#[test]
fn fixed_seed_inline_named_block_properties() {
    let mut seed = 0x4158_535f_5632u64;
    for case in 0..500 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let key = format!("key_{seed}");
        let text = format!("UI: {seed} = - 🚀");
        let source = format!(
            "items:\n  - {key}:\n      - {}\n    number = {}\n",
            axs::string::encode(&text),
            seed as i64
        );
        let tree = parse(&source).unwrap();
        let encoded = stringify(&tree).unwrap();
        assert_eq!(parse(&encoded).unwrap(), tree, "case {case}, seed {seed}");
        assert_eq!(stringify(&parse(&encoded).unwrap()).unwrap(), encoded);
        let legacy = source.replacen("items:", "items", 1);
        assert!(parse(&legacy).is_err());
    }
}
