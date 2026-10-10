# AXS release-readiness audit

Date: 2026-10-10. Verdict: **NOT READY for a stable 1.0 release**.

The audited implementation passes the checks below, including an actual MSRV
run. No unresolved critical code defect was reproduced. Stable publication is
still blocked by unresolved repository/registry identity, absent native
Linux/Windows CI results, and approval of the public compatibility contract.
Nothing was published, tagged, or presented as AXS 1.0.

## Scope and provenance

Reviewed Cargo metadata, every public module, lexer/token parser/primitive
parser/string codec/text serializer, AxsValue/AxsError, both private Serde
adapters, README, syntax/status documents, and all existing tests. The incoming
checkout already contained the uncommitted Serde milestone; those changes were
preserved. Baseline independently verified: 128 integration tests and one
doctest passed on aarch64 macOS with rustc 1.99.0.

The initial audit added eleven integration tests; its suite had **139 integration
tests plus one doctest**, with no removed or ignored tests. Existing coverage
includes 2,000 generated core trees, 20,000 float patterns, 5,000 arbitrary
inputs, and 2,000 Serde integer/float/string cases. New audit coverage adds
1,000 structured-document mutations with a fixed seed, deterministic maps,
map/enum schema interpretation, numeric flatten rejection, parsed specification examples, exact integer
boundary errors, every error code, bounded/escaped diagnostics, and a
subprocess-isolated 1 MiB stack probe. Sampling is not exhaustive fuzzing.

## Findings by severity

| ID | Severity | Finding / disposition |
|---|---|---|
| C0 | Critical | None reproduced in this audit. This is not a proof that none exist. |
| H1 | High | Initially no license or license file. Resolved by the owner's explicit Apache-2.0 selection: complete unmodified official LICENSE, Cargo metadata, README section, and verified packaged license. No longer a release blocker. |
| H2 | High | No Git remote or canonical repository URL; no native Linux/Windows CI results. Repository/registry identity and green native CI must be established before a multi-platform stable claim. Open release blocker. |
| M1 | Medium | Long-line diagnostics allocated output proportional to the entire line and span. A 100,000-character reproducer rendered 200,084 bytes. Fixed by bounded source windows, field lengths, and markers. |
| M2 | Medium | Diagnostic text copied raw terminal controls from untrusted source/message/filename/help. Fixed in render and Display; controls are escaped. Original error fields remain available. |
| M3 | Medium | Numeric deserialize_any buffering, exact f32 narrowing, nullable Some rejection, and object roots are intentional restrictions with substantial interoperability consequences. Regression tests and docs agree; approve these as stable contracts before 1.0. No syntax redesign attempted. |
| M4 | Medium | Size/token limits are not hard RSS/CPU quotas. Low-level string/value helpers have no document budget; text serialization may allocate temporary escaped text and sorted entry vectors. Bounded document workflows are tested; caller-owned oversized trees/custom implementations still need caller resource control. Documented trade-off. |
| L1 | Low | Missing MSRV, CI, description, readme, keywords/categories, and explicit package contents. Fixed without inventing authorship or URLs; licensing was subsequently resolved by explicit owner selection. |
| L2 | Low | MSRV Clippy raised comparison_chain, format_collect, and needless_lifetimes warnings. Fixed in lexer/test code, preserving tests and behavior. |
| L3 | Low | Public lexer/token/string/primitive modules, exhaustive enums, and legacy String-error APIs enlarge the compatibility surface. Preserved; owner must include them in the eventual stable API commitment. |
| L4 | Low | The existing demonstration binary and profile.axs are packaged. Decide whether the demonstration belongs in the stable distribution. |
| L5 | Low | The specification combined two whitespace alternatives with the same key in one text block, which fails as a complete document. Split into separately parseable examples; every text example now has a regression check. |

## Confirmed fixes and changed files

- `src/error.rs`: crop source to 100 Unicode scalars on either side of the
  marker, clip message/help to 1,024 scalars and filenames to 256, omit excerpts
  for sources exceeding 8 MiB, and escape controls. Display applies the same
  message policy. Codes, structs, original error fields, and signatures remain
  unchanged. UTF-8 boundaries and extreme caller-provided spans are tested.
- `src/lexer.rs`, `tests/serde.rs`: minimal equivalent rewrites for old Clippy
  warnings. No grammar or limit changes and no test removal.
- `tests/release_audit.rs`: eleven focused tests described above. Stack aborts are
  isolated in a child process so a regression becomes a normal test failure.
