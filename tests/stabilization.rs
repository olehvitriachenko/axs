use std::collections::HashMap;

use axs::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, MAX_TOKENS};
use axs::{AxsError, AxsValue, ErrorKind, SourceSpan, parse, stringify, tokenize};

fn roundtrip(value: AxsValue) {
    let original = HashMap::from([("value".into(), value)]);
    let encoded = stringify(&original).unwrap();
    let restored = parse(&encoded).unwrap();
    assert_eq!(original, restored, "{encoded}");
    assert_value_bits(&original["value"], &restored["value"]);
    assert_eq!(encoded, stringify(&restored).unwrap());
}

fn assert_value_bits(left: &AxsValue, right: &AxsValue) {
    match (left, right) {
        (AxsValue::Float(a), AxsValue::Float(b)) => assert_eq!(a.to_bits(), b.to_bits()),
        (AxsValue::Array(a), AxsValue::Array(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert_value_bits(a, b);
            }
        }
        (AxsValue::Object(a), AxsValue::Object(b)) => {
            assert_eq!(a.len(), b.len());
            for (key, value) in a {
                assert_value_bits(value, &b[key]);
            }
        }
        _ => assert_eq!(left, right),
    }
}

#[test]
fn quotes_array_strings_that_resemble_inline_objects() {
    for text in [
        "name = value",
        "key=",
        "_key = 1",
        "key-name = {}",
        "name = \"a=b\"",
    ] {
        roundtrip(AxsValue::Array(vec![AxsValue::String(text.into())]));
    }
}

#[test]
fn preserves_equals_in_quoted_and_unquoted_values() {
    let values = parse("scalar = a=b=c\nitems:\n  - \"name = value\"\n  - bad key = text\n  - name = \"a=b\"\n    extra = c=d").unwrap();
    assert_eq!(values["scalar"], AxsValue::String("a=b=c".into()));
    let AxsValue::Array(items) = &values["items"] else {
        panic!("Expected array")
    };
    assert_eq!(items[0], AxsValue::String("name = value".into()));
    assert_eq!(items[1], AxsValue::String("bad key = text".into()));
    let AxsValue::Object(object) = &items[2] else {
        panic!("Expected object")
    };
    assert_eq!(object["name"], AxsValue::String("a=b".into()));
    assert_eq!(object["extra"], AxsValue::String("c=d".into()));
    assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
}

#[test]
fn comments_are_only_full_lines() {
    let values = parse("# comment\nroot:\n      # ignored indentation\n\n  value = text # literal\n  items:\n    # comment\n    - # literal\n    - name = # literal\n      # comment\n      other = true # literal").unwrap();
    assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
    let AxsValue::Object(root) = &values["root"] else {
        panic!("Expected object")
    };
    assert_eq!(root["value"], AxsValue::String("text # literal".into()));
    assert_eq!(
        parse("n = 22 # literal").unwrap()["n"],
        AxsValue::String("22 # literal".into())
    );
    assert_eq!(
        parse("n = \"ok\" # suffix").unwrap_err().kind,
        ErrorKind::InvalidString
    );
}

#[test]
fn whitespace_and_crlf_are_consistent() {
    let source =
        "user:\r\n  name  =   Oleh  \r\n  items:\r\n    -    Rust  \r\n    - \"  Kyiv  \"\r\n";
    let values = parse(source).unwrap();
    assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
    assert_eq!(
        parse("name = \u{2003}Oleh\u{2003}").unwrap()["name"],
        AxsValue::String("Oleh".into())
    );
    let error = parse("  \u{2003}name = Oleh").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert_eq!(error.span.unwrap().column, 3);
    assert_eq!(
        parse("#\tcomment").unwrap_err().kind,
        ErrorKind::InvalidIndentation
    );
}

#[test]
fn rejects_malformed_strings_without_panicking() {
    for raw in [
        "\"",
        "\"open",
        "\"a\"tail",
        "\"a\"\"",
        "\"\\\"",
        "\"\\q\"",
        "\"\\u\"",
        "\"\\u{}\"",
        "\"\\u{D800}\"",
        "\"\\u{110000}\"",
        "\"\\u{1234567}\"",
        "\"\\u{nothex}\"",
        "\"\\u{1F680\"",
        "\"a\0b\"",
    ] {
        assert!(axs::string::decode(raw).is_err(), "{raw:?}");
        assert!(axs::value_parser::parse_value(raw).is_err(), "{raw:?}");
        let error = parse(&format!("value = {raw}")).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidString, "{raw:?}");
        let span = error.span.unwrap();
        assert_eq!(span.column, 9);
        assert_eq!(span.length, raw.chars().count());
    }
}

