# AXS parser status

The Rust lexer, token parser, primitive parser, and serializer share the grammar
below. This stabilizes the existing format; it does not assert production
readiness. The crate version remains unchanged.

## Grammar and supported values

A document is a root object (a `HashMap<String, AxsValue>`). Empty input is an
empty object. LF and CRLF line endings are supported. Keys match
`[A-Za-z_][A-Za-z0-9_-]*`; duplicate keys within an object are errors.

- `key = value` assigns a primitive or an empty collection. The first `=`
  separates the key from the value; later `=` characters belong to the value.
- `key` followed by a block indented one level introduces a nonempty object or
  array. The first entry selects the block type; object fields and array items
  cannot be mixed at the same block level.
- `- value` is an array element. A bare `-` requires a nested block.
- `- key = value` starts an inline array object. Further fields appear one level
  deeper. The first field must have `=`; a nested first field uses a bare `-`
  and an ordinary object block instead.
- After `- `, a value starting with `"` is always a quoted scalar, including
  strings containing `=`. Otherwise a valid key before the first `=` selects
  an inline object. An invalid key prefix, e.g. `bad key = text`, remains a
  string for compatibility. Quote `name = text` when an array string is intended.
- Indentation uses two ASCII spaces per level. Root entries are unindented;
  increases must be exactly one level and dedentation must return to an open
  block. Unexpected indentation after a scalar is an error.
- Surrounding whitespace on keys and values is trimmed (Unicode whitespace
  included), except within quotes. Leading non-ASCII whitespace on content
  lines is invalid indentation. Raw tabs are forbidden everywhere, including
  comments; `\t` inside a quoted string is allowed.
- Blank lines and full-line comments whose trimmed text starts with `#` do not
  affect nesting. `#` within an assignment or array element is literal text,
  not an inline comment. `"text" # suffix` is an invalid quoted value.

Supported values are strings, signed `i64` integers, finite `f64` floats,
booleans, null, arrays, and objects. `[]` and `{}` denote empty collections;
nonempty collections require blocks. Reserved literals are exactly `true`,
`false`, `null`, `[]`, and `{}`.

Numeric grammar (ASCII digits only):

```text
integer = "-"? ("0" | [1-9][0-9]*)
float   = integer "." [0-9]+
```

Valid numeric syntax outside the `i64` range or producing a non-finite `f64`
is an `AXS008` error. Leading-zero forms, plus signs, exponent notation,
`NaN`, and `Infinity` remain strings. `-0` is integer zero; `-0.0` is a float
with its sign bit preserved. Decimal floats use Rust's `f64` rounding,
including subnormal results and underflow to signed zero. Original decimal
precision is not retained.

Quoted strings consume the entire trimmed value and support `\"`, `\\`, `\n`,
`\r`, `\t`, `\0`, and `\u{HEX}` (1–6 hex digits forming a Unicode scalar).
Unknown/incomplete escapes, unescaped inner quotes or controls, missing quotes,
and trailing text are errors. Unquoted strings may contain inner quotes but
cannot contain unescaped control characters after trimming. Whitespace and
controls in string data are representable using quotes and escapes.

## Round trips and constraints

For a root object with valid keys, supported finite values, and an encoding
within the limits below, `parse(stringify(value)) == value`. Float bit equality
is also checked, including negative zero. Serialization sorts keys, emits
ordinary blocks for inline objects, quotes reserved/numeric/ambiguous strings,
and escapes controls. Rust `f64::Display` (`to_string`) emits decimal notation;
`.0` is appended to integral-looking floats. The serializer checks the encoded
float against the numeric grammar and original bits before returning it.
Scientific notation is not introduced into the grammar.

Both parsing and serialization enforce:

- Block depth 0 through 128 inclusive, with root fields at depth 0. A child
  block adds one level. Inline array objects count as one extra level, just as
  their expanded serializer output does; continuation fields share that level.
  Empty inline collections do not require another block. The lexer checks
  physical indentation; the parser additionally checks implicit object levels.
- At most 8,388,608 UTF-8 input/output bytes (8 MiB).
- At most 262,144 tokens, including identifiers, values, structural tokens,
  newlines, and EOF. The serializer validates its output with the lexer to
  apply this same budget. Its output buffer is checked while appending.

These fixed limits are implementation constraints, not configurable options.
Recursive parsing/serialization stops at the depth limit. Invalid keys,
non-finite floats, excessive depth, size, or token count fail explicitly;
serialization returns no partial document. Low-level public string/value
helpers process individual values and do not apply document budgets.

## Errors and compatibility

`parse` and `parse_tokens` retain `Result<_, AxsError>`; `stringify` retains
`Result<String, String>`. `parser::parse` and `parse_legacy` retain their string
error signatures but now delegate to the token parser. Before removing the old
implementation, its accepted-example compatibility tests passed against the
updated token parser. Legacy error wording now includes stable AXS codes;
previous inconsistencies (quoted `=` in arrays, duplicate inline fields, and
missing safety limits) are not retained.

| Code | Meaning |
|---|---|
| AXS001 | Unexpected token / incompatible block entry |
| AXS002 | Invalid indentation, missing nested block, or excessive depth |
| AXS003 | Invalid key |
| AXS004 | Missing/invalid non-string primitive value |
| AXS005 | Duplicate key |
| AXS006 | Unexpected EOF when reading a required token |
| AXS007 | Malformed quoted string |
| AXS008 | Numeric range/non-finite conversion error |
| AXS009 | Syntax/resource limit (document bytes or token count) |

Spans are one-based lines and Unicode scalar columns/lengths, not byte offsets
or terminal display widths. Invalid values highlight their entire token;
duplicate keys highlight the duplicate occurrence. Structural errors point to
the offending token or synthetic EOF line. Input-size errors use position 1:1;
token-limit errors point to the current line or EOF. Diagnostic rendering clamps
caller-supplied spans to the source line to avoid excessive marker allocation.
Error fields, variants, codes, and token struct shapes are preserved.

## Verification and remaining limitations

The suite contains **97 integration tests**: the original 74 plus 23 focused
stabilization tests. Coverage includes quoted `=`, inline objects and duplicate
fields, comments/whitespace/CRLF, malformed strings and Unicode escapes,
invalid block transitions/dedentation, numeric boundaries, reserved strings,
finite float extremes, signed zero, collection depth boundaries, resource
budgets, stable diagnostics, and legacy entry points.

Dependency-free property tests use fixed seeds: 2,000 generated bounded trees,
20,000 float bit patterns (non-finite patterns skipped), and 5,000 generated
inputs checked for deterministic results and absence of panics. Tree round trips
also check types, float bits, and canonical reserialization. These tests are
reproducible sampling, not exhaustive proof, and have no automatic shrinking.
No cargo-fuzz harness or external dependency was added.

Verified commands: `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test`.

Remaining limitations: no exponent-number syntax, inline comments, multiline
strings, streaming parser, or configurable budgets. Source formatting, comments,
and object insertion order are not preserved. Fixed budgets bound document
processing, not total caller-owned memory or OS allocation failures. Public
`AxsValue` construction, cloning, and destruction of caller-built deeply nested
values remain the caller's responsibility. Diagnostic columns do not account
for wide glyphs or combining characters. Serialization performs a final lexical
validation pass, so it temporarily holds output and tokens together. Fuzzing and
broader platform/toolchain coverage remain future validation work.
