# AXS 0.2.0 prerelease checklist

Status as of 2026-10-10 on branch `feat/token-parser`. Proposed release:
**0.2.0** (pre-1.0 development release). The manifest still says `0.1.0`;
nothing has been published, pushed, or tagged. AXS 1.0 is **not** ready.

## Completed verification

All results below come from local runs on aarch64 macOS (Darwin 25.4), rustc
1.99.0, Node 24.15.0, npm 11.14.1.

- [x] `scripts/check.sh` passes end to end.
- [x] `cargo fmt --check`
- [x] `cargo clippy --locked --all-targets -- -D warnings`
- [x] `cargo test --locked`: 189 integration tests + 2 binary unit tests pass
- [x] `cargo test --locked --doc`: 1 README doctest passes
- [x] `cargo +1.85.0 test --locked`: same 192 tests pass on the declared MSRV
- [x] `cargo package --locked --allow-dirty`: 43 files, archive builds and verifies
- [x] `cargo clippy --locked --all-targets --target x86_64-unknown-linux-gnu -- -D warnings`
- [x] `cargo clippy --locked --all-targets --target x86_64-pc-windows-gnu -- -D warnings`
- [x] Extension: `npm ci` from the lockfile, `tsc` compile with no errors against
      `@types/vscode` 1.90.0, and `out/extension.js` (the `main` entrypoint) exists
- [x] `node_modules/`, `out/`, `target/`, and `*.vsix` are Git-ignored; none are tracked
- [x] Crate archive excludes `editors/`, `target/`, and `node_modules`
- [x] `repository` points to the configured remote, which responds with HTTP 200
- [x] crates.io API reported `axs` as nonexistent (HTTP 404) on 2026-10-10

## Outstanding release blockers

1. **Choose and set the version.** Bump `Cargo.toml` (and run `cargo update -p axs`
   to refresh `Cargo.lock`) to `0.2.0`, or a prerelease such as `0.2.0-alpha.1`.
   Bump `editors/vscode/package.json` separately if the extension is shipped.
   Move the CHANGELOG `Unreleased` section under the chosen version and date.
2. **Commit and review** this preparation work. Nothing has been committed.
3. **No native Linux or Windows test run.** Hosted CI is blocked by billing.
   Run the suite natively on both platforms before claiming support (see below).
4. **Decide on dependency feature leakage.** `clap`, `tempfile`, and
   `serde_json` with `arbitrary_precision` + `raw_value` are unconditional
   dependencies because the CLI shares the package with the library. Library
   users compile them too, and Cargo feature unification turns on
   `serde_json/arbitrary_precision` in their whole dependency graph, which
   changes how serde_json numbers behave in unrelated crates. Before publishing
   the library, consider moving the CLI behind an optional `cli` feature, which
   changes `cargo install` instructions. If you publish without that change,
   document the side effect.
5. **Claim the crate name.** Availability was observed, not reserved. `cargo
   publish` is the only way to secure it.

Not blockers for 0.2.0, but required before 1.0: approval of the Serde/numeric,
CLI/JSON, error-code, and public-module compatibility contracts listed in
RELEASE_AUDIT.md.

## TypeScript SDK (`packages/axs`)

Not part of the 0.2.0 crate release unless you decide otherwise. Verified locally
on 2026-10-10 (Node 24.15.0, rustc 1.99.0, wasm-bindgen 0.2.129):

- [x] `npm run build`: wasm32 build, wasm-bindgen glue for Node and web, and
      TypeScript output, producing one 165,669-byte `.wasm`
- [x] `npm run typecheck`, and 28/28 Node.js runtime tests (`npm test`)
- [x] `npm run test:package`: the 15-file tarball installs with no runtime
      dependencies, and ESM, CommonJS, and bundler-mode consumers compile with
      `skipLibCheck: false` and run, along with the examples
- [x] Browser build: `examples/browser` checked by hand in Chromium 152 (the
      Claude desktop browser pane) through a local HTTP server
