# AXS parser status

The Rust lexer, token parser, primitive parser, and serializer implement
Grammar v3 from [spec/AXS.md](spec/AXS.md). Grammar v3 replaces Grammar v2
([spec/AXS-v2.md](spec/AXS-v2.md)) without a compatibility mode; it does not
assert production readiness. The crate version remains unchanged.

## Grammar and supported values

A document is a root object, `AxsObject` (`IndexMap<String, AxsValue>`), which
keeps keys in source order; equality ignores order. Empty input is an empty
object. LF and CRLF line endings are supported. Bare keys match
`[A-Za-z_][A-Za-z0-9_.-]*`; any other key is a quoted string. Duplicate keys
within an object are errors, including `a` and `"a"`.

- `key: value` holds a scalar, `[]`, `{}`, an inline array, or `|` (a text
  block). The first `:` followed by a space or the line end is structural;
  later colons, `=`, and `#` belong to the value.
- `key:` followed by a block indented one level introduces a nonempty object or
  array. The first entry selects the block type; mixing is an error. `key[N]:`
  and `key[N]: [...]` declare a length that must match. `key{a,b}:` is a table
  whose `x,y` rows (no `-` prefix) each become an object with the header's keys.
  `key: {a: 1,b: x}` is an inline object of primitive values.
- `- value` is an array element. A bare `-` requires a nested block.
- `- key: value` starts an inline array object; further fields are two spaces
  deeper than the dash. When the first field opens a block (object, array,
  table, or text block), its content is four spaces deeper than the dash. The
  lexer emits two Indent tokens for that increase and records both levels.
- After `- `, content is a field only when a bare or quoted key is followed by
  `:` and then a space or the line end. Otherwise it is a scalar, so
  `- http://host`, `- 10:30`, and `- bad key: x` are strings.
- Inline arrays, inline objects, and table cells hold primitives only and split
  on commas outside quotes; only a leading `"` (or, in inline objects, one right
  after `: `) opens a quoted element. Empty elements, trailing commas, nested
  collections, and unquoted `[`, `]`, `{`, `}` are errors. A table row that is
  `-` or starts with `- ` is an error.
- Text blocks keep everything indented one level deeper than their key (tabs,
  `#`, quotes) with that indentation removed, join lines with LF, drop trailing
  blank lines, and have no final newline. They need one non-blank line.
- Indentation uses two ASCII spaces per level; increases are one level except
  after `- key:` headers. Dedentation must return to an open level. Leading
  non-ASCII whitespace is invalid indentation. Raw tabs are forbidden outside
  text blocks.
- Blank lines and full-line `#` comments do not affect nesting. There are no
  inline comments.

Supported values are strings, signed `i64` integers, finite `f64` floats,
booleans, null, arrays, and objects. Reserved literals are exactly `true`,
`false`, `null`, `[]`, and `{}`. Unquoted values starting with `[` or `{` (other
than inline arrays and the empty literals) are errors.

Numeric grammar (ASCII digits only):

```text
integer = "-"? ("0" | [1-9][0-9]*)
float   = integer ("." [0-9]+)? ([eE] [+-]? [0-9]+)?   (fraction or exponent required)
```

Numbers outside the `i64` range or producing a non-finite `f64` are `AXS008`
errors. Leading-zero forms, plus signs, `.5`, `NaN`, and `Infinity` remain
strings. `-0` is integer zero; `-0.0` is a float with its sign bit preserved.