- `Cargo.toml`: declare Rust 1.85, factual description/readme/categories/
  keywords, explicit package files, and tested Serde 1.0.229 dependency floor
  (compatible 1.x updates allowed). Existing 0.1.0 version retained.
- `.github/workflows/ci.yml`: six jobs, stable and 1.85.0 on ubuntu-latest,
  macos-latest, windows-latest. Read-only token, no persisted credentials,
  commit-pinned checkout/toolchain actions, timeout, locked dependencies, and
  formatting/Clippy/tests/doctests/local package verification.
- `README.md`, `PARSER_STATUS.md`, `SERDE_STATUS.md`, `spec/AXS.md`: MSRV,
  diagnostics, compatibility/versioning policy, scope of helper budgets, and
  executed-vs-pending validation. README usage still compiles as a doctest.
- `CHANGELOG.md`, this file: unreleased history, findings, proposed release steps.

## Behavioral and safety review

The object-only root, two-space indentation, first-equals assignment rule,
full-line-only comments, literal # in values, quote decoding, leading-zero
strings, and decimal-only numeric grammar are unchanged. Lexing rejects tabs,
invalid indentation, jumps, keys, and excess physical depth. Parsing rejects
invalid block transitions, duplicate fields (including inline objects), and
implicit inline-object depth exceeding canonical block depth 128. Token-parser
loops consume tokens or return errors. Lexer unwraps access a nonempty indent
stack with a permanent zero sentinel; malformed input cannot remove it.

Integer syntax overflow errors rather than falling back to strings. Numeric
Serde conversions check signedness, bounds, and exact representability. Finite
f64 values, promoted f32 values, negative zero, and subnormals round-trip by bits.
Decimal text parsing retains the documented f64 rounding, including underflow;
this is not arbitrary-precision decimal storage. Exponent spellings, NaN, and
Infinity remain strings, while non-finite Rust floats fail serialization.

Externally tagged enums share ordinary map/string shapes. Interpretation is
schema-driven by T, as Serde formats normally require; the tree has no enum
variant type. An enum data variant and an ordinary single-key map may encode
identically, but each round-trips into its declared supported type. Unit enums
use strings; data variants use a single-key object. Invalid tags/payloads and
renamed keys outside the existing grammar fail.

None and unit use null. Some values that serialize as null are rejected because
they would deserialize as None; no extra syntax was introduced to distinguish
them. Required/default/optional/renamed/unknown/denied fields follow Serde's
standard concrete-type callbacks. Numeric buffering through untagged/flattened
representations fails predictably, rather than delegating unchecked casts to
Serde Content. Serialize cannot detect a derive attribute after it lowers to
ordinary values; successful serialization of an unsupported representation
alone does not guarantee its deserialization. Custom visitors can deliberately
transform values and are responsible for their own semantics.

Input/output documents are limited to 8 MiB and 262,144 tokens. Serde conversion
also checks 512 recursion edges, 262,144 serialization callbacks/nodes, and
8 MiB aggregate string/key bytes. Length hints do not reserve untrusted amounts;
collect_str uses bounded formatting. Deserialization owns data and sorts map
entries for deterministic error order; one key clone retains field-path context.
Serialization sorts keys deterministically. No unsafe code, native library
requirements, or second parser were introduced. Allocator failure, arbitrary
custom Serialize/Deserialize/Display code, and caller-created deeply recursive
AxsValue clone/drop operations remain outside format resource guarantees.

## Metadata and packaging

Name/version remain `axs` / `0.1.0`; edition remains 2024. Description, readme,
rust-version, categories, keywords, and explicit includes are now set. Package
contents include source/tests/spec/docs/CI and the existing demonstration; target
artifacts, attachments, and unrelated workspace files are excluded. The local
archive builds successfully with packaging verification.

`license = "Apache-2.0"` records the owner's explicit selection. Root `LICENSE`
contains the complete, unmodified official Apache License 2.0 text, including
its appendix and literal template placeholders. The existing `LICENSE*` include
pattern packages it; the archived license was verified byte-for-byte against
the root file and official download. No copyright owner or organization was
invented, and no separate NOTICE file was added.

The canonical repository remains unset because no authoritative URL exists
in this checkout. Authors were not inferred from Git names
or environment usernames; authorship metadata is optional. Dedicated homepage
and documentation URLs were not invented. Registry name ownership/availability
has not been verified; owner must establish it before publication. Local Cargo
packaging success does not validate registry ownership or prove crates.io
publishability. Cargo still warns that documentation, homepage, and repository
URLs are absent; no URLs were invented to suppress that warning.

