# AXS Language 0.1.0: Marketplace release readiness

Updated: 2026-10-11. Branch: `feat/typescript-sdk`.
Release changes are recorded in logical SDK, extension, and verification
commits. This follow-up performs local checks and packaging only; it does not
publish, push, tag, or create tokens. Release conditions below still apply.
The installed-VSIX smoke results below are from the prior 2026-10-10 audit;
the 2026-10-11 check ran `scripts/check.sh` without `--vscode-smoke` and passed
in 234 seconds: 198 Rust tests, 32 SDK tests, 23 extension tests, npm tarball
installation, and the exact 10-file VSIX allow-list. Historical timing figures
below describe the earlier audit, not a performance guarantee.

## Smart Enter follow-up (2026-10-11)

New block headers and bare nested openers insert an indented `- ` by default.
Existing object/list children override the preference; set
`axs.smartEnter.defaultCollection` to `object` for indentation-only new blocks.
Odd indentation falls back to normal Enter. Unit/runtime tests: 25/25 pass;
strict typecheck and VSIX packaging pass. Installed-host checks pass 15/15
on VS Code 1.140.0 (macOS arm64), including clean shutdown of the isolated
test instance. The installed-host test covers the
marker/cursor, one-step Undo/Redo, object/list inference, and resource settings.
The original audit below describes the earlier behavior and measurements.

## What changed for the release

- **No Rust or CLI needed.** Validation and formatting call `check()` and
  `format()` from `@axs-lang/core`, which runs the Rust core compiled to
  WebAssembly. The `.wasm` ships inside the VSIX next to the bundle.
- **SDK 0.2.0 (unpublished).** The published 0.1.0 exposes only `parse` and
  `stringify`. `stringify(parse(text))` cannot reproduce `axs fmt`: it turns
  `1.0` into `1` and rejects `100000000000000000000.0`. The shared bindings
  (`crates/axs-wasm`) and the SDK therefore gained `check` and `format`, as
  version 0.2.0. npm 0.1.0 is untouched. The extension bundles the local
  `packages/axs` build (`file:` devDependency), so publishing the extension
  does not require publishing the SDK.
- **Bundling.** esbuild produces `dist/extension.js`, and the build copies
  `dist/axs_wasm_bg.wasm` next to it. The loader reads the `.wasm` from
  `__dirname`, so nothing depends on the repository layout or `node_modules`
  at runtime.
- **Fixed:** Smart Enter edits were silently dropped in the real editor.
  `registerTextEditorCommand` runs its handler inside an edit transaction,
  which discarded the nested `editor.edit` and `default:type` calls. This was
  found by the installed-extension smoke test and fixed by registering a
  plain command.
- **Initial Smart Enter (superseded above):** a `key:` header no longer forces `- `. Enter
  continues the structure the current line started. After a header it adds a
  dash only if the block's existing first child is a list item. An empty
  `- ` item ends the list and outdents.
- **Fixed highlighting:** `true`, `null`, and numbers are colored only when
  they are the whole value, as the AXS primitive parser treats them.
- **Other changes:**
  - Two-space indentation with spaces is the default for `[axs]`.
  - Untrusted and virtual workspaces are supported; nothing external runs.
  - The internal `axs.continueList` command is hidden from the palette.
  - The obsolete `axs.cliPath` setting was removed, as noted in the changelog.

## Tests executed (all on macOS arm64, Node 24.15.0)

| Check | Result |
|---|---|
| `scripts/check.sh --vscode-smoke` (whole repository, clean environment) | Pass, exit 0, 38 s |
| Rust: fmt, Clippy `-D warnings` (workspace and wasm32), 198 tests | Pass |
| SDK: build, strict typecheck, 32 tests incl. byte-for-byte parity with `axs check`/`axs fmt` on 10 documents (CRLF, Unicode, comments, big floats), tarball install test | Pass |
| Extension: strict typecheck (`skipLibCheck: false`) for `src` and tests | Pass |
| Extension unit tests: 23. Smart Enter (11), UTF-16 positions and minimal format edits (4), grammar tokenization with `vscode-textmate` (3), and bundled WebAssembly runtime (5): valid and invalid documents, `.wasm` loaded next to the bundle, a document near 8 MiB (check ≈45 ms, format ≈60 ms) | Pass |
| VSIX packaging and exact content allow-list | Pass, 10 files, 82.2 KB |
| **Installed-extension smoke test in VS Code 1.140.0**: the VSIX is installed into an isolated extensions directory and profile, then driven from inside the extension host | **14/14 pass** |

Smoke-test steps run in real VS Code:

1. The extension is installed from the VSIX and inactive before any `.axs`
   file is opened.
