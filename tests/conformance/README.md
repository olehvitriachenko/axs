# AXS Grammar v3 conformance fixtures

Normative examples for [spec/AXS.md](../../spec/AXS.md), run by
`tests/conformance.rs`.

- `valid/NAME.axs` must parse to the value in `valid/NAME.json`. Object key
  order in the JSON is significant. A JSON number with a fraction or exponent
  is an AXS float; one without is an AXS integer.
- `invalid/NAME.axs` must be rejected. Its first line is a comment naming the
  expected error category from spec §14.

Every fixture also checks round trips: `parse(stringify(value)) == value`.
