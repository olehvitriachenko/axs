# Changelog

## 0.1.0 (unreleased)

First Marketplace release.

- Updated for AXS Grammar v3: `key: value` fields, array lengths
  (`tags[3]:`), tables (`users{id,name}:` with dashless rows), inline arrays
  and objects, `|` text blocks, quoted and dotted keys, exponent floats, and
  error codes `AXS010`–`AXS012`. Highlighting, auto-indent, and Smart Enter
  follow the new syntax; Format Document keeps key order.

- Smart Enter inserts `- ` after new named blocks and nested collection
  openers, while mirroring existing object/list children. The new
  `axs.smartEnter.defaultCollection` setting allows an object preference.
  Invalid odd indentation falls back to normal editor behavior.

- Syntax highlighting: block headers (`key:`, `- key:`), fields, list items, quoted strings with escapes, numbers, booleans,
  `null`, empty collections, and full-line comments. Literals are highlighted
  only when they form the whole value, so `title: Hello 42 true` is a string.
- Live diagnostics with a 400 ms debounce, AXS error codes, and Unicode-aware
  locations.
- Format Document producing the same canonical output as `axs fmt`, keeping
  the document's line endings.
- Smart Enter for lists, blocks, inline objects, and ending lists.
- The AXS Rust core is bundled as WebAssembly (via `@axs-lang/core`), so no
  Rust toolchain or `axs` CLI is needed. The pre-release `axs.cliPath` setting
  was removed because the extension no longer runs the CLI.
- Two-space indentation with spaces is the default for `.axs` files.
