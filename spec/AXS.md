# AXS — Data Format for Language Models

**Grammar Version:** 3 (pre-1.0 draft)
**Status:** Experimental, implemented by the Rust crate
**Reference Implementation:** Rust

## 1. Overview

AXS is a compact, human-readable, indentation-based data format with the JSON
data model. It is designed primarily for data that language models read and
write: tool results, structured output, and context. It aims to spend fewer
tokens than JSON on the same data while staying unambiguous and strict.

### Design principles

- Same data model as JSON, plus a distinct integer type.
- Fewer tokens than JSON: no mandatory quotes, braces, or commas for nested
  structures, and tables for arrays of uniform objects.
- Familiar to models: `key: value`, indentation, `[a, b]` lists, `|` text blocks.
- Strict and deterministic: one document has exactly one meaning, and every
  invalid document is an error. Recovery from model mistakes is a separate,
  explicit repair mode, never part of the core parser.
- Self-checking: optional array lengths detect truncated output.
- Safe: a parser never executes code.

### Changes from Grammar v2

Grammar v3 replaces Grammar v2 without a migration path.

| v2 | v3 |
|---|---|
| `name = Oleh` | `name: Oleh` |
| `tags = []`, `config = {}` | `tags: []`, `config: {}` |
| Arrays of scalars only as `-` blocks | Also inline: `tags: [a, b]` |
| No multiline strings | `body: \|` text blocks |
| Arrays of objects repeat every key | Tables: `users{name, age}:` |
| — | Optional lengths: `users[2]:` |
| Unquoted keys only | Quoted keys: `"@type": Person` |
| No exponent notation | `1.5e-7` is a float |
| Serializer sorts keys | Key order is preserved |

## 2. Encoding and documents

An AXS document is UTF-8 text. Lines end with LF or CRLF. Tabs are prohibited
except inside quoted strings (as `\t`) and inside text block content (§8).

The root of a document is an object. An empty document is an empty object.
Producers that need to encode a non-object value (for example a JSON array
returned by a tool) wrap it in an object, conventionally under the key `result`.

```axs
name: Oleh
age: 22
active: true

address:
  city: Kyiv
  country: Ukraine
```

## 3. Data model

| Type | Example | Description |
|---|---|---|
| String | `name: Oleh` | UTF-8 text |
| Integer | `age: 22` | Signed 64-bit integer |
| Float | `height: 1.76` | Finite IEEE 754 binary64 |
| Boolean | `active: true` | `true` or `false` |
| Null | `value: null` | Absence of value |
| Array | `tags: [a, b]` | Ordered sequence of values |
| Object | `user:` + block | Ordered map with unique string keys |

Object keys are unique and **ordered**: parsing preserves source order and
serialization preserves the order of the value being serialized. Two objects
with the same entries in a different order are equal as data, but serializers
must not reorder them unless explicitly asked to (for example `fmt --sort-keys`).

## 4. Keys

An unquoted key begins with an ASCII letter or `_` and continues with ASCII
letters, digits, `_`, `-`, or `.`.

Any other key is written as a quoted string (§5):

```axs
"@type": Person
"Content-Type": application/json
"1": first
"": empty key
```

`Content-Type` needs no quotes; it is quoted above only to show that quoting is
always allowed. The canonical form leaves a key
unquoted whenever it can.

## 5. Strings

### Unquoted strings

A value that is not a reserved literal (§7), a number (§6), or one of the
structural forms below is an unquoted string. Surrounding spaces are trimmed.

```axs
message: Hello, world!
url: https://example.com/a?b=c#d
time: 10:30
title: Fix: crash on start
```

Everything after the first structural `: ` is the value, so later colons,
`#`, and `=` are literal text.

A string must be quoted when it:

- is empty, or begins or ends with a space;
- equals `true`, `false`, `null`, or matches number syntax (`"123"`, `"1.5"`);
- begins with `"`, `[`, or `{`;
- is exactly `|`;
- contains a control character (including LF, CR, and tab), unless it is
  written as a text block (§8);
