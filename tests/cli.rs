use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use axs::AxsValue;
use axs::lexer::{MAX_INPUT_BYTES, MAX_NESTING_DEPTH, MAX_TOKENS};
use tempfile::TempDir;

struct Fixture(TempDir);
impl Fixture {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }
    fn file(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }
    fn run(&self, command: &str, path: &Path, check: bool) -> Output {
        let mut process = Command::new(env!("CARGO_BIN_EXE_axs"));
        process.current_dir(self.0.path()).arg(command);
        if check {
            process.arg("--check");
        }
        process.arg("--").arg(path).output().unwrap()
    }
    fn entries(&self) -> usize {
        fs::read_dir(self.0.path()).unwrap().count()
    }
}
fn status(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout)
            .chars()
            .take(1024)
            .collect::<String>(),
        String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(1024)
            .collect::<String>()
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}
fn quiet_success(output: &Output) {
    status(output, 0);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn help_and_version_are_available_for_every_command() {
    for args in [
        vec!["--help"],
        vec!["check", "--help"],
        vec!["fmt", "--help"],
        vec!["to-json", "--help"],
        vec!["from-json", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_axs"))
            .args(args)
            .output()
            .unwrap();
        status(&output, 0);
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
        assert!(output.stderr.is_empty());
    }
    let output = Command::new(env!("CARGO_BIN_EXE_axs"))
        .arg("--version")
        .output()
        .unwrap();
    status(&output, 0);
    assert_eq!(output.stdout, b"axs 0.1.0\n");
}

#[test]
fn argument_errors_have_exit_code_two() {
    for args in [
        vec![],
        vec!["missing-command"],
        vec!["check"],
        vec!["fmt", "--unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_axs"))
            .args(args)
            .output()
            .unwrap();
        status(&output, 2);
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn check_validates_v2_without_modifying_or_printing_status() {
    let fixture = Fixture::new();
    let source = "# comment\nuser:\n  name = Олег\nskills:\n  - UI:\n      - MUI\n";
    let path = fixture.file("Профіль 🚀.axs", source);
    quiet_success(&fixture.run("check", &path, false));
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    assert_eq!(fixture.entries(), 1);
}

#[test]
fn invalid_documents_render_structured_diagnostics_on_stderr() {
    let fixture = Fixture::new();
    for source in [
        "user\n  age = 22",
        "x = \"bad\\q\"",
        "name = one\nname = two",
        "x = 9223372036854775808",
    ] {
        let path = fixture.file("invalid.axs", source);
        let output = fixture.run("check", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("error[AXS"));
        assert!(error.contains("invalid.axs:"));
        assert!(error.contains('^'));
        assert_eq!(fs::read_to_string(path).unwrap(), source);
    }
}

#[test]
fn io_failures_use_exit_code_three() {
    let fixture = Fixture::new();
    for path in [
        fixture.0.path().join("missing.axs"),
        fixture.0.path().to_owned(),
    ] {
        for command in ["check", "fmt", "to-json", "from-json"] {
            let output = fixture.run(command, &path, false);
            status(&output, 3);
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("I/O error"));
        }
    }
}

#[test]
fn format_check_detects_differences_without_writing() {
    let fixture = Fixture::new();
    let path = fixture.file("input.axs", "# removed\nz=1\na = 2\n");
    let before = fs::read(&path).unwrap();
    let output = fixture.run("fmt", &path, true);
    status(&output, 1);
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not canonically formatted"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fixture.entries(), 1);
    fs::write(&path, "a = 2\nz = 1\n").unwrap();
    quiet_success(&fixture.run("fmt", &path, true));
}

#[test]
fn formatting_replaces_only_after_parse_and_preserves_data() {
    let fixture = Fixture::new();
    let source = "# lost\nz=1\nusers:\n  - name = Oleh\n    skills:\n      - \"UI:\"\n";
    let path = fixture.file("input.axs", source);
    let original = axs::parse(source).unwrap();
    quiet_success(&fixture.run("fmt", &path, false));
    let formatted = fs::read_to_string(&path).unwrap();
    assert_eq!(formatted, axs::stringify(&original).unwrap());
    assert_eq!(axs::parse(&formatted).unwrap(), original);
    quiet_success(&fixture.run("fmt", &path, true));
    assert_eq!(fixture.entries(), 1);
}

#[test]
fn invalid_formatting_never_changes_input_or_leaves_temporary_files() {
    let fixture = Fixture::new();
    for source in ["key:\n  bad\n", "x = \"open", "x = 1\nx = 2"] {
        let path = fixture.file("input.axs", source);
        for check in [false, true] {
            let output = fixture.run("fmt", &path, check);
            status(&output, 1);
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
            assert_eq!(fixture.entries(), 1);
        }
    }
}

#[test]
fn readonly_input_is_preserved_on_failed_replacement() {
    let fixture = Fixture::new();
    let path = fixture.file("input.axs", "z=1\na=2\n");
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let output = fixture.run("fmt", &path, false);
    status(&output, 3);
    assert_eq!(fs::read_to_string(&path).unwrap(), "z=1\na=2\n");
    assert_eq!(fixture.entries(), 1);
    fs::set_permissions(&path, original_permissions).unwrap();
}

#[cfg(unix)]
#[test]
fn unix_permissions_are_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let path = fixture.file("input.axs", "z=1\na=2\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    quiet_success(&fixture.run("fmt", &path, false));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_readable_but_in_place_formatting_refuses_them() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.file("target.axs", "z=1\na=2\n");
    let link = fixture.0.path().join("link.axs");
    symlink(&target, &link).unwrap();
    quiet_success(&fixture.run("check", &link, false));
    status(&fixture.run("fmt", &link, true), 1);
    let output = fixture.run("fmt", &link, false);
    status(&output, 3);
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlinks"));
    assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(target).unwrap(), "z=1\na=2\n");
    assert_eq!(fixture.entries(), 2);
}

#[test]
fn preexisting_temporary_looking_file_is_never_overwritten() {
    let fixture = Fixture::new();
    let sentinel = fixture.file(".axs-existing", "sentinel");
    let path = fixture.file("input.axs", "x=1\n");
    quiet_success(&fixture.run("fmt", &path, false));
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "sentinel");
    assert_eq!(fixture.entries(), 2);
}

