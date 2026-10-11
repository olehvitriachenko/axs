# AXS prerelease checklist

Updated 2026-10-11 on `feat/typescript-sdk`. Rust crate: `0.1.0`; npm SDK
checkout: `@axs-lang/core` `0.2.0`; VS Code extension: `0.1.0`.
These are independent package versions. This follow-up commits existing changes
and prepares package links; it does not publish, push, tag, or bump versions.
AXS 1.0 is not released.

## Local verification

`scripts/check.sh` checks the following on the current macOS arm64 checkout:

- Rust formatting, workspace Clippy with warnings denied, workspace tests,
  README doctest, and verified crate packaging.
- WebAssembly target Clippy, SDK build and strict typecheck, Node runtime tests,
  CLI parity tests, and installation of the packed npm tarball into consumers.
- Extension strict typecheck, Smart Enter/Unicode/grammar/WASM tests, VSIX
  packaging, and the exact package content allow-list.

2026-10-11 result: `./scripts/check.sh` passed in 234 seconds, with no skipped
SDK or extension tests. Rust: 189 integration + 2 binary unit + 6 WASM unit +
1 doctest = 198 unique tests (the doctest also passed its separate run).
SDK: 32/32 tests; packed tarball: 16 files, installed consumers pass.
Extension: 23/23 tests; VSIX: exactly 10 files. Clippy reported no warnings.
Node was 24.15.0 and npm was 11.12.1.

Exact primary commands executed by the script:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked --doc
cargo package --locked --allow-dirty -p axs
cargo clippy --locked -p axs-wasm --target wasm32-unknown-unknown -- -D warnings
npm --prefix packages/axs ci --no-audit --no-fund
npm --prefix packages/axs run build
npm --prefix packages/axs run typecheck
npm --prefix packages/axs test
npm --prefix packages/axs run test:package
npm --prefix editors/vscode ci --no-audit --no-fund
npm --prefix editors/vscode run typecheck
npm --prefix editors/vscode test
npm --prefix editors/vscode run package
```

The package-install test builds and packs locally and installs offline into
fresh consumer projects. Packaging commands do not publish either package.
Generated `target`, `node_modules`, `dist`, `out-test`, tarballs, and VSIX files
are ignored and are not committed.

Previous 2026-10-10 checks also covered Rust 1.85, Linux and Windows cross-target
Clippy, and manual browser usage. These checks were not repeated in this
follow-up. Cross-target Clippy is not a native platform test.
The installed-VSIX smoke test previously reported 14/14 steps on macOS arm64
with VS Code 1.140.0; see editors/vscode/RELEASE_READINESS.md. The current run
does not invoke `--vscode-smoke` or open VS Code.

## npm and repository linkage

- Package name: `@axs-lang/core`. `repository.directory` is `packages/axs`;
  repository, homepage, and issue links use the configured Git remote.
- `npm view @axs-lang/core version repository --json` on 2026-10-11 reports
  version `0.1.0` and the same repository/directory. The local 0.2.0 adds
  `check` and `format`; consumers need its local tarball until publication.
- `publishConfig.access` is `public`; the package has no `private` flag.
  Publication is a separate manual action requiring authorized npm access.
- The extension uses `file:../../packages/axs` as a development dependency
  and bundles its JavaScript and WASM. It has no runtime npm dependency or
  requirement for an installed Rust toolchain/CLI. After SDK 0.2.0 is published
  and tested, its development dependency can switch to the registry version.
- npm tarball tests check CommonJS, ESM, and bundler-mode TypeScript consumers.
  The crate archive excludes SDK and editor build artifacts.

## Remaining release work

1. Review the logical commits and choose release versions for each component.
   Move the relevant CHANGELOG entries under the chosen release date before
   publication. No Rust version change is made here.
2. Run native Linux and Windows tests. Previous hosted CI billing problems were
   recorded in the earlier audit; CI availability was not rechecked here.
3. Decide whether to isolate CLI dependencies (`clap`, `tempfile`, and
   `serde_json` with `arbitrary_precision`/`raw_value`) behind a feature or a
   separate package. They currently affect library compilation and Cargo
   feature unification; document this if shipping unchanged.
4. Confirm registry permissions/name availability and perform publication only
   with explicit authorization. The local package link does not establish npm
   account ownership or reserve a crates.io name.
5. Test Node.js 22 (the SDK minimum), automate browser tests, and test the
   oldest declared VS Code version (1.90). Framework-specific support and
   remote extension hosts have not been verified.
6. Approve the compatibility contracts in RELEASE_AUDIT.md before stable 1.0.

## Known limitations

- Canonical formatting drops comments, original whitespace/quoting, and key
  order. The editor preserves the document's LF/CRLF line ending convention.
- The CLI accepts one regular-file path, without stdin/batch/recursive modes.
- Byte/token/depth limits are not memory or CPU quotas.
- Numeric buffered Serde deserialization is rejected by design; see
  SERDE_STATUS.md. Conversion errors have no source spans.
- In-place CLI formatting does not preserve ownership, ACLs, xattrs, or hard
  links and does not guarantee crash durability.
- Editor diagnostics report the first error and convert Unicode-scalar spans
  to UTF-16, not display-width positions. Untitled and virtual documents use
  the bundled WASM core. Browser extension hosts are unsupported.
- Marketplace package ID is `reaxios.axs-language`; publication and publisher
  access remain separate from local packaging.