- appears in an inline array or table cell and contains `,`, `[`, or `]`
  (see §9 and §10);
- appears as a `- ` array item and would read as an object field, such as
  `UI:` or `note: x` (see §9).

Inside inline arrays and table cells only a leading `"` starts a quoted
string, so `a"b` is an ordinary unquoted element.

### Quoted strings

Quoted strings use `"` and support these escapes:

| Escape | Meaning |
|---|---|
| `\"` | Double quote |
| `\\` | Backslash |
| `\n` | Line feed |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | Null character |
| `\u{1F680}` | Unicode scalar value (1–6 hex digits) |

Any other escape is an error. Nothing may follow the closing quote except
spaces, or the `,` / `]` delimiters inside inline arrays and tables.

## 6. Numbers

Integers use signed decimal notation without leading zeros, except for zero
itself, and must fit in a signed 64-bit integer. A `+` sign is not numeric.

```axs
age: 22
offset: -5
zero: 0
```

Floats contain a fractional part, an exponent, or both:

```axs
height: 1.76
temperature: -4.5
tiny: 1.5e-7
huge: 6.02E23
whole: 1e3
```

A fraction needs digits on both sides of `.`. An exponent is `e` or `E`, an
optional sign, and one or more digits. Results must be finite; overflow is an
error, underflow rounds to signed zero. `NaN`, `Infinity`, `00123`, `.5`, and
`+1` are unquoted strings.

The canonical serializer writes the shortest digits that round-trip. As in
JavaScript, it uses exponent notation (`1.5e-7`, `1e21`) only when the magnitude
is below `1e-6` or at least `1e21`, and plain decimals otherwise. It preserves
`-0.0`. A decimal float with an integral value keeps a fractional part (`1.0`),
so it stays a float.

## 7. Booleans and null

Only lowercase `true`, `false`, and `null` are literals. Quote them to get
strings. Other words such as `yes`, `no`, `on`, `off`, `True`, and `NULL` are
ordinary unquoted strings.

## 8. Text blocks

A value that is exactly `|` starts a text block. The block's content is every
following line indented at least one level deeper than the line holding `|`,
up to the first non-blank line that is not.

```axs
body: |
  First line
    indented line
  # not a comment, just text

  last line
status: open
```

`body` is `"First line\n  indented line\n# not a comment, just text\n\nlast line"`.

Rules:

- Exactly the block's indentation is removed from each content line; deeper
  indentation is kept.
- Content lines may contain any character, including tabs, `#`, `:`, and `"`.
  No escapes are processed.
- Blank lines inside the block become empty lines. Blank lines at the end of the
  block are not part of the content.
- Lines are joined with LF. There is no final line break. CRLF in the source
  is read as LF.
- A block must contain at least one non-blank line.

Strings with leading or trailing line breaks, CR characters, or other control
characters cannot be text blocks and use quoted form. The canonical serializer
uses a text block for any other string containing LF.

## 9. Arrays

### Inline arrays

An array of primitive values may be written on one line:

```axs
tags: [rust, typescript, go]
ids: [1, 2, 3]
mixed: [1, two, 3.0, true, null, "a, b"]
empty: []
```

Elements are separated by `,` and trimmed. Elements are primitives only (no
nested arrays, objects, or text blocks). An unquoted element cannot contain `,`,
`[`, or `]`. Trailing commas and empty elements are errors.

### Block arrays

Any array may be written as a block of `-` items one level deeper:

```axs
skills:
  - Rust
  - TypeScript
matrix:
  -
    - 1
    - 2
  - [3, 4]
```

A bare `-` opens a nested block (an array or an object). `- [a, b]` is a
nested inline array, and `- []` / `- {}` are empty collections.

### Objects inside block arrays

An item whose content starts with `key: ` (or `key:` at line end) is an object.
Its first field sits on the dash line; further fields align two spaces deeper
than the dash:

```axs
users:
  - name: Oleh
    age: 22
    skills: [Rust, TypeScript]
  - name: Alex
    profile:
      city: Lviv
```

When the first field opens a block (a nested object, block array, table, or
text block), the block content is four spaces deeper than the dash, because the
item opens two levels: the implicit object and its named child.

```axs
categories:
  - UI:
      - MUI
      - Tailwind
    name: components
```

After a dash, content is an object field only when its prefix is a valid key
(or quoted key) followed by `:` and then a space or the line end. Otherwise it
is a scalar: `- http://host` and `- 10:30` are strings. A string item that
looks like a field (`- note: x`, `- UI:`) must be quoted.

### Lengths

Any non-empty array may declare its length right after the key:

```axs
tags[3]: [rust, typescript, go]
skills[2]:
  - Rust
  - TypeScript
```

The length is a decimal integer without leading zeros. A length is optional;
when present it must equal the number of elements, otherwise the document is
invalid. Lengths let a reader detect truncated output. The canonical serializer
always writes them. Empty arrays are written as `[]` without a length.

## 10. Tables

An array of objects that all have the same keys and only primitive values may
be written as a table. The header lists the field names in braces; each `-` row
lists the values in the same order:

```axs
users[2]{id, name, email, active}:
  - 1, Oleh, oleh@example.com, true
  - 2, Alex, null, false
```

is equivalent to:

```axs
users[2]:
  - id: 1
    name: Oleh
    email: oleh@example.com
    active: true
  - id: 2
    name: Alex
    email: null
    active: false
```

Rules:

- Field names are keys (§4), separated by `,`, with no duplicates and at least
  one field. The length (`[N]`) is optional and comes before the braces.
- Every row has exactly as many cells as there are fields.
- Cells follow the inline array element rules (§9): primitives only, quote
  strings containing `,` and strings that need quotes anywhere.
- A table has at least one row. Rows are one level deeper than the header, or,
  for a header on a dash line (`- users{a, b}:`), two levels deeper than the dash.
- Each row becomes an object with keys in header order.

The canonical serializer writes an array as a table when it has at least one
element, every element is an object with the same keys in the same order, at
least one key, and no value that is an array, an object, or a string that would
need a text block. Otherwise it falls back to inline arrays (all primitives) or
block arrays.

## 11. Objects

A key followed by `:` at line end opens a nested object or block array one level
deeper. Which one is decided by the first content line: `- ` items make an
array, fields make an object. Mixing them is an error.

```axs
user:
  name: Oleh
  address:
    city: Kyiv
settings: {}
```

A block header must have non-empty indented content. Empty objects and arrays
are written as `{}` and `[]`. A length (`key[N]:`) or field list (`key{a}:`)
makes the block an array; a block of fields after such a header is an error.

Duplicate keys in one object are an error, including keys that differ only in
quoting (`a` and `"a"`).

## 12. Indentation, blank lines, and comments

- One level is exactly two ASCII spaces. Root lines have no indentation.
- Indentation may increase by one level at a time, except for blocks opened on
  a dash line (two levels, §9). It may decrease to any open level.
- Blank lines do not affect structure.
- A line whose first non-space character is `#` is a comment, except inside a
  text block. Comments do not affect structure.
- There are no inline comments: `#` after a value is part of the value.
- Supported nesting depth is at least 128 levels; implementations may reject
  deeper documents with a resource-limit error.

## 13. Serialization

A compliant serializer preserves values, types, and key order:

`parse(stringify(value)) == value`, including key order, integer vs float, and
float bits.

The canonical form chosen by the reference serializer:

- `key: value` with one space after `:` and none before it;
- unquoted keys and strings whenever the rules allow;
- arrays: table, else inline array, else block array (§10);
- lengths on every non-empty array;
- text blocks for strings containing LF when allowed (§8);
- no comments, no blank lines.

Serialization is not required to preserve comments, whitespace, or the
author's choice between equivalent forms.

## 14. Errors