Quoted strings support `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, and `\u{HEX}`
(1–6 hex digits forming a Unicode scalar). Unknown escapes, unescaped quotes or
controls, and trailing text are errors.

## Round trips and constraints

For a root object with supported finite values within the limits below,
`parse(stringify(value)) == value`, including key order and float bits
(tested by `tests/conformance.rs` and the property tests). The serializer keeps
key order, writes arrays of uniform primitive-valued objects as tables, arrays of
primitives inline, and everything else as blocks; writes lengths on every
nonempty array; uses text blocks for multiline strings they can represent; and
writes objects with one to four primitive fields inline, uses `,` without
spaces, and quotes keys and strings only where their position requires it. Floats use the
shortest round-trip digits, with exponents only below `1e-6` or from `1e21`, as
in JavaScript; the encoding is re-parsed and bit-checked before it is returned.
`value::sort_keys` (and `axs fmt --sort-keys`) sorts keys recursively.

Both parsing and serialization enforce:

- Block depth 0 through 128 inclusive, with root fields at depth 0. A child
  block adds one level; inline array objects and table rows count as one extra
  level. The lexer checks physical indentation; the parser checks implicit
  levels. Parsing and serializing depth 128 fits in a 1 MiB stack.
- At most 8,388,608 UTF-8 input/output bytes (8 MiB).
- At most 262,144 tokens, counting structural tokens, each inline array element,
  table cell, and table column, and EOF. The serializer validates its output
  with the lexer to apply the same budget.

These fixed limits are not configurable. Excessive depth, size, or token count
and non-finite floats fail explicitly; serialization returns no partial
document. Low-level public string/value helpers do not apply document budgets.

## Errors and compatibility

`parse` and `parse_tokens` return `Result<AxsObject, AxsError>`; `stringify`
returns `Result<String, String>`, and `serializer::stringify_typed` the
structured error. `parser::parse` and `parse_legacy` keep their string-error
signatures and use the same Grammar v3 parser.

| Code | Meaning |
|---|---|
| AXS001 | Unexpected token, missing `:` or space after it, mixed block |
| AXS002 | Invalid indentation or excessive depth |
| AXS003 | Invalid key |
| AXS004 | Missing or invalid value, empty text block |
| AXS005 | Duplicate key |
| AXS006 | Unexpected EOF when reading a required token |
| AXS007 | Malformed quoted string or key |
| AXS008 | Numeric range/non-finite conversion error |
| AXS009 | Syntax/resource limit (document bytes or token count) |
| AXS010 | Declared array length does not match |
| AXS011 | Malformed inline array, inline object, table header, or table row |

Spans are one-based lines and Unicode scalar columns/lengths. Invalid values
highlight their token; duplicate keys highlight the duplicate; a missing value
points at its `:`. Diagnostic rendering crops the source to 100 Unicode scalars
around the marker, clips message/help to 1,024 scalars and filenames to 256,
and escapes control characters.

Public API changes from Grammar v2: `AxsValue::Object` and the parse result are
`AxsObject` instead of `HashMap`; `TokenKind` drops `Equals` and adds `Length`,
`Columns`, `Items`, `Fields`, and `Text`, and table rows no longer emit `Dash`; `ErrorKind` adds `LengthMismatch` and
`InvalidCollection`. Serde deserialization visits fields in document order, and
serialization keeps struct field order (a `HashMap` keeps its own iteration
order, as with serde_json).

## Verification and remaining limitations

Grammar v3 verification: 201 integration tests, two CLI unit tests, and one
README doctest. `tests/conformance.rs` runs every fixture in
`tests/conformance/` (21 valid documents with expected JSON, 23 invalid
documents) and checks canonical round trips. `tests/grammar_v3.rs` covers
canonical forms, tables, text blocks, inline blocks, positions, quoting in each
position, depth 128 boundaries, budgets, Serde, and a fixed-seed property test.
Earlier suites were migrated to v3 syntax; tests that asserted v2-only behavior
(sorted output, `=` assignments, rejected non-bare keys) were rewritten to
assert the v3 behavior instead.

Dependency-free property tests use fixed seeds: 2,000 generated bounded trees,
20,000 float bit patterns (non-finite patterns skipped), and 5,000 generated
inputs checked for deterministic results and absence of panics. These tests are
reproducible sampling, not exhaustive proof, and have no automatic shrinking.
No cargo-fuzz harness was added. Serde integration is documented separately in
`SERDE_STATUS.md`.

Remaining limitations: no repair mode for model mistakes, no query language,
no inline comments, streaming parser, or configurable budgets. Comments and
source formatting are not preserved. JSON documents with non-object roots are
rejected by `axs from-json` rather than wrapped. Fixed budgets bound document
processing, not total caller-owned memory. Diagnostic columns do not account
for wide glyphs. The TypeScript SDK, VS Code extension, and their docs still
describe Grammar v2 until they are rebuilt against this crate.
