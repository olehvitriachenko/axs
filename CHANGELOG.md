# Changelog

## Unreleased

No published release history was verified. The manifest is still 0.1.0; earlier
development milestone labels are not asserted to be crates.io releases.

### Added

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
  published. Apache-2.0 licensing is resolved; canonical repository metadata
  still requires the owner's decision.

## Proposed next versions

Recommend **0.2.0** for the next development release once repository and registry
identity are resolved. This recommendation uses the actual 0.1.0 manifest,
not informal milestone numbers. After native CI passes and public contracts are
approved, consider **1.0.0-rc.1**, followed by 1.0.0 only after release-candidate
validation. No version change in this file declares a release.
