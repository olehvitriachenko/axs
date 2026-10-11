# AXS Language

Language support for [AXS](https://github.com/olehvitriachenko/axs), a
human-friendly, indentation-based configuration and serialization format.

- **Syntax highlighting** for block headers, keys, list items, strings, escapes,
  numbers, booleans, `null`, empty collections, and comments
- **Live diagnostics** while you type, with stable error codes (`AXS001`–`AXS009`)
  and exact locations, including lines with emoji and other non-ASCII text
- **Format Document** that produces the same canonical output as `axs fmt`
- **Smart Enter** that continues lists and indents blocks the way AXS expects
- **Works out of the box:** the AXS Rust core ships inside the extension as
  WebAssembly, so you don't need Rust, Cargo, or the `axs` CLI

## Example

```axs
# Application settings
server:
  host = localhost
  port = 3000
  enabled = true
skills:
  - Rust
  - TypeScript
categories:
  - UI:
      - MUI
      - Tailwind
    weight = 2
```

A duplicate key such as a second `port = 3001` under `server:` is underlined
immediately:

```text
AXS005  Duplicate key 'port'                                    settings.axs [Ln 5, Col 3]
```

## Formatting

**Format Document** (<kbd>Shift</kbd>+<kbd>Alt</kbd>+<kbd>F</kbd>, or
<kbd>Shift</kbd>+<kbd>Option</kbd>+<kbd>F</kbd> on macOS) rewrites the file in
canonical AXS. It sorts keys, uses two-space indentation and colon block headers,
and applies minimal quoting.

> **Formatting removes comments and changes key order.** It keeps every value
> and type: `1.0` stays a float and large integers keep every digit. The output
> is identical to `axs fmt`, except that the document keeps its own line endings
> (LF or CRLF).

Invalid documents are not formatted. The error stays visible as a diagnostic.

## Smart Enter

Enter continues the structure the current line started:

| Line before Enter | New line |
|---|---|
| `skills:` | indented by 2 with `- ` for a new block |
| `skills:` above existing `- ` items | `- ` (matches the existing items) |
| `- Rust` | `- ` at the same indent |
| `- ` (empty item) | removes the dash and moves out one level, which ends the list |
| `- UI:` | indented 4 below the dash and `- ` for a new block |
| `-` (bare dash) | indented by 2 with `- ` for a new nested collection |
| `- name = Oleh` | indented by 2 for the next field of that object |
| `key = value` | same indent |

For a new `key:` block, Enter inserts two spaces and `- ` by default:

```axs
data:
  -
```

Existing object children keep indentation without a dash; existing lists keep
list markers. The same rule works after `- UI:` (four spaces below the dash)
and a bare `-` (two spaces below it). To prefer objects in new blocks, set
`"axs.smartEnter.defaultCollection": "object"` in VS Code settings; the default
is `"array"`. Existing children override the preference. An empty `- ` followed
by Enter exits the list. A blank list marker is a typing placeholder: fill it
with a value or child block before validation succeeds. Comments, multiple cursors,
selections, and Enter in the middle of a line keep VS Code's normal behavior.
For `.axs` files the extension defaults to two-space indentation with spaces,
since AXS doesn't allow tabs.

## Details

- Diagnostics update 400 ms after you stop typing and immediately when a file
  opens. They are removed when the file closes or switches to another language.
- The core reports the first error in a document.
- The AXS limits apply: 8 MiB per document, 262,144 tokens, and 128 nesting
  levels. Validation runs synchronously; latency depends on document size and
  the machine.
- The extension supports untrusted and virtual workspaces, since it never runs
  external programs; it reads its bundled WASM file.
- Requires VS Code 1.90 or newer.

## License

Apache-2.0
