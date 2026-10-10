# AXS Serde milestone

## API and implementation

```text
axs::from_str<T: serde::de::DeserializeOwned>(&str) -> Result<T, AxsError>
axs::to_string<T: serde::Serialize + ?Sized>(&T) -> Result<String, AxsError>
```

Deserialization uses the existing parser, wraps its root map in
`AxsValue::Object`, and visits the owned tree. Serialization builds an
`AxsValue` tree and passes its root map to the existing text serializer. There
is no second parser, new text syntax, or automatic scalar-root envelope.
Adapters are private. `parse`, `tokenize`, `stringify`, and the `AxsValue` data
model retain their existing APIs. AxsValue itself has no new Serde trait impls.

The runtime dependency is Serde with its default `std` feature. `derive` is
only a development dependency feature; consumers enable it in their own crate.
README's derive example is included in crate documentation and compiled as a
doctest. No other direct dependencies or optional feature switches were added.

## Type mapping

| Rust / Serde value | AXS tree representation |
|---|---|
| bool | Boolean |
| str, String, char | String (char requires exactly one Unicode scalar on input) |
| signed integers through i128 | Integer, checked against i64 range on output |
| unsigned integers through u128 | Integer, checked against 0..=i64::MAX on output |
| finite f64 | Float, bit-preserving text round trip |
| finite f32 | Float, promoted exactly to f64 on output |
| None, unit, unit structs | Null |
| Some(non-null value) | Contained representation |
| newtype structs | Transparent contained representation |
| sequences, arrays, tuples, tuple structs | Array |
| explicit Serde bytes | Array of checked u8 integers; no binary text primitive |
| maps, structs | Object |
| externally tagged unit variant | String with the Serde variant name |
| externally tagged newtype variant | Single-key Object: variant name -> value |
| externally tagged tuple variant | Single-key Object: variant name -> Array |
| externally tagged struct variant | Single-key Object: variant name -> Object |

Root values must lower to an Object. Structs, maps, transparent wrappers around
objects, and externally tagged data variants work. Scalar, sequence, None,
unit/unit-struct, and unit-variant roots are rejected. Empty structs/maps encode
as an empty document. Empty tuples/sequences require an enclosing field.

Keys must serialize as strings and satisfy `[A-Za-z_][A-Za-z0-9_-]*`.
String-represented keys such as transparent String newtypes, valid ASCII chars,
and unit variants are accepted; numeric keys are rejected, not stringified.
Renamed struct fields and data-variant tags must also satisfy the key grammar.
Unit-variant names are string values and are not restricted to key syntax.
Duplicate keys from custom serialization/flattening fail rather than overwrite.
Structs and map entries serialize in sorted key order; map deserialization also
sorts entries so error order is deterministic.

For example, a nested `State::Ready` field uses `state = Ready`; a nested
`State::Progress { percent: 75 }` uses:

```text
state
  Progress
    percent = 75
```

A unit variant must deserialize from its string representation. A payload
object with null is not accepted as a unit variant, so it remains distinguishable
from a newtype variant containing unit. Unknown variants, multiple enum keys,
incorrect payload kinds, and incorrect tuple lengths fail.

## Checked conversions and nullable values

Integer targets accept Integer values only, with checked target-width/sign
conversion. Float values are not coerced to integers, even when integral.
Integer-to-float conversion is allowed only if exact. Float-to-f32 conversion
must be finite and bit-preserving after promotion back to f64; overflow,
underflow, and precision loss fail. Numeric strings are not converted to numbers.
Booleans, strings, chars, null, sequences, and maps require compatible tree types.

`-0.0` preserves its sign bit. All finite f64 values and exactly promoted f32
values use the core's decimal float encoding; the grammar still does not accept
exponent notation as a number. i128/u128 inputs only support the existing i64
range. Parsing decimal text uses the core's documented f64 rounding before
Serde conversion; Serde does not add arbitrary decimal precision.