2. Opening `valid.axs` activates it. The only `.wasm` read is
   `<installed extension>/dist/axs_wasm_bg.wasm`.
3. A valid document has no diagnostics.
4. An invalid escape after an emoji reports `AXS007` at the correct UTF-16
   range (`1:8-14`).
5. A duplicate key reports `AXS005` on line 2.
6. Live diagnostics appear after the debounce (measured 414–416 ms) and not
   before 200 ms.
7. With 10 rapid alternating edits, the final text's result always wins,
   whether the final text is valid or invalid.
8. Format Document produces the `axs fmt` output and keeps CRLF, and a second
   run produces no edits.
9. Formatting an invalid document makes no edits.
10. Smart Enter in the editor: object header, list continuation, ending a
    list, mirroring list children, `- UI:` (+4), bare `-`, inline-object
    fields, and assignments.
11. Multiple cursors use the default Enter, with no list items inserted.
12. The Enter keybinding and the `[axs]` tab size and space defaults are
    active.
13. Diagnostics are removed when the document's language changes.
14. Syntax scopes are confirmed through VS Code's own tokenizer.

Not exercised: a physical Enter key press. The command and its keybinding
contribution are verified, but the test cannot synthesize keystrokes.

## VSIX contents (`axs-language-0.1.0.vsix`)

```text
[Content_Types].xml
extension.vsixmanifest
extension/LICENSE.txt                     Apache-2.0
extension/changelog.md
extension/readme.md
extension/package.json
extension/language-configuration.json
extension/syntaxes/axs.tmLanguage.json
extension/dist/extension.js               34 KB, esbuild bundle (extension, Smart Enter, SDK)
extension/dist/axs_wasm_bg.wasm           163 KB, AXS core from SDK 0.2.0
```

No sources, tests, maps, `node_modules`, or temporary files are included.
SHA-256 of the VSIX was 7e24f15b… for this build; it changes with each build.

## Platform status

| Platform | Status |
|---|---|
| macOS arm64, VS Code 1.140.0 | **Verified** (installed VSIX, in-editor tests) |
| Windows, Linux, macOS x64 | **Not run.** There is no native code; the extension is JavaScript plus a platform-independent `.wasm` loaded through Node `fs`. Expected to work, but unverified. |
| VS Code 1.90–1.139 (declared `engines: ^1.90.0`) | **Not run.** Only 1.140.0 is installed. VS Code 1.90 ships Node 20; the bundle targets Node 20. |
| vscode.dev / github.dev (web extension host) | **Not supported.** The extension has no `browser` entry point. |
| Remote (SSH, WSL, containers) | Not tested. It runs as a workspace extension with no external dependencies. |

## Remaining blockers and conditions

1. **Publisher: resolved.** `publisher` is `reaxios` (Reaxios), and its
   Marketplace page responds with HTTP 200. The extension ID is
   `reaxios.axs-language`, which had no listing yet when checked.
2. **Commits: resolved.** Review the SDK, extension, and verification commits
   before publication.
3. **Engine range (condition).** Either test on VS Code 1.90 (for example with
   `@vscode/test-electron`, which downloads that VS Code version) or raise
   `engines.vscode` to the oldest version you actually test.
4. **Windows and Linux (condition).** Run `node scripts/smoke-test.mjs` on
   each, or state macOS-only verification in the release notes.
5. **Presentation (recommended).** Add a 128×128 PNG `icon` and screenshots or
   a GIF. Relative README images resolve against the GitHub repository after
   you push.
6. **SDK source (note).** Building the extension needs the local SDK 0.2.0
   build, which needs Rust. After you approve and publish `@axs-lang/core`
   0.2.0, the devDependency can switch from `file:../../packages/axs` to
   `^0.2.0`.

## Exact publication steps (for you to run)

1. `"publisher"` is already `reaxios` in `editors/vscode/package.json`.
2. From the repository root, rebuild and verify:

   ```sh
   scripts/check.sh --vscode-smoke
   ```

3. Inspect the package:

   ```sh
   cd editors/vscode && npx vsce ls --no-dependencies
   ```

4. Commit, then tag if you want (for example `vscode-v0.1.0`) and push.
5. Create an Azure DevOps personal access token with the **Marketplace
   (Manage)** scope for your publisher's organization. Don't commit or share it.
6. Sign in and publish the exact VSIX you verified:

   ```sh
   npx vsce login reaxios
   npx vsce publish --packagePath axs-language-0.1.0.vsix
   ```

7. Install the extension from the Marketplace in a clean profile. Open a
   `.axs` file and confirm diagnostics and formatting.
