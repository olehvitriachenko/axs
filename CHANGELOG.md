# Changelog

## Unreleased

No published release history was verified. The manifest is still 0.1.0; earlier
development milestone labels are not asserted to be crates.io releases.

### Changed: SDK and VS Code extension follow Grammar v3

- `@axs-lang/core` (checkout 0.2.0) adds `query(document, text)` over the new
  `query`/`querySource` WebAssembly exports, the `LengthMismatch`,
  `InvalidCollection`, and `InvalidQuery` error kinds (AXS010-AXS012), and keeps
  key order in `parse`/`stringify`; keys that are not bare keys are written
  quoted instead of rejected.
- The VS Code extension highlights tables, lengths, inline arrays/objects, and
  text blocks, and Smart Enter and auto-indent handle them. Verified in a real
  VS Code with the installed-VSIX smoke test (15/15 steps).

### Added: queries

- `axs::query` and `axs query [--from-json] [--json] QUERY FILE` select,
  filter, and project data: `users[age > 20]{name}`, `users[-1]`,
  `issues[state == open and labels.name ~ bug].number` (spec/QUERY.md).
  `ErrorKind` adds `InvalidQuery` (AXS012); errors name available fields.

### Breaking grammar change: Grammar v3

- Grammar v3 replaces v2 without a migration mode (spec/AXS.md; v2 kept as
  spec/AXS-v2.md). Fields are `key: value`; `key = value` is rejected.
- New: inline arrays `[a, b]`, `|` text blocks, tables `key{a, b}:` with
  `- x, y` rows, optional checked lengths `key[N]`, quoted keys, `.` in bare
  keys, and exponent floats.
- Key order is preserved: `AxsObject` (`IndexMap`) replaces `HashMap`, and the
  serializer no longer sorts. `axs fmt --sort-keys` and `value::sort_keys`
  sort explicitly. JSON conversion keeps key order.
- The serializer picks tables, inline arrays, or blocks, writes lengths, and
  uses text blocks for multiline strings. Non-bare keys are quoted instead of
  rejected.
- `TokenKind` drops `Equals` and adds `Length`, `Columns`, `Items`, `Text`.
  `ErrorKind` adds `LengthMismatch` (AXS010) and `InvalidCollection` (AXS011).
- Normative conformance fixtures in `tests/conformance/`.
- Token-driven canonical form: commas without spaces, table rows without a `-`
  prefix (a dashed row is now an error), and inline objects `{a: 1,b: x}` for
  objects of up to four primitive fields. Measured in `bench/tokens/`: 14.6%
  fewer tokens than minified JSON overall (TOON 4.4.0: 14.3%).

### Breaking grammar change: Grammar v2

- Grammar v2 required `key:` for all named nonempty object/array blocks. The
  parser rejects legacy bare keys and mixed old/new block syntax; no legacy
  grammar mode is provided. `key = value`, empty `[]`/`{}` assignments, primitive
  typing, escaping, and resource limits are preserved.
- Support `- key:` as an inline object's first named collection, with child
  content four spaces below the dash and continuation fields two spaces below.
  Serializer output uses colon headers and expanded array objects with bare
  dashes. Ambiguous colon-prefixed array strings are quoted.
- Public `TokenKind` adds `Colon`; exhaustive downstream matches need updating.
  Function signatures and Serde mappings remain unchanged. `parse_legacy` keeps
  its string-error signature but enforces v2 syntax. See spec/AXS.md and README
  for migration examples. No version bump, tag, or publication was performed.

### Added

- Smart Enter inserts `- ` after new named blocks and nested collection
  openers, while mirroring existing object/list children. The new
  `axs.smartEnter.defaultCollection` setting allows an object preference.
  Invalid odd indentation falls back to normal editor behavior.

- `@axs-lang/core` 0.2.0 (not published; 0.1.0 on npm is unchanged): `check`
  and `format`, backed by new `check`/`format` WebAssembly bindings. `format`
  serializes the Rust tree directly, so it matches `axs fmt` byte-for-byte,
  which `stringify(parse(text))` cannot do because `1.0` would become `1`.
  It is tested against the CLI.
- VS Code extension prepared for its first Marketplace release (0.1.0, not
  published). Validation and formatting now run the bundled WebAssembly core,
  so Rust and the `axs` CLI are no longer needed. The extension is bundled with
  esbuild, packaged with an allow-listed VSIX, and covered by unit, grammar,
  and WebAssembly runtime tests plus an installed-VSIX smoke test in VS Code.
  See editors/vscode/RELEASE_READINESS.md.
- TypeScript/JavaScript SDK (`packages/axs`, npm name
  `@axs-lang/core`): `parse`, `stringify`, `init`,
  and a structured `AxsError`, backed by the Rust core compiled to WebAssembly.
  The Node.js build is CommonJS with synchronous loading and works through both
  `require` and `import`. A browser ESM build needs `await init()`. Integers
  outside ±(2^53 − 1) map to `bigint`, unsafe integer numbers and out-of-range
  bigints are rejected, and `-0` and float bits are preserved. Includes Node.js
  and NestJS-style examples, a browser example, runtime tests, and a tarball
  install test.
