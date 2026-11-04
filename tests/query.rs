use axs::query::{MAX_CONDITION_DEPTH, MAX_QUERY_CHARS};
use axs::{AxsValue, ErrorKind, SourceSpan, parse, query};

const DOCUMENT: &str = "\
repo: axs
meta: {stars: 42,open: true}
users[4]{name,age,city}:
  Oleh,22,Kyiv
  Alex,25,Lviv
  Maria,31,Kyiv
  Ivan,19.5,null
issues[3]:
  - number: 1
    state: open
    labels[2]{name,color}:
      bug,red
      docs,blue
    tags[2]: [core,parser]
  - number: 2
    state: closed
    labels[1]{name,color}:
      docs,blue
    tags[1]: [docs]
  - number: 3
    state: open
    labels: []
    tags: []
";

fn run(text: &str) -> String {
    let document = parse(DOCUMENT).unwrap();
    let result = query(&document, text).unwrap_or_else(|error| panic!("{text}: {error}"));
    let wrapped = match result {
        AxsValue::Object(object) => object,
        other => axs::AxsObject::from([("result".to_owned(), other)]),
    };
    axs::stringify(&wrapped).unwrap()
}

fn fails(text: &str) -> axs::AxsError {
    let document = parse(DOCUMENT).unwrap();
    let error = query(&document, text).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidQuery, "{text}");
    error
}

#[test]
fn paths_select_fields_and_elements() {
    assert_eq!(run("repo"), "result: axs\n");
    assert_eq!(run("meta"), "stars: 42\nopen: true\n");
    assert_eq!(run("meta.stars"), "result: 42\n");
    assert_eq!(run("users[0]"), "name: Oleh\nage: 22\ncity: Kyiv\n");
    assert_eq!(run("users[-1].name"), "result: Ivan\n");
    assert_eq!(run("users[ 1 ].city"), "result: Lviv\n");
    assert_eq!(run(".").lines().next(), Some("repo: axs"));
    assert_eq!(run("").lines().next(), Some("repo: axs"));
    assert_eq!(run(".repo"), "result: axs\n");
}

#[test]
fn fields_and_projections_apply_to_every_array_element() {
    assert_eq!(run("users.name"), "result[4]: [Oleh,Alex,Maria,Ivan]\n");
    assert_eq!(run("users[*].age"), "result[4]: [22,25,31,19.5]\n");
    assert_eq!(
        run("users{name,city}"),
        "result[4]{name,city}:\n  Oleh,Kyiv\n  Alex,Lviv\n  Maria,Kyiv\n  Ivan,null\n"
    );
    // Missing projected fields are left out; elements without a field are skipped.
    assert_eq!(run("meta{stars,missing}"), "stars: 42\n");
    assert_eq!(
        run("issues.labels.name"),
        "result[3]:\n  - [bug,docs]\n  - [docs]\n  - []\n"
    );
    assert_eq!(run("meta[*]"), "result[2]: [42,true]\n");
}

#[test]
fn filters_compare_numbers_strings_and_literals() {
    assert_eq!(run("users[age > 22].name"), "result[2]: [Alex,Maria]\n");
    assert_eq!(
        run("users[age >= 22].name"),
        "result[3]: [Oleh,Alex,Maria]\n"
    );
    assert_eq!(run("users[age < 20].name"), "result[1]: [Ivan]\n");
    assert_eq!(run("users[age <= 19.5].name"), "result[1]: [Ivan]\n");
    assert_eq!(run("users[age == 22.0].name"), "result[1]: [Oleh]\n");
    assert_eq!(run("users[city == Kyiv].name"), "result[2]: [Oleh,Maria]\n");
    assert_eq!(run("users[city != Kyiv].name"), "result[2]: [Alex,Ivan]\n");
    assert_eq!(run("users[city == null].name"), "result[1]: [Ivan]\n");
    assert_eq!(run("users[name < B].name"), "result[1]: [Alex]\n");
    assert_eq!(run("users[name ~ \"ri\"].name"), "result[1]: [Maria]\n");
    // A quoted number is a string and never equals the number.
    assert_eq!(run("users[age == \"22\"].name"), "result: []\n");
    // Ordering across types is false rather than an error.
    assert_eq!(run("users[city > 1].name"), "result: []\n");
    assert_eq!(run("users[nope == 1]"), "result: []\n");
}