## Executed verification (initial audit and licensing follow-up)

Native host: aarch64-apple-darwin, macOS. Stable rustc 1.99.0; MSRV rustc 1.85.0
(4d91de4e4, 2025-02-17), installed and executed rather than inferred.

| Exact command | Outcome |
|---|---|
| `cargo fmt --check` | Pass |
| `cargo clippy --all-targets -- -D warnings` | Pass |
| `cargo test` | Pass: 139 integration tests, one doctest |
| `cargo test --doc` | Pass: one README doctest |
| `cargo +1.85.0 fmt --check` | Pass |
| `cargo +1.85.0 clippy --locked --all-targets -- -D warnings` | Pass after the three minimal lint fixes |
| `cargo +1.85.0 test --locked` | Pass: full suite on actual MSRV |
| `cargo +1.85.0 test --locked --doc` | Pass |
| `cargo check --locked --all-targets --target x86_64-unknown-linux-gnu` | Pass, compilation only |
| `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` | Pass, compilation only |
| `cargo package --list` | Rejected dirty checkout, as expected; no existing work was committed merely to bypass this safety check |
| `cargo package --list --allow-dirty` | Pass after licensing update; LICENSE included in intended file list |
| `cargo package --allow-dirty` | Pass after licensing update with missing-URL metadata warning; local archive compilation and license contents verified |
| `cargo +1.85.0 package --locked --allow-dirty` | Pass with metadata warnings during the initial audit, before licensing update; archive compiled on MSRV |
| `git diff --check` | Pass |

Licensing follow-up: reran `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`,
`cargo package --list --allow-dirty`, and `cargo package --allow-dirty`.
All passed; tests remain 139 integration tests plus one README doctest.
Only LICENSE, Cargo.toml, README.md, CHANGELOG.md, and this audit were changed
in this follow-up. Grammar, public APIs, implementation, tests, dependencies,
and version were preserved. Documentation contains no conflicting license
declarations. No publication, tag, or version bump was performed.

Setup executed: `rustup toolchain install 1.85.0 --profile minimal --component
rustfmt --component clippy` and `rustup target add x86_64-unknown-linux-gnu
x86_64-pc-windows-gnu`. CI YAML parsed locally and its six-job matrix checked.
Action commits were resolved against their providers' repositories. These checks
are not an execution of GitHub Actions itself.

Cross-target tests were not linked or run. Docker CLI exists, but `docker info
--format '{{.OSType}} {{.Architecture}}'` fails because no local daemon socket
exists. No Windows runtime/emulator is available. No Git remote exists to run
hosted CI. Native Linux/Windows CI is pending; no cross-platform runtime support
claim is made solely from cargo check. Toolchain/dependency compatibility is
verified for the current Cargo.lock graph, not every future allowed Serde release.

## Verdict and proposed next steps

Recommend the next development version **0.2.0**, not 1.0.0 now. Manifest version
was deliberately not changed. Earlier informal development milestones are not
asserted to be published crate versions.

Release blockers, in order:

1. Owner confirms canonical repository and crates.io identity; fill only
   authoritative metadata. Apache-2.0 license selection is resolved.
2. Commit/review the changes and execute the native six-job CI matrix. Review
   the local package contents and demonstrate the packaged API for consumers.
3. Approve strict f32/numeric-buffering, nullable Some, enum/root, error-code,
   public-module, MSRV, and demonstration-target contracts. Document migrations
   if changing them; do not silently broaden grammar for convenience.
4. Once these pass, cut a reviewed 1.0.0-rc.1 candidate and validate representative
   downstream consumers and broader fuzz/property coverage before 1.0.0.

Further fuzzing, Unicode display-width diagnostics, retained source metadata
for Serde conversion spans, and hard process quotas remain known limitations or
future validation choices, not features introduced by this audit. The absence
of borrowed/streaming Serde APIs is an acceptable documented scope boundary.

References checked: [Rust 2024 / 1.85.0](https://doc.rust-lang.org/edition-guide/rust-2024/index.html),
[official Apache License 2.0 text](https://www.apache.org/licenses/LICENSE-2.0.txt),
[Cargo metadata requirements](https://doc.rust-lang.org/cargo/reference/manifest.html),
[checkout action](https://github.com/actions/checkout),
[Rust toolchain action](https://github.com/dtolnay/rust-toolchain).
