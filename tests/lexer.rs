use axs::lexer::{TokenKind, tokenize};

fn kinds(source: &str) -> Vec<TokenKind> {
    tokenize(source)
        .unwrap()
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn tokenizes_field() {
    assert_eq!(
        kinds("name: Oleh"),
        vec![
            TokenKind::Identifier("name".into()),
            TokenKind::Colon,
            TokenKind::Value("Oleh".into()),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_indentation() {
    assert_eq!(
        kinds("user:\n  name: Oleh\nage: 22"),
        vec![
            TokenKind::Identifier("user".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Identifier("name".into()),
            TokenKind::Colon,
            TokenKind::Value("Oleh".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Identifier("age".into()),
            TokenKind::Colon,
            TokenKind::Value("22".into()),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_array() {
    assert_eq!(
        kinds("skills:\n  - Rust\n  - TypeScript"),
        vec![
            TokenKind::Identifier("skills".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Dash,
            TokenKind::Value("Rust".into()),
            TokenKind::Newline,
            TokenKind::Dash,
            TokenKind::Value("TypeScript".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_inline_array_with_length() {
    assert_eq!(
        kinds("tags[2]: [a, \"b, c\"]"),
        vec![
            TokenKind::Identifier("tags".into()),
            TokenKind::Length(2),
            TokenKind::Colon,
            TokenKind::Items(vec!["a".into(), "\"b, c\"".into()]),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_table() {
    assert_eq!(
        kinds("users{id, \"@n\"}:\n  - 1, Oleh"),
        vec![
            TokenKind::Identifier("users".into()),
            TokenKind::Columns(vec!["id".into(), "@n".into()]),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Dash,
            TokenKind::Items(vec!["1".into(), "Oleh".into()]),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_text_block() {
    assert_eq!(
        kinds("body: |\n  a\n\n    b\nnext: 1"),
        vec![
            TokenKind::Identifier("body".into()),
            TokenKind::Colon,
            TokenKind::Text("a\n\n  b".into()),
            TokenKind::Newline,
            TokenKind::Identifier("next".into()),
            TokenKind::Colon,
            TokenKind::Value("1".into()),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_array_objects() {
    let tokens = kinds("users:\n  - name: Oleh\n    age: 22");

    assert!(tokens.contains(&TokenKind::Dash));
    assert!(tokens.contains(&TokenKind::Identifier("name".into())));
    assert!(tokens.contains(&TokenKind::Value("Oleh".into())));
}

#[test]
fn preserves_multiword_values() {
    let tokens = kinds("message: Hello world: again");

    assert!(tokens.contains(&TokenKind::Value("Hello world: again".into())));
}

#[test]
fn quoted_array_item_with_colon() {
    let tokens = kinds("messages:\n  - \"Hello: World\"");

    assert!(tokens.contains(&TokenKind::Value("\"Hello: World\"".into())));
}

#[test]
fn array_item_without_space_after_colon_is_scalar() {
    let tokens = kinds("links:\n  - http://host");

    assert!(tokens.contains(&TokenKind::Value("http://host".into())));
}

#[test]
fn tracks_source_positions() {
    let tokens = tokenize("name:   Oleh").unwrap();

    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.column, 1);
    assert_eq!(tokens[1].span.column, 5);
    assert_eq!(tokens[2].span.column, 9);
}

#[test]
fn tracks_array_value_position() {
    let tokens = tokenize("skills:\n  -    Rust").unwrap();

    let value = tokens
        .iter()
        .find(|t| matches!(t.kind, TokenKind::Value(_)))
        .unwrap();

    assert_eq!(value.span.line, 2);
    assert_eq!(value.span.column, 8);
}

#[test]
fn rejects_invalid_indentation() {
    assert!(tokenize("user:\n   name: Oleh").is_err());
}

#[test]
fn rejects_invalid_indent_jump() {
    assert!(tokenize("user:\n    name: Oleh").is_err());
}

#[test]
fn rejects_tabs() {
    assert!(tokenize("name:\tOleh").is_err());
}

#[test]
fn allows_tabs_inside_text_blocks() {
    assert_eq!(kinds("log: |\n  a\tb")[2], TokenKind::Text("a\tb".into()));
}

#[test]
fn rejects_invalid_key() {
    assert!(tokenize("123name: Oleh").is_err());
}

#[test]
fn rejects_v2_assignment() {
    assert!(tokenize("name = Oleh").is_err());
}