#[test]
fn invalid_utf8_is_rejected_without_modification() {
    let fixture = Fixture::new();
    let source = [b'x', b'=', 0xff];
    let path = fixture.file("invalid", source);
    for command in ["check", "fmt", "to-json", "from-json"] {
        let output = fixture.run(command, &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("UTF-8"));
        assert_eq!(fs::read(&path).unwrap(), source);
    }
}

#[test]
fn input_reading_is_bounded_at_eight_mib() {
    let fixture = Fixture::new();
    let exact = fixture.file("exact.axs", "#".repeat(MAX_INPUT_BYTES));
    quiet_success(&fixture.run("check", &exact, false));
    let oversized = fixture.file("oversized", " ".repeat(MAX_INPUT_BYTES + 1));
    for command in ["check", "fmt", "to-json", "from-json"] {
        let output = fixture.run(command, &oversized, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("AXS009"));
    }
    assert_eq!(
        fs::metadata(oversized).unwrap().len(),
        (MAX_INPUT_BYTES + 1) as u64
    );
}

#[test]
fn empty_documents_and_json_objects_are_supported() {
    let fixture = Fixture::new();
    let axs = fixture.file("empty.axs", "");
    quiet_success(&fixture.run("check", &axs, false));
    quiet_success(&fixture.run("fmt", &axs, false));
    let output = fixture.run("to-json", &axs, false);
    status(&output, 0);
    assert_eq!(output.stdout, b"{}\n");
    let json = fixture.file("empty.json", "{}");
    quiet_success(&fixture.run("from-json", &json, false));
    fs::write(&json, "").unwrap();
    status(&fixture.run("from-json", &json, false), 1);
}

#[test]
fn axs_to_json_preserves_nested_supported_values_and_i64_digits() {
    let fixture = Fixture::new();
    let source = "max = 9223372036854775807\nmin = -9223372036854775808\nuser:\n  name = \"UI: = - 🚀\"\n  active = true\n  absent = null\n  empty = {}\nvalues:\n  - 0.5\n  - []\n";
    let path = fixture.file("input.axs", source);
    let output = fixture.run("to-json", &path, false);
    status(&output, 0);
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["max"].as_i64(), Some(i64::MAX));
    assert_eq!(json["min"].as_i64(), Some(i64::MIN));
    assert_eq!(json["user"]["name"], "UI: = - 🚀");
    assert_eq!(json["user"]["active"], true);
    assert!(json["user"]["absent"].is_null());
    assert!(json["user"]["empty"].as_object().unwrap().is_empty());
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

#[test]
fn json_to_axs_is_canonical_v2_and_keeps_input_unchanged() {
    let fixture = Fixture::new();
    let source = r#"{"z":null,"user":{"name":"Олег","skills":["UI:","name = value"]},"empty":{}}"#;
    let path = fixture.file("input.json", source);
    let output = fixture.run("from-json", &path, false);
    status(&output, 0);
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "empty = {}\nuser:\n  name = Олег\n  skills:\n    - \"UI:\"\n    - \"name = value\"\nz = null\n"
    );
    assert_eq!(fs::read_to_string(path).unwrap(), source);
}