#[test]
fn unicode_and_control_strings_roundtrip() {
    for text in [
        "",
        "\"",
        "\\",
        "\0\x01\n\r\t\x7f",
        "\u{85}\u{2028}",
        "Олег 🚀 = Київ",
        "# comment",
        "-",
        "- item",
        "  whitespace  ",
    ] {
        roundtrip(AxsValue::String(text.into()));
        roundtrip(AxsValue::Array(vec![AxsValue::String(text.into())]));
        assert_eq!(
            axs::string::decode(&axs::string::encode(text)).unwrap(),
            text
        );
    }
    assert_eq!(
        parse("x = \"\\u{1F680}\\u{000041}\"").unwrap()["x"],
        AxsValue::String("🚀A".into())
    );
    assert_eq!(parse("x = a\0b").unwrap_err().kind, ErrorKind::InvalidValue);
}

#[test]
fn rejects_invalid_block_structure_and_dedentation() {
    for source in [
        "  x = 1",
        "x = 1\n  y = 2",
        "x:\n    y = 2",
        "x:\n  y:\n    z = 1\n   a = 2",
        "x:\n  - one\n    - two",
        "x:\n  - one\n  y = 2",
        "x:\n  a = 1\n  - two",
        "x:\n  -\ny = 2",
        "x:\n  - name = one\n      age = 22",
        "x:\n  - name = one\n    - two",
    ] {
        assert!(parse(source).is_err(), "{source}");
    }
    let error = parse("x = 1\n  y = 2").unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert_eq!(error.span.unwrap().line, 2);
    let values = parse("x:\n  a:\n    b = 1\ny = 2").unwrap();
    assert_eq!(values["y"], AxsValue::Integer(2));
}

#[test]
fn empty_document_and_collections_roundtrip() {
    assert!(parse("# comment\n\n  # ignored").unwrap().is_empty());
    assert_eq!(stringify(&HashMap::new()).unwrap(), "");
    roundtrip(AxsValue::Array(vec![
        AxsValue::Array(vec![]),
        AxsValue::Object(HashMap::new()),
        AxsValue::Null,
        AxsValue::Boolean(false),
    ]));
}

#[test]
fn numeric_grammar_does_not_expand() {
    for text in [
        "1e10", "1.0e-10", "NaN", "Infinity", "inf", "+1", ".5", "1.", "00", "-01", "01.5", "--1",
        "1_000", "１２", "0x10",
    ] {
        assert_eq!(
            parse(&format!("n = {text}")).unwrap()["n"],
            AxsValue::String(text.into())
        );
        roundtrip(AxsValue::String(text.into()));
    }
    for text in ["0", "-0", "1", "-1"] {
        assert!(matches!(
            parse(&format!("n = {text}")).unwrap()["n"],
            AxsValue::Integer(_)
        ));
    }
    for text in ["0.0", "-0.0", "1.5", "-1.5"] {
        assert!(matches!(
            parse(&format!("n = {text}")).unwrap()["n"],
            AxsValue::Float(_)
        ));
    }
}

#[test]
fn reserved_and_out_of_range_numeric_strings_keep_their_type() {
    for text in [
        "true",
        "false",
        "null",
        "[]",
        "{}",
        "0",
        "-0",
        "1.0",
        "-0.0",
        "9223372036854775808",
        "-9223372036854775809",
    ] {
        roundtrip(AxsValue::String(text.into()));
        roundtrip(AxsValue::Array(vec![AxsValue::String(text.into())]));
    }
    roundtrip(AxsValue::String(format!("{}.0", "9".repeat(400))));
}

#[test]
fn finite_float_extremes_and_negative_zero_roundtrip() {
    for number in [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::MAX,
        f64::MIN,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        1e-200,
        1e200,
    ] {
        roundtrip(AxsValue::Float(number));
    }
    let AxsValue::Float(zero) = parse("n = -0.0").unwrap()["n"] else {
        panic!("Expected float")
    };
    assert_eq!(zero.to_bits(), (-0.0f64).to_bits());
    // Decimal-to-binary conversion uses f64 rounding, including underflow.
    let source = format!("n = -0.{}1", "0".repeat(400));
    let AxsValue::Float(zero) = parse(&source).unwrap()["n"] else {
        panic!("Expected float")
    };
    assert_eq!(zero.to_bits(), (-0.0f64).to_bits());
}

