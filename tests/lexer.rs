use axs::lexer::{TokenKind, tokenize};

fn kinds(source: &str) -> Vec<TokenKind> {
    tokenize(source)
        .unwrap()
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn tokenizes_assignment() {
    assert_eq!(
        kinds("name = Oleh"),
        vec![
            TokenKind::Identifier("name".into()),
            TokenKind::Equals,
            TokenKind::Value("Oleh".into()),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn tokenizes_indentation() {
    assert_eq!(
        kinds("user:\n  name = Oleh\nage = 22"),
        vec![
            TokenKind::Identifier("user".into()),
            TokenKind::Colon,
            TokenKind::Newline,
            TokenKind::Indent,
            TokenKind::Identifier("name".into()),
            TokenKind::Equals,
            TokenKind::Value("Oleh".into()),
            TokenKind::Newline,
            TokenKind::Dedent,
            TokenKind::Identifier("age".into()),
            TokenKind::Equals,
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
fn tokenizes_array_objects() {
    let tokens = kinds("users:\n  - name = Oleh\n    age = 22");

    assert!(tokens.contains(&TokenKind::Dash));
    assert!(tokens.contains(&TokenKind::Identifier("name".into())));
    assert!(tokens.contains(&TokenKind::Value("Oleh".into())));
}

#[test]
fn preserves_multiword_values() {
    let tokens = kinds("message = Hello world!");

    assert!(tokens.contains(&TokenKind::Value("Hello world!".into())));
}

#[test]
fn quoted_array_item_with_equals() {
    let tokens = kinds("messages:\n  - \"Hello = World\"");

    assert!(tokens.contains(&TokenKind::Value("\"Hello = World\"".into())));
}

#[test]
fn tracks_source_positions() {
    let tokens = tokenize("name = Oleh").unwrap();

    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.column, 1);
    assert_eq!(tokens[1].span.column, 6);
    assert_eq!(tokens[2].span.column, 8);
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
    assert!(tokenize("user:\n   name = Oleh").is_err());
}

#[test]
fn rejects_invalid_indent_jump() {
    assert!(tokenize("user:\n    name = Oleh").is_err());
}

#[test]
fn rejects_tabs() {
    assert!(tokenize("name\t=\tOleh").is_err());
}

#[test]
fn rejects_invalid_key() {
    assert!(tokenize("123name = Oleh").is_err());
}