#[test]
fn json_root_arrays_and_scalars_are_rejected() {
    let fixture = Fixture::new();
    for source in ["[]", "[1]", "true", "null", "42", "\"text\""] {
        let path = fixture.file("root.json", source);
        let output = fixture.run("from-json", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("root must be an object"));
    }
}

#[test]
fn malformed_json_and_invalid_keys_produce_no_partial_stdout() {
    let fixture = Fixture::new();
    for source in [
        "{",
        "{\"x\":}",
        "{} trailing",
        "{\"x\":\"\\q\"}",
        "{\"x\":NaN}",
        "{\"x\":01}",
        "{\"x\":\"\\uD800\"}",
        "{\"bad key\":1}",
        "{\"ключ\":null}",
    ] {
        let path = fixture.file("bad.json", source);
        let output = fixture.run("from-json", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("AXS"));
    }
}

#[test]
fn json_syntax_diagnostics_use_unicode_scalar_columns() {
    let fixture = Fixture::new();
    let source = "{\"name\":\"🚀\",\"x\":?}";
    let path = fixture.file("Unicode 🚀.json", source);
    let output = fixture.run("from-json", &path, false);
    status(&output, 1);
    assert!(output.stdout.is_empty());
    let column = source.chars().position(|ch| ch == '?').unwrap() + 1;
    let message = String::from_utf8(output.stderr).unwrap();
    assert!(
        message.contains(&format!("Unicode 🚀.json:1:{column}")),
        "{message}"
    );
    assert!(message.contains('^'));
}

#[test]
fn duplicate_json_keys_are_never_silently_overwritten() {
    let fixture = Fixture::new();
    for source in [
        r#"{"x":1,"x":2}"#,
        r#"{"x":[{"y":1,"y":2}]}"#,
        r#"{"a":1,"\u0061":2}"#,
    ] {
        let path = fixture.file("duplicates.json", source);
        let output = fixture.run("from-json", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("AXS005"));
    }
}

#[test]
fn json_numeric_boundaries_and_precision_changing_numbers_fail() {
    let fixture = Fixture::new();
    for raw in [
        "9223372036854775808",
        "-9223372036854775809",
        "18446744073709551616",
        "1e400",
        "1e-400",
        "9007199254740993.0",
        "0.10000000000000001",
        "1e99999999999999999999",
    ] {
        let path = fixture.file("number.json", format!("{{\"n\":{raw}}}"));
        let output = fixture.run("from-json", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("AXS008"),
            "{raw}"
        );
    }
    let path = fixture.file("number.json", format!("{{\"n\":{}}}", "9".repeat(100_000)));
    let output = fixture.run("from-json", &path, false);
    status(&output, 1);
    assert!(output.stderr.len() < 4096);
}

#[test]
fn finite_float_extremes_exponents_and_negative_zero_roundtrip() {
    let fixture = Fixture::new();
    for raw in [
        "0.1",
        "1.00",
        "1E+2",
        "1e-200",
        "1e200",
        "1.7976931348623157e308",
        "5e-324",
        "-0",
        "-0.0",
        "-0e10",
        "9223372036854775807",
        "-9223372036854775808",
    ] {
        let path = fixture.file("number.json", format!("{{\"n\":{raw}}}"));
        let converted = fixture.run("from-json", &path, false);
        status(&converted, 0);
        let axs = fixture.file("number.axs", &converted.stdout);
        let original = axs::parse(&String::from_utf8(converted.stdout.clone()).unwrap()).unwrap();
        let exported = fixture.run("to-json", &axs, false);
        status(&exported, 0);
        fs::write(&path, &exported.stdout).unwrap();
        let restored = fixture.run("from-json", &path, false);
        status(&restored, 0);
        assert_eq!(converted.stdout, restored.stdout, "{raw}");
        if raw.starts_with("-0") {
            let AxsValue::Float(value) = original["n"] else {
                panic!("Negative zero must be a float")
            };
            assert_eq!(value.to_bits(), (-0.0f64).to_bits());
        }
    }
}

#[test]
fn json_depth_and_axs_depth_share_the_same_boundary() {
    let fixture = Fixture::new();
    for depth in [MAX_NESTING_DEPTH, MAX_NESTING_DEPTH + 1, 10_000] {
        let source = format!("{{\"root\":{}1{}}}", "[".repeat(depth), "]".repeat(depth));
        let path = fixture.file("deep.json", source);
        let output = fixture.run("from-json", &path, false);
        status(&output, if depth == MAX_NESTING_DEPTH { 0 } else { 1 });
        if depth == MAX_NESTING_DEPTH {
            let axs = fixture.file("deep.axs", &output.stdout);
            quiet_success(&fixture.run("check", &axs, false));
            status(&fixture.run("to-json", &axs, false), 0);
        } else {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("AXS002"));
        }
    }
}