#[test]
fn rejects_unsupported_values_explicitly() {
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let values = HashMap::from([("n".into(), AxsValue::Array(vec![AxsValue::Float(number)]))]);
        assert!(stringify(&values).unwrap_err().contains("Non-finite"));
    }
    for key in ["", "1key", "bad key", "ключ", "a=b", "a\n"] {
        assert!(
            stringify(&HashMap::from([(key.into(), AxsValue::Null)]))
                .unwrap_err()
                .contains("Invalid AXS key")
        );
    }
}

fn nested_source(depth: usize, tail: &str) -> String {
    let mut source = String::new();
    for level in 0..depth {
        source.push_str(&"  ".repeat(level));
        source.push_str(if level == 0 { "items:\n" } else { "-\n" });
    }
    source.push_str(&"  ".repeat(depth));
    source.push_str(tail);
    source
}

#[test]
fn nested_arrays_obey_the_same_depth_limit() {
    let source = nested_source(MAX_NESTING_DEPTH, "- 1");
    let values = parse(&source).unwrap();
    assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
    assert_eq!(
        parse(&nested_source(MAX_NESTING_DEPTH + 1, "- 1"))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidIndentation
    );
}

#[test]
fn inline_objects_count_their_implicit_depth() {
    let mut source = nested_source(MAX_NESTING_DEPTH - 1, "- key = 1\n");
    source.push_str(&"  ".repeat(MAX_NESTING_DEPTH));
    source.push_str("other = 2\n");
    let values = parse(&source).unwrap();
    assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
    let error = parse(&nested_source(MAX_NESTING_DEPTH, "- key = 1")).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidIndentation);
    assert!(error.message.contains("Maximum nesting depth"));
    assert_eq!(error.span.unwrap().column, 259);
}

#[test]
fn legacy_entry_point_uses_the_same_grammar_and_limits() {
    for source in [
        "items:\n  - \"a=b\"\n  - bad key = text",
        "n = 9223372036854775808",
        "x = \"",
        "items:\n  - key = one\n    key = two",
    ] {
        assert_eq!(
            axs::parse_legacy(source),
            parse(source).map_err(|error| error.to_string())
        );
    }
    let source = nested_source(MAX_NESTING_DEPTH + 1, "- 1");
    assert!(
        axs::parse_legacy(&source)
            .unwrap_err()
            .contains("Maximum nesting depth")
    );
}

#[test]
fn serializer_rejects_excessive_depth_before_recursing_further() {
    for use_arrays in [false, true] {
        let mut value = AxsValue::Integer(1);
        for _ in 0..MAX_NESTING_DEPTH {
            value = if use_arrays {
                AxsValue::Array(vec![value])
            } else {
                AxsValue::Object(HashMap::from([("child".into(), value)]))
            };
        }
        roundtrip(value.clone());
        let value = AxsValue::Array(vec![value]);
        assert!(
            stringify(&HashMap::from([("value".into(), value)]))
                .unwrap_err()
                .contains("Maximum nesting depth")
        );
    }
}

#[test]
fn spans_cover_entire_invalid_values_and_unicode_columns() {
    let error = parse("x = 🚀\nn = 9223372036854775808").unwrap_err();
    assert_eq!(error.span.unwrap(), SourceSpan::new(2, 5, 19));
    let error = parse("items:\n  - name = \"🚀\\q\"").unwrap_err();
    assert_eq!(error.span.unwrap(), SourceSpan::new(2, 12, 5));
    let error = parse("items:\n  - name = a\n    name = b").unwrap_err();
    assert_eq!(error.span.unwrap(), SourceSpan::new(3, 5, 4));
}

#[test]
fn diagnostic_rendering_clamps_caller_supplied_spans() {
    let error = AxsError::new(ErrorKind::SyntaxError, "bad span").with_span(SourceSpan::new(
        1,
        usize::MAX,
        usize::MAX,
    ));
    assert!(error.render("🚀", "input.axs").len() < 200);
}