- `crates/axs-wasm` (workspace member, `publish = false`): wasm-bindgen bindings
  that exchange a versioned binary payload with the SDK, so no JSON
  intermediate is used and Rust never calls back into user JavaScript.
- `axs::serializer::stringify_typed` is now public: it is `stringify` with a
  structured `AxsError` instead of a string. This is additive, and `stringify`
  is unchanged.
- VS Code extension (`editors/vscode`, unpublished): TextMate syntax
  highlighting, Smart Enter list continuation, live CLI diagnostics with a
  400 ms debounce and stale-result protection, and Format Document via
  `axs fmt` on a temporary copy.
- `scripts/check.sh`: local verification (fmt, Clippy, tests, doctests, crate
  packaging, extension `npm ci` and TypeScript build) that does not depend on
  hosted CI.
- Release metadata: Cargo `repository` URL
  (https://github.com/olehvitriachenko/axs), `command-line-utilities` category,
  and RELEASE_CHECKLIST.md for the proposed 0.2.0 prerelease.
- Replace the demonstration executable with a Clap-based `axs` CLI: `check`,
  `fmt`, `fmt --check`, `to-json`, `from-json`, help, and version support.
  Library signatures, Grammar v2, primitives, and Serde contracts are unchanged.
- Safe in-place formatting with same-directory temporary files, permission
  preservation, syncing, concurrent-edit revalidation, failure cleanup, and
  symlink refusal. Document formatting loss and OS/filesystem constraints.
- Private serde_json/AxsValue conversion with duplicate-key detection, checked
  i64 numbers, conservative f64 decimal round-trip checks, negative-zero
  preservation, and input/output/depth/node budgets. No arbitrary-precision AXS
  primitive or alternate AXS parser was introduced.
- Executable integration coverage for CLI commands, exit codes and diagnostics,
  JSON/data preservation, file safety, resource limits, and stdout failures;
  binary unit tests for concurrent edits and temporary-file cleanup.
- Anchor crate-package include patterns at the root so unrelated editor
  node_modules README/license files do not enter the Rust archive.
- Apache-2.0 licensing: complete official root LICENSE, Cargo license metadata,
  and README license section.
- Serde `from_str` / `to_string` integration through the existing AxsValue tree,
  checked numeric conversions, externally tagged enum mappings, and conversion
  guards. Existing object-root grammar and public tree APIs are preserved.
- Regression/property coverage for parser/serializer consistency, Serde mappings,
  malformed inputs, numeric limits, deterministic output, recursion, and budgets.
- Verified Rust 1.85.0 MSRV, explicit package metadata/content list, and pinned
  GitHub Actions CI for stable/MSRV on Linux, macOS, and Windows.
- Parser/Serde status documents, a compiled README example, and release audit.

### Fixed

- VS Code Smart Enter edits were silently dropped in the real editor:
  `registerTextEditorCommand` ran the handler inside its own edit transaction.
  Smart Enter also inserted `- ` after every `key:` header, even in objects.
  It now mirrors the block's existing children, and an empty `- ` item ends
  the list. Highlighting no longer colors `true` or numbers inside unquoted
  strings.
- VS Code extension: the CLI was only found at a repository-relative path,
  which does not exist in an installed extension and lacked `.exe` on Windows;
  it now falls back to `axs` on `PATH`, and `axs.cliPath` is a declared setting.
  A missing CLI is reported once instead of only in the developer console.
  Smart Enter defers to the built-in Enter with multiple cursors. Type
  definitions are pinned to the declared VS Code 1.90 / Node 20 baseline, and
  `.vscodeignore` keeps sources, maps, and `node_modules` out of a VSIX.
- Excessive nesting, numeric overflow fallback, lone-quote decoding panic, and
  ambiguous array-string serialization during the preceding core milestones.
- Release-audit diagnostics: long-line/message amplification and raw terminal
  controls in `render`/`Display`. Source excerpts and diagnostic fields are now
  bounded and controls escaped; structured error fields and codes are unchanged.
- Three older-Clippy warnings on the verified MSRV, without suppressing lints
  or removing tests.

### Compatibility notes

- Legacy parsing delegates to the token parser; public string-error signatures
  remain available. Human error strings include stable AXS codes.
- Numeric buffered Serde deserialization is intentionally unsupported; nullable
  Some values are rejected to avoid information loss. See SERDE_STATUS.md.
- Crate version was not bumped, no release tag was created, and nothing was
  published. Apache-2.0 licensing is resolved, and the repository URL is set
  from the configured Git remote.

## Proposed next versions

Recommend **0.2.0** for the next development release once repository and registry
identity are resolved. This recommendation uses the actual 0.1.0 manifest,
not informal milestone numbers. After native CI passes and public contracts are
approved, consider **1.0.0-rc.1**, followed by 1.0.0 only after release-candidate
validation. No version change in this file declares a release.