#[test]
fn json_node_and_axs_token_budgets_are_enforced() {
    let fixture = Fixture::new();
    for size in [MAX_TOKENS / 3, MAX_TOKENS + 1] {
        let source = format!("{{\"items\":[{}null]}}", "null,".repeat(size));
        let path = fixture.file("many.json", source);
        let output = fixture.run("from-json", &path, false);
        status(&output, 1);
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("AXS009"));
    }
}

#[test]
fn conversion_output_expansion_is_bounded_before_stdout() {
    let fixture = Fixture::new();
    // AXS escaping expands this JSON document beyond the 8 MiB output budget.
    let json = fixture.file(
        "large.json",
        format!("{{\"x\":\"{}\"}}", "\\b".repeat(MAX_INPUT_BYTES / 3)),
    );
    let output = fixture.run("from-json", &json, false);
    status(&output, 1);
    assert!(output.stdout.is_empty());
    // JSON escapes controls more heavily than AXS's compact \\0 encoding.
    let axs = fixture.file(
        "large.axs",
        format!("x = \"{}\"\n", "\\0".repeat(MAX_INPUT_BYTES / 3)),
    );
    let output = fixture.run("to-json", &axs, false);
    status(&output, 1);
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn broken_stdout_pipe_is_an_io_error_without_a_panic() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    let fixture = Fixture::new();
    let path = fixture.file("input.axs", "x = 1");
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_axs"))
        .arg("to-json")
        .arg(path)
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .output()
        .unwrap();
    status(&output, 3);
    assert!(String::from_utf8_lossy(&output.stderr).contains("writing stdout"));
}

#[test]
fn fixed_seed_json_roundtrips_preserve_float_bits_and_special_strings() {
    let fixture = Fixture::new();
    let mut seed = 0x0041_5853_5f43_4c49u64;
    for case in 0..64 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let float = f64::from_bits(seed);
        if !float.is_finite() {
            continue;
        }
        let tree = std::collections::HashMap::from([
            ("number".into(), AxsValue::Float(float)),
            ("integer".into(), AxsValue::Integer(seed as i64)),
            (
                "text".into(),
                AxsValue::String(format!("UI: = - {case} 🚀\n\0")),
            ),
        ]);
        let original = axs::stringify(&tree).unwrap();
        let axs = fixture.file("property.axs", &original);
        let exported = fixture.run("to-json", &axs, false);
        status(&exported, 0);
        let json = fixture.file("property.json", &exported.stdout);
        let restored = fixture.run("from-json", &json, false);
        status(&restored, 0);
        assert_eq!(
            restored.stdout,
            original.as_bytes(),
            "case {case}, seed {seed}"
        );
    }
}

#[test]
fn reserved_numeric_and_escaped_json_strings_retain_their_types() {
    let fixture = Fixture::new();
    let strings = [
        "true",
        "false",
        "null",
        "1",
        "01",
        "1e2",
        "",
        "UI:",
        "x = y",
        "-",
        "# literal",
        "🚀\n\t\0\"\\",
    ];
    let source = serde_json::json!({ "values": strings });
    let json = fixture.file("strings.json", serde_json::to_vec(&source).unwrap());
    let encoded = fixture.run("from-json", &json, false);
    status(&encoded, 0);
    let axs = fixture.file("strings.axs", &encoded.stdout);
    let restored = fixture.run("to-json", &axs, false);
    status(&restored, 0);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&restored.stdout).unwrap(),
        source
    );
}

#[test]
fn formatting_output_limit_failure_never_replaces_the_input() {
    let fixture = Fixture::new();
    let source = format!("x=name={}\n", "x".repeat(MAX_INPUT_BYTES - 8));
    assert_eq!(source.len(), MAX_INPUT_BYTES);
    let path = fixture.file("large.axs", &source);
    let output = fixture.run("fmt", &path, false);
    status(&output, 1);
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("output size"));
    assert_eq!(fs::read_to_string(path).unwrap(), source);
    assert_eq!(fixture.entries(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn non_utf8_file_names_are_accepted_as_os_paths() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    let path = fixture
        .0
        .path()
        .join(OsString::from_vec(b"name-\xff.axs".to_vec()));
    fs::write(&path, "x=1\n").unwrap();
    quiet_success(&fixture.run("check", &path, false));
    quiet_success(&fixture.run("fmt", &path, false));
    assert_eq!(fs::read_to_string(path).unwrap(), "x = 1\n");
}

#[cfg(windows)]
#[test]
fn windows_locked_target_is_preserved_after_failed_atomic_replacement() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let path = fixture.file("locked.axs", "x=1\n");
    // Allow readers but deny deletion/replacement while this handle is open.
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let output = fixture.run("fmt", &path, false);
    status(&output, 3);
    assert_eq!(fs::read_to_string(&path).unwrap(), "x=1\n");
    assert_eq!(fixture.entries(), 1);
    drop(locked);
}
