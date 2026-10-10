# AXS

AXS is a UTF-8, indentation-based data serialization format. Documents have an
object root; nonempty objects and arrays use blocks, and indentation is two
ASCII spaces per level. The Rust implementation includes a parser, deterministic
serializer, structured diagnostics, and Serde integration.

Grammar v2 requires `:` on every named nonempty block:

```text
user:
  name = Oleh
  age = 22
skills:
  - Rust
categories:
  - UI:
      - MUI
      - Tailwind
```

`=` assigns primitive values or empty collections (`[]`, `{}`). A bare `-`
introduces an unnamed nested collection. In `- UI:`, the child content is four
spaces deeper than the dash; sibling object fields are two spaces deeper.
Quoted strings may contain `:`, `=`, and `-` as literal text. Quote array strings
such as `UI:` or `http://host` so they are not classified as inline object headers.

This deliberately breaks the old grammar before 1.0. For example:

```axs-legacy
user
  name = Oleh
```

must become:

```text
user:
  name = Oleh
```

Bare named keys and documents mixing bare and colon block headers are rejected,
including through `parse_legacy` (which preserves its signature only). See
[the specification](spec/AXS.md) for the formal grammar and migration examples.
Public `TokenKind` adds `Colon`; exhaustive matches must handle it. Rust API
signatures, primitive typing, string escaping, and resource limits are retained.

In a consuming crate, add `axs` and enable Serde's derive feature:

```toml
[dependencies]
axs = { path = "../axs" }
serde = { version = "1", features = ["derive"] }
```

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Profile {
    name: String,
    age: u8,
    skills: Vec<String>,
    nickname: Option<String>,
    state: State,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum State {
    Ready,
    Progress { percent: u8 },
}

fn main() -> Result<(), axs::AxsError> {
    let profile = Profile {
        name: "Олег 🚀".into(),
        age: 22,
        skills: vec!["Rust".into(), "name = value".into()],
        nickname: None,
        state: State::Progress { percent: 75 },
    };

    let text = axs::to_string(&profile)?;
    let restored: Profile = axs::from_str(&text)?;
    assert_eq!(profile, restored);

    // Existing tree APIs remain available.
    let tree = axs::parse(&text)?;
    assert_eq!(tree["age"], axs::AxsValue::Integer(22));
    Ok(())
}
```

`axs::from_str<T: serde::de::DeserializeOwned>` and
`axs::to_string<T: serde::Serialize + ?Sized>` return `AxsError`. Structs and
maps with valid string-represented keys are supported at the document root.
Wrap scalar or sequence values in a struct or map. Externally tagged enums use
strings for unit variants and single-key objects for data variants.

Numeric conversions are checked: integers must fit `i64`, floats must be finite,
and narrowing or integer-to-float conversion must be exact. A serialized `f32`
is promoted exactly to `f64`; reading `0.1` directly as `f32` fails because it
would lose precision. Numeric buffered deserialization (`deserialize_any`,
including numeric untagged/flattened representations) is unsupported rather
than permitting unchecked Serde casts. `None` and unit use `null`; `Some` values
that serialize to null are rejected to avoid losing their distinction from None.

The document limits are 128 block levels, 8 MiB of UTF-8 input/output, and
262,144 tokens. Conversion has additional guards on recursion, intermediate
nodes, and string/key bytes. Grammar, mappings, errors, and remaining validation
work are documented in [PARSER_STATUS.md](PARSER_STATUS.md) and
[SERDE_STATUS.md](SERDE_STATUS.md). This milestone does not declare AXS 1.0 ready.

## Command-line interface

Install from this checkout (no registry publication is assumed):

```sh
cargo install --path . --locked
axs --help
axs --version
```

Or use `cargo run -- <command> <file>` during development:

```sh
axs check config.axs
axs fmt --check config.axs
axs fmt config.axs
axs to-json config.axs > converted.json
axs from-json config.json > converted.axs
axs check --help
axs fmt --help
axs to-json --help
axs from-json --help
```

Each command accepts one regular-file path. There is no stdin, recursive, or
multi-file mode; `--` can precede filenames starting with a dash. `check` and
`fmt --check` never modify inputs. Successful checks and formatting are silent.
Conversions write only data to stdout; errors go to stderr. Check process exit
status before consuming conversion output. Redirect to a different destination:
a shell can truncate a redirected file before the CLI reads it.

| Exit code | Meaning |
|---|---|
| 0 | Success, including help/version |
| 1 | Invalid document, unsupported conversion, resource limit, or noncanonical `fmt --check` |
| 2 | Invalid/missing command-line arguments |
| 3 | File/read/write/replacement error, including a broken stdout pipe |

Formatting uses the library's canonical Grammar v2 serializer: sorted keys,
colon block headers, expanded array objects, canonical quoting, and LF newlines.
**Comments, whitespace, original quoting, and input key order are lost.** Parsed
values and their types are retained. Empty documents format to empty files.
Canonical files are not rewritten.

In-place formatting validates and serializes fully before creating a random,
exclusive temporary file beside the input. It copies file permissions, syncs
the temporary file, rechecks the input for ordinary concurrent edits, and uses
[tempfile's persist operation](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist)
for replacement. Write/revalidation/replacement failures preserve the original
and remove the temporary file during normal error unwinding. No delete-before-
rename fallback is used. Read-only noncanonical inputs are rejected. In-place
formatting refuses leaf symlinks; read-only commands follow symlinks to regular
files. Parent directories are resolved once for formatting.

Unix permission bits and the Windows read-only attribute are preserved;
ownership, ACLs, extended attributes, and hard-link relationships are not copied.
Windows handles that deny replacement may cause exit 3. Filesystem replacement
support is required. Concurrent-edit detection is best effort, not an atomic
compare-and-swap or protection against adversarial directory writers. There is
no directory-fsync/power-loss guarantee; crashes may leave a temporary file.
Native Windows/Linux runtime checks remain pending in the release audit.

JSON import uses `serde_json`, requires object roots and valid AXS keys, and
rejects duplicate keys (including equivalent escaped names). Integer literals
must fit `i64`; they are never converted to float or string to bypass overflow.
Decimal/exponent literals become finite `f64` only when their decimal numerical
value matches shortest JSON rendering after normalization of insignificant
zeros and exponent spelling. `0.1`, `1.00`, and `1e-200` are supported; normal
binary floating-point rounding still applies. `0.10000000000000001`,
`9007199254740993.0`, overflow, and nonzero underflow are rejected instead of
silently changing the decimal value. This conservative check can reject longer
exact binary-float decimal spellings; arbitrary-precision decimal storage is
not provided. JSON `-0` becomes AXS `-0.0` to preserve its sign. JSON exponent
syntax does not expand AXS primitive grammar: output uses decimal notation.

JSON export preserves i64 digits and finite f64 round-trip bits using JSON
numbers, including floating negative zero. JSON consumers must use number types
that can retain those values; JavaScript's Number cannot exactly hold all i64
integers. Number spelling and object order are not source-preservation contracts.
The bridge is private CLI code over AxsValue; existing checked Serde adapters
and their buffered-number restrictions are unchanged.

All input files are read through an 8 MiB + 1 byte guard, then checked for UTF-8.
Both AXS and JSON conversion output are capped at 8 MiB, including trailing
newlines, and are buffered before writing stdout. I/O failure can still leave a
partially delivered stdout stream. AXS output retains the 262,144-token and
128-level limits. JSON tree conversion separately caps nodes and collection
entries at 262,144 and applies the same canonical collection-depth limit; empty
collections can remain inline at the boundary. Size limits are not total memory
quotas, and JSON fragments may be scanned again during checked conversion.
JSON syntax errors use bounded structured diagnostics with Unicode scalar
columns. Conversion errors do not invent source spans.

## Toolchain and compatibility

Minimum supported Rust version: **1.85.0** (edition 2024). The audit runs the
locked dependency graph on that compiler, not just a newer toolchain. The
GitHub Actions workflow defines stable and 1.85.0 jobs on Linux, macOS, and
Windows, but it has not run yet; only macOS (aarch64) has been verified natively.
See [RELEASE_AUDIT.md](RELEASE_AUDIT.md) and
[RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) for checks actually executed and
outstanding release conditions. The manifest remains `0.1.0`; no stable release
is declared.
The local path dependency above does not assume crates.io availability.

Before 1.0, incompatible API/grammar policy changes should use a new minor
version, with migration notes; patches should preserve documented behavior.
For a future 1.x release, compatible additions use minor versions and
incompatible API, grammar, type-mapping, or error-code changes require a major
version. An MSRV increase should be announced in a minor release. Error codes
and structured span conventions are compatibility contracts; human-readable
message wording and cropped/escaped diagnostic presentation are not.

`AxsError::render` clips long source lines and diagnostic fields and escapes
control characters; `Display` applies the same message policy. Detailed error
fields retain their original contents. Document limits are not process-memory
or CPU quotas: low-level `string::encode`, `string::decode`, and
`value_parser::parse_value` work on individual caller-owned values without a
byte budget. Caller-created deeply recursive AxsValue operations and custom
Serde implementations remain the caller's responsibility. The canonical
repository is <https://github.com/olehvitriachenko/axs>.

## Local checks

Hosted CI is not required to verify a change. From the repository root:

```sh
scripts/check.sh
```

The script runs `cargo fmt --check`, Clippy with `-D warnings`, the locked
workspace tests and doctests, and `cargo package`. It then builds and tests the
TypeScript SDK (WebAssembly build, type-check, runtime tests, and a tarball
install test), and finally runs `npm ci` and the TypeScript build for the VS
Code extension. `--skip-sdk` and `--skip-extension` omit those parts; the SDK
needs `rustup target add wasm32-unknown-unknown` and the `wasm-bindgen` CLI
version locked in `Cargo.lock`. It stops at
the first failing step and exits with that step's status. It needs a Rust
toolchain with rustfmt and Clippy, plus Node.js and npm for the extension.

## TypeScript and JavaScript

`packages/axs` contains an unpublished npm package (provisional name
`@axs-lang/core`) that runs this crate compiled to WebAssembly through the
`crates/axs-wasm` bindings, so there's no second parser and no Rust requirement
for its users:

```ts
import { parse, stringify } from "@axs-lang/core";

const config = parse("server:\n  port = 3000\n");
stringify({ id: 9223372036854775807n }); // "id = 9223372036854775807\n"
```

Integers outside JavaScript's safe range are returned as `bigint`, and unsafe
integer `number`s are rejected instead of rounded. Node.js loads the module
synchronously; browsers call `await init()` first. See
[packages/axs/README.md](packages/axs/README.md) for the numeric policy,
errors, tested environments, and build instructions.

## VS Code extension

`editors/vscode` contains an unpublished extension that provides syntax
highlighting, Smart Enter list continuation, live diagnostics (400 ms debounce),
and Format Document. Diagnostics and formatting run the `axs` CLI on a temporary
copy of the document, so formatting has the same comment/order loss described
above. The extension uses the `axs.cliPath` setting when set, otherwise
`target/debug/axs` in a development checkout, otherwise `axs` from `PATH`.

To try it, build the CLI and extension, then open the repository in VS Code and
run the **Run AXS Extension** launch configuration:

```sh
cargo build
npm --prefix editors/vscode ci
npm --prefix editors/vscode run compile
```

## License

AXS is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE)
for the complete license text.