#[test]
fn input_and_output_byte_budgets_are_enforced() {
    let mut comment = "#".repeat(MAX_INPUT_BYTES);
    assert!(parse(&comment).unwrap().is_empty());
    comment.push('#');
    let error = parse(&comment).unwrap_err();
    assert_eq!(error.kind, ErrorKind::SyntaxError);
    assert!(error.message.contains("input size"));
    let value = HashMap::from([("v".into(), AxsValue::String("x".repeat(MAX_INPUT_BYTES)))]);
    assert!(stringify(&value).unwrap_err().contains("output size"));
    // Escaping can exceed the output budget even when the original string fits.
    let value = HashMap::from([(
        "v".into(),
        AxsValue::String("\0".repeat(MAX_INPUT_BYTES / 2)),
    )]);
    assert!(stringify(&value).unwrap_err().contains("output size"));
}

#[test]
fn token_budgets_are_shared_by_reader_and_writer() {
    assert!(tokenize(&"x = 0\n".repeat(MAX_TOKENS / 4 - 1)).is_ok());
    let error = tokenize(&"x = 0\n".repeat(MAX_TOKENS / 4)).unwrap_err();
    assert_eq!(error.kind, ErrorKind::SyntaxError);
    assert!(error.message.contains("token count"));
    let value = HashMap::from([(
        "v".into(),
        AxsValue::Array(vec![AxsValue::Null; MAX_TOKENS / 2]),
    )]);
    assert!(stringify(&value).unwrap_err().contains("token count"));
}

// Fixed-seed, bounded generators provide reproducible property tests without
// external dependencies. Failures report the seed/case; there is no shrinker.
struct Generator(u64);
impl Generator {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn value(&mut self, depth: usize) -> AxsValue {
        let kind = self.next() % if depth == 0 { 5 } else { 7 };
        match kind {
            0 => {
                let mut text = String::new();
                for _ in 0..self.next() % 30 {
                    text.push(char::from_u32((self.next() % 0x110000) as u32).unwrap_or('"'));
                }
                AxsValue::String(text)
            }
            1 => AxsValue::Integer(self.next() as i64),
            2 => {
                let bits = self.next();
                let value = f64::from_bits(bits);
                AxsValue::Float(if value.is_finite() { value } else { -0.0 })
            }
            3 => AxsValue::Boolean(self.next() & 1 == 0),
            4 => AxsValue::Null,
            5 => AxsValue::Array(
                (0..self.next() % 5)
                    .map(|_| self.value(depth - 1))
                    .collect(),
            ),
            _ => AxsValue::Object(
                (0..self.next() % 5)
                    .map(|index| (format!("k_{index}"), self.value(depth - 1)))
                    .collect(),
            ),
        }
    }
}

#[test]
fn generated_supported_trees_roundtrip_with_types_and_float_bits() {
    let mut generator = Generator(0x4158_535f_5452_4545);
    for _ in 0..2_000 {
        roundtrip(generator.value(5));
    }
}

#[test]
fn generated_finite_float_bits_roundtrip() {
    let mut generator = Generator(0x4158_535f_464c_4f41);
    for _ in 0..20_000 {
        let number = f64::from_bits(generator.next());
        if number.is_finite() {
            roundtrip(AxsValue::Float(number));
        }
    }
}

#[test]
fn generated_inputs_are_deterministic_and_never_panic() {
    let mut generator = Generator(0x4158_535f_494e_5054);
    let alphabet = [
        ' ', '\n', '\r', '\t', '=', '-', '#', '"', '\\', '{', '}', '0', '9', 'a', '_', '🚀', '\0',
        '\u{2003}',
    ];
    for case in 0..5_000 {
        let mut source = String::new();
        for _ in 0..generator.next() % 150 {
            source.push(alphabet[generator.next() as usize % alphabet.len()]);
        }
        let result = std::panic::catch_unwind(|| {
            assert_eq!(parse(&source), parse(&source));
            assert_eq!(tokenize(&source), tokenize(&source));
            let _ = axs::value_parser::parse_value(&source);
            let _ = axs::string::decode(&source);
            if let Ok(values) = parse(&source) {
                assert_eq!(parse(&stringify(&values).unwrap()).unwrap(), values);
            }
        });
        assert!(result.is_ok(), "case {case}: {source:?}");
    }
}
