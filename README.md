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