Invalid documents must produce an error, never a partial result. Implementations
should report line and column. Error categories:

- Invalid key, duplicate key
- Invalid indentation, mixed array/object block, empty block
- Invalid escape or unterminated string
- Malformed inline array, table header, or table row (wrong cell count) — `AXS011`
- Array length mismatch — `AXS010`
- Invalid or empty text block
- Unsupported numeric value (overflow, non-finite)
- Resource limit exceeded

Repairing likely model mistakes (wrong indentation width, `=` instead of `:`,
truncated output, unquoted commas) belongs to a separate repair mode that reports
every change it makes. Repair mode is specified separately and is not part of
this grammar.

## 15. Security

AXS is data, not a programming language. A parser must not execute code,
resolve references, or include external content. Implementations should bound
input size, token count, and depth. The Rust implementation's current limits
are listed in [PARSER_STATUS.md](../PARSER_STATUS.md).

## 16. JSON mapping

| JSON | AXS |
|---|---|
| object | object, same key order |
| array | array |
| string | string |
| number without `.`/exponent, fits `i64` | integer |
| other finite number | float |
| `true`/`false`/`null` | same |

An integer outside the `i64` range cannot be represented and is an error.
Converting AXS to JSON writes integers without a fractional part and floats
with one (or with an exponent), so the integer/float distinction survives a
round trip through JSON text.

## 17. Formal structural grammar

This indentation-aware EBNF is written over logical lines. `NL` ends a content
line; blank and comment lines produce nothing. `INDENT`/`DEDENT` change one
two-space level. `SCALAR` is a primitive or `[]`/`{}`; `INLINE` is an inline
array; `TEXT` is a text block's content lines. Semantic rules from the sections
above (duplicate keys, lengths, cell counts, depth) apply on top of it.

```ebnf
document     = { field } , EOF ;
field        = key , len? , ":" , SP , ( SCALAR | INLINE ) , NL
             | key , ":" , SP , "|" , NL , INDENT , TEXT , DEDENT
             | key , len? , ":" , NL , INDENT , block , DEDENT
             | key , len? , cols , ":" , NL , INDENT , rows , DEDENT ;
block        = field , { field } | item , { item } ;
item         = "-" , SP , ( SCALAR | INLINE ) , NL
             | "-" , SP , "|" , NL , INDENT , TEXT , DEDENT
             | "-" , NL , INDENT , block , DEDENT
             | "-" , SP , inline_field , [ INDENT , field , { field } , DEDENT ] ;
inline_field = key , len? , ":" , SP , ( SCALAR | INLINE ) , NL
             | key , ":" , SP , "|" , NL , INDENT , INDENT , TEXT , DEDENT , DEDENT
             | key , len? , ":" , NL , INDENT , INDENT , block , DEDENT , DEDENT
             | key , len? , cols , ":" , NL , INDENT , INDENT , rows , DEDENT , DEDENT ;
rows         = row , { row } ;
row          = "-" , SP , cell , { "," , cell } , NL ;
cols         = "{" , key , { "," , key } , "}" ;
len          = "[" , DIGITS , "]" ;
key          = bare_key | QUOTED ;
bare_key     = ( ASCII_LETTER | "_" ) , { ASCII_LETTER | ASCII_DIGIT | "_" | "-" | "." } ;
```

In the `inline_field` forms, the trailing `[ INDENT , field ... ]` of `item`
holds the continuation fields at the implicit object level, one level deeper
than the dash; the inner `INDENT , INDENT` pairs open the implicit object and
the named child. Optional spaces are allowed around `,`, inside `[ ]` and `{ }`,
and before `:`; the canonical form uses `, ` and no space before `:`.

## 18. Compatibility

This specification is experimental and may change before 1.0. Grammar v3 is not
compatible with Grammar v2, which is preserved in [AXS-v2.md](AXS-v2.md) for
reference. Conformance fixtures live in `tests/conformance/` and are normative
examples of this specification.
