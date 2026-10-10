# AXS — Data Serialization Format

**Specification Version:** 0.6.0-draft  
**Status:** Experimental  
**Reference Implementation:** Rust

## 1. Overview

AXS is a human-readable, indentation-based data serialization format designed to minimize typing while preserving predictable data types.

### Design principles

- Minimal syntax and boilerplate.
- Human-readable, human-writable.
- No mandatory quotation marks for ordinary strings.
- No commas or braces required for non-empty nested structures.
- Deterministic parsing.
- Safe serialization without executing code.
- Language-independent data model.

## 2. Encoding

AXS documents use UTF-8.

A document consists of key-value entries and nested collections.

The root of an AXS document is an object.

Example:

```text
name = Oleh
age = 22
active = true

address
  city = Kyiv
  country = Ukraine
```

## 3. Data Types

| Type | Example | Description |
|---|---|---|
| String | `name = Oleh` | UTF-8 text |
| Integer | `age = 22` | Signed 64-bit integer |
| Float | `height = 1.76` | Finite floating-point number |
| Boolean | `active = true` | True or false |
| Null | `value = null` | Absence of value |
| Array | `items = []` | Ordered collection |
| Object | `config = {}` | String-keyed collection |

Arrays and objects may contain any supported AXS values.

## 4. Key-Value Syntax

A scalar entry uses the following syntax:

```text
key = value
```

Spaces around `=` are optional:

```text
name=Oleh
name = Oleh
```

Keys must begin with an ASCII letter or underscore and may subsequently contain ASCII letters, digits, underscores, and hyphens.

Duplicate keys within the same object are invalid.

## 5. Strings

AXS supports quoted and unquoted strings.

```text
name = Oleh
message = Hello world!
nameAsNumber = "123"
enabledAsText = "true"
```

Quoted strings support the following escape sequences:

| Escape | Meaning |
|---|---|
| `\"` | Double quote |
| `\\` | Backslash |
| `\n` | Line feed |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | Null character |
| `\u{1F680}` | Unicode scalar value |

An empty string is written as `""`.

Values matching reserved literals or numeric syntax must be quoted when a string is intended.

## 6. Numbers

Integers use signed decimal notation without leading zeros, except for zero itself.

```text
age = 22
offset = -5
zero = 0
```

A value such as `00123` is interpreted as a string.

Floating-point values contain a decimal point:

```text
height = 1.76
temperature = -4.5
```

The Rust implementation rejects integer overflow and non-finite floating-point
results with `AXS008`. Decimal floats require digits on both sides of the point
and no leading zeros in the whole part, except for zero itself. An optional
minus sign is supported; a plus sign is not numeric syntax. Conversion to `f64`
uses normal binary rounding, including underflow to signed zero.

Exponent notation, `NaN`, and `Infinity` are unquoted strings. The serializer
emits decimal float notation and preserves negative floating-point zero.

## 7. Booleans and Null

AXS recognizes only lowercase literals:

```text
active = true
disabled = false
value = null
```

These literals may be quoted to produce strings.

## 8. Objects

Non-empty nested objects use indentation:

```text
user
  name = Oleh
  age = 22
  address
    city = Kyiv
```

An empty object is represented by `{}`:

```text
settings = {}
```

## 9. Arrays

Arrays use hyphen-prefixed entries.

```text
skills
  - Rust
  - TypeScript
  - PostgreSQL
```

An empty array is represented by `[]`:

```text
skills = []
```

Array elements may also be objects:

```text
users
  - name = Oleh
    age = 22
  - name = Alex
    age = 25
```

Nested arrays and objects are supported:

```text
matrix
  -
    - 1
    - 2
  -
    - 3
    - 4
```

## 10. Indentation

- One nesting level equals exactly two ASCII spaces.
- Tabs are prohibited.
- Root-level entries have no indentation.
- Unexpected indentation is an error.
- Blank lines do not affect nesting.
- The Rust implementation supports block depths 0 through 128 inclusive.
  Inline array objects count as an extra level, as in their serialized block form.
- Non-ASCII leading whitespace on content lines is invalid indentation.

## 11. Comments

A full-line comment begins with `#`.

```text
# Application settings
debug = true
```

Inline comments are not part of the current specification.

`#` inside an assignment or array value is literal text. Blank and full-line
comment lines do not change indentation; raw tabs remain prohibited everywhere.

## 12. Serialization

A compliant serializer must preserve the semantic value and type of supported data.

The core round-trip requirement is:

`parse(stringify(value)) == value`

Serialization is not required to preserve whitespace, comments, or original key order.

The reference serializer sorts object keys lexicographically.

## 13. Errors

Invalid syntax must produce an error rather than executing code.

Error categories include:

- Invalid key
- Duplicate key
- Invalid indentation
- Invalid escape sequence
- Missing value
- Malformed collection
- Unsupported numeric representation

Implementations should report source line and column positions where possible.

## 14. Security

AXS is a data format, not a programming language.

A parser must not execute arbitrary code contained in a document.

The Rust implementation enforces fixed limits of 128 block levels, 8 MiB per
input/output document, and 262,144 tokens including structural tokens and EOF.
Resource-limit errors use `AXS002` for depth and `AXS009` for size/token count.
See `../PARSER_STATUS.md` for the implemented grammar and known limitations.

## 15. Compatibility

This specification is experimental.

Syntax and semantic rules may change before version 1.0.

Future additions may include schemas, binary encoding, streaming, and language-specific bindings.