- [x] `crates/axs-wasm`: 6 native codec tests, and Clippy for the host and wasm32

Outstanding before an npm release:

1. Confirm or register an npm scope; `@axs-lang/core` is provisional, and the
   unscoped name `axs` is already taken on npm by an unrelated package.
2. Remove `"private": true` only when you are ready to publish.
3. Test Node.js 22 (the declared minimum) and any frameworks you intend to
   claim (NestJS, Next.js, Vite/webpack, Deno, Bun); none were run.
4. Add an automated browser test if browser support is advertised.

## Platform support status

| Platform | Build | Tests run | Status |
|---|---|---|---|
| macOS aarch64 | native | 192/192 (stable and 1.85.0) | Verified |
| macOS x86_64 | not built | none | Unverified |
| Linux x86_64 | Clippy type-check only | none | **Unverified** |
| Windows x86_64 (gnu) | Clippy type-check only | none | **Unverified** |
| Windows x86_64 (msvc) | not built | none | **Unverified** |
| VS Code extension | compiled (tsc) | not run in an Extension Host | Manually unverified |

Cross-target Clippy proves only that the code type-checks for those targets,
including the `cfg(windows)` deny-delete test and the Linux-only non-UTF-8
filename test. It does not link or execute them. Tools available locally: no
mingw or Linux cross linker. Docker Desktop is installed but its daemon was not
running, and pulling a Rust image was not done without approval. Possible free
options:

- Linux: start Docker Desktop, then
  `docker run --rm -v "$PWD":/src -w /src rust:1.85 cargo test --locked`
  (this checks Linux aarch64, not x86_64, on Apple silicon).
- Windows: run `cargo test --locked` on any Windows machine or VM with the MSVC
  or GNU toolchain. The deny-delete replacement test only runs there.

## Known limitations

- Formatting is canonical and drops comments, whitespace, original quoting, and
  key order (also applies to Format Document in VS Code).
- CLI takes one regular-file path; there is no stdin, batch, or recursive mode.
- Size and token limits are not memory or CPU quotas; see README.
- Numeric buffered Serde deserialization (`deserialize_any`, numeric untagged or
  flattened enums) is rejected by design. See SERDE_STATUS.md.
- Conversion errors (JSON ↔ AXS, Serde) have no source spans.
- In-place formatting does not preserve ownership, ACLs, xattrs, or hard links,
  and gives no crash durability guarantee.
- Extension: diagnostics run only for documents saved to disk (`file:` scheme),
  not for untitled buffers. Only the first error is reported. Diagnostic columns
  are Unicode-scalar based and converted to UTF-16, so the marker is not
  display-width aware. The extension needs an `axs` executable; it does not
  bundle one.
- Extension: `publisher` is `axs-dev`, a placeholder whose Marketplace ownership
  has not been verified. The extension directory has no LICENSE file, so `vsce`
  will warn.

## Manual publication steps

None of these were performed. Run them yourself after resolving the blockers.

1. Resolve blockers 1, 2, and 4 above; commit.
2. `scripts/check.sh` must pass on the release commit, and also
   `cargo +1.85.0 test --locked`.
3. Native Linux and Windows runs: `cargo test --locked` on each. Record the results
   in RELEASE_AUDIT.md.
4. Inspect the crate archive with `cargo package --locked --list`, then do a dry
   run: `cargo publish --locked --dry-run`.
5. `cargo login` (with your own crates.io token), then `cargo publish --locked`.
6. `git tag -a v0.2.0 -m "AXS 0.2.0"`, then `git push origin <branch> v0.2.0`.
7. Optionally, create the GitHub release from the tag with the CHANGELOG section.
8. Extension, if shipping: create or confirm the Marketplace publisher and update
   `publisher`, add a LICENSE to `editors/vscode`, then
   `npx @vscode/vsce ls` to review files (expect `package.json`,
   `language-configuration.json`, `syntaxes/`, `out/extension.js`), then
   `npx @vscode/vsce package` and test-install the VSIX before running
   `vsce publish`.
