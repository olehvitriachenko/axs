# AXS

AXS is a UTF-8, indentation-based data serialization format. Documents have an
object root; nonempty objects and arrays use blocks, and indentation is two
ASCII spaces per level. The Rust implementation includes a parser, deterministic
serializer, structured diagnostics, and Serde integration.

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

## Toolchain and compatibility

Minimum supported Rust version: **1.85.0** (edition 2024). The audit runs the
locked dependency graph on that compiler, not just a newer toolchain. CI covers
stable and 1.85.0 on Linux, macOS, and Windows. See
[RELEASE_AUDIT.md](RELEASE_AUDIT.md) for checks actually executed and outstanding
release conditions. The manifest remains `0.1.0`; no stable release is declared.
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
repository URL remains an unresolved owner decision in this checkout.

## License

AXS is licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE)
for the complete license text.