#[test]
fn conditions_combine_with_and_or_and_parentheses() {
    assert_eq!(
        run("users[city == Kyiv and age > 25].name"),
        "result[1]: [Maria]\n"
    );
    assert_eq!(
        run("users[city == Lviv or age < 20].name"),
        "result[2]: [Alex,Ivan]\n"
    );
    // `and` binds tighter than `or`.
    assert_eq!(
        run("users[city == Lviv or city == Kyiv and age > 25].name"),
        "result[2]: [Alex,Maria]\n"
    );
    assert_eq!(
        run("users[(city == Lviv or city == Kyiv) and age > 21].name"),
        "result[3]: [Oleh,Alex,Maria]\n"
    );
}

#[test]
fn contains_checks_substrings_and_array_members() {
    assert_eq!(run("issues[tags ~ parser].number"), "result[1]: [1]\n");
    assert_eq!(
        run("issues[labels.name ~ docs].number"),
        "result[2]: [1,2]\n"
    );
    assert_eq!(
        run("issues[state == open and labels.name ~ bug]{number,state}"),
        "result[1]{number,state}:\n  1,open\n"
    );
    assert_eq!(run("users.name[@ ~ a]"), "result[2]: [Maria,Ivan]\n");
    assert_eq!(run("issues.number[@ != 2]"), "result[2]: [1,3]\n");
}

#[test]
fn steps_chain_left_to_right() {
    assert_eq!(run("users[city == Kyiv][-1].name"), "result: Maria\n");
    assert_eq!(run("issues[0].labels[1]{name}"), "name: docs\n");
    assert_eq!(
        run("issues[state == open].tags"),
        "result[2]:\n  - [core,parser]\n  - []\n"
    );
}

#[test]
fn quoted_names_reach_any_key() {
    let document = parse("\"@id\": x\n\"a.b\": {\"c d\": 1}\n").unwrap();
    assert_eq!(
        query(&document, "\"@id\"").unwrap(),
        AxsValue::String("x".into())
    );
    assert_eq!(
        query(&document, "\"a.b\".\"c d\"").unwrap(),
        AxsValue::Integer(1)
    );
}

#[test]
fn evaluation_errors_name_the_problem() {
    let error = fails("meta.nope");
    assert_eq!(error.message, "No field 'nope'");
    assert_eq!(error.span, Some(SourceSpan::new(1, 6, 4)));
    assert_eq!(error.help.as_deref(), Some("Available fields: stars, open"));
    let error = fails("users[9]");
    assert!(error.message.contains("out of range for 4 elements"));
    assert_eq!(error.span, Some(SourceSpan::new(1, 6, 3)));
    assert!(fails("users[-5]").message.contains("out of range"));
    assert!(fails("repo[0]").message.contains("needs an array"));
    assert!(fails("repo.x").message.contains("is a string"));
    assert!(fails("meta[stars > 1]").message.contains("needs an array"));
}

#[test]
fn syntax_errors_point_at_the_query() {
    for (text, column) in [
        ("users[age = 1]", 11),
        ("users[age >]", 12),
        ("users[age > 1", 14),
        ("users{name", 11),
        ("users.", 7),
        ("users..name", 7),
        ("users name", 7),
        ("1users", 1),
        ("users[(age > 1]", 15),
        ("users[\"x > 1]", 7),
        ("users[age > [1]]", 13),
    ] {
        let error = fails(text);
        assert_eq!(
            error.span.map(|span| span.column),
            Some(column),
            "{text}: {error}"
        );
    }
}

#[test]
fn query_size_and_nesting_are_bounded() {
    let long = format!("users[name == {}]", "x".repeat(MAX_QUERY_CHARS));
    assert!(fails(&long).message.contains("limited"));
    let deep = format!(
        "users[{}age > 1{}]",
        "(".repeat(MAX_CONDITION_DEPTH + 1),
        ")".repeat(MAX_CONDITION_DEPTH + 1)
    );
    assert!(fails(&deep).message.contains("nested too deeply"));
    let fine = format!(
        "users[{}age > 1{}].name",
        "(".repeat(MAX_CONDITION_DEPTH),
        ")".repeat(MAX_CONDITION_DEPTH)
    );
    assert_eq!(run(&fine), "result[4]: [Oleh,Alex,Maria,Ivan]\n");
}