Numeric `deserialize_any` is deliberately unsupported. Serde's buffered Content
handling can otherwise narrow floats or cast integers after the adapter has
lost control of checked conversion. Default externally tagged enums are covered.
Numeric untagged enums, internally/adjacently tagged buffered payloads, numeric
flattened fields, and dynamic numeric visitors relying on `deserialize_any`
are outside the supported deserialization contract and fail explicitly when
buffering reaches a number. Non-numeric `deserialize_any` remains available.
Some such attributes can work with non-numeric payloads, but are not guaranteed
as general enum mappings. Serialization cannot detect those derive attributes
once Serde lowers them to ordinary maps/values; successful encoding alone is
not a guarantee that an unsupported target representation will deserialize.
Use concrete numeric fields and default externally tagged enums for checked
round trips. Custom visitors that deliberately cast or transform typed values
remain responsible for their own semantics.

Missing optional fields follow Serde's normal None behavior; explicit null
also deserializes as None. Required missing fields error, default fields use
`#[serde(default)]`, unknown fields are ignored unless denied, and renames are
honored. Unknown values use `deserialize_ignored_any`, so ignoring a numeric
field does not trigger buffered numeric conversion. Some values that lower to
Null (e.g. Some(()), Some(None), or custom null newtypes) are rejected on output:
otherwise deserialization would produce None. Some(Some(non-null)) is supported.
Unit fields require explicit null unless Serde defaults them. AXS does not
preserve the difference between an omitted optional field and explicit null.

## Constraints and errors

Existing document limits remain unchanged: block depths 0..=128, 8,388,608 UTF-8
input/output bytes, and 262,144 tokens including structural tokens and EOF.
Collections are checked during Serde tree construction, before further recursive
serialization. Empty collections may remain inline at the boundary. Core parsing
already checks arrays and inline objects using canonical block depth.

Additional conversion guards prevent building an unbounded intermediate tree:
512 recursive conversion edges (including transparent wrappers), 262,144
serialization callbacks/nodes, and 8 MiB aggregate string/key bytes. These are
separate from text limits: small Rust input strings can expand through escaping,
while indentation, key delimiters, and numeric text also contribute to output.
The text serializer applies the actual output-byte/token limits independently.
Serialization length hints never trigger allocations proportional to an
untrusted hint. `collect_str` formats into a bounded buffer. Deserialization
keys and enum tags use the same guarded adapter as values.

Both Serde error traits are implemented on AxsError. Type mismatches, missing
fields, malformed custom Serde protocols, and custom errors use InvalidValue
(AXS004). Checked numeric failures use InvalidNumber (AXS008); invalid keys use
AXS003; duplicates use AXS005; block depth uses AXS002; conversion/size/token
budgets use AXS009. Serde derive may wrap an underlying error in its own custom
message (e.g. untagged variant mismatch), retaining AxsError but changing it to
AXS004. The core serializer now has a private typed-error path; the public
`stringify` still returns string errors, now formatted with their AXS code.

Parser errors retain their original spans. Conversion errors have field/index
context where available and span=None because AxsValue contains no source
locations. Serialization errors do not claim source positions in Rust data.
Custom visitors that stop before consuming an entire array/map error rather
than silently discard elements. No adapter uses unwrap, expect, or panic on
user data. Arbitrary user-written Serialize/Deserialize/Display implementations
can still panic or recurse outside the adapter; their behavior cannot be guarded
by a data-format library.

## Verification and remaining release work

Baseline verified: 97 core integration tests. Serde adds 31 integration tests
covering nested structs/maps/sequences, tuple and newtype mappings, char/bytes,
Option/unit, all externally tagged variant shapes, rename/default/unknown-field
handling, roots, malformed inputs and custom map/visitor protocols, parser spans
and conversion paths, numeric bounds and exactness, depth/resource guards, and
buffered-number rejection. A fixed-seed property test runs 2,000 cases of integer,
f32/f64 bit-preserving, and special-string round trips (non-finite patterns skipped).
The README contributes one compiling doctest.

Commands: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, and `cargo test --doc`.

The release audit establishes and tests Rust 1.85.0 and adds stable/MSRV
Linux/macOS/Windows CI. Native cross-platform CI is still outstanding; see
RELEASE_AUDIT.md. Before a stable release: approve the numeric/buffered-enum
policy and nullable/root mappings, expand fuzzing/property coverage, and decide whether
source metadata should be retained for Serde type-error spans. This milestone
adds no Serde borrowing/streaming API, CLI, or bindings, and
does not declare AXS 1.0 ready.
