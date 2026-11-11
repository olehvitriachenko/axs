# @axs-lang/core

Parse and serialize [AXS](https://github.com/olehvitriachenko/axs) documents
from JavaScript and TypeScript. The package runs the Rust AXS core compiled to
WebAssembly. Grammar, validation, limits, and canonical formatting are the same
as the Rust crate and the `axs` CLI, and you don't need Rust installed to use it.

> **Status:** pre-1.0. Version 0.1.0 is published on npm. This checkout is
> **0.2.0, which is not published**: it adds `check` and `format`.

The npm manifest links to this repository, this package directory, and the
shared GitHub issue tracker. For the published API, install with
`npm install @axs-lang/core`. To test this checkout's new API before publication,
build and install its local tarball as described below. The VS Code extension
uses `file:../../packages/axs` during development and bundles the SDK; after
0.2.0 is published and tested, that dependency can use the registry version.

```ts
import { parse, stringify } from "@axs-lang/core";

const source = `
server:
  port: 3000
  enabled: true
  skills:
    - Rust
    - TypeScript
`;

const config = parse(source);
// { server: { port: 3000, enabled: true, skills: ["Rust", "TypeScript"] } }

const encoded = stringify(config);
// "server:\n  port: 3000\n  enabled: true\n  skills[2]: [Rust,TypeScript]\n"
```

Grammar v3 (see the repository's `spec/AXS.md`) replaces v2: `key = value` is no
longer valid, and the SDK needs the matching WebAssembly build.

## API

```ts
function parse(source: string): AxsObject;
function stringify(value: AxsObject | object): string;
function check(source: string): void;    // 0.2.0+: validate like `axs check`
function format(source: string): string; // 0.2.0+: canonical text, identical to `axs fmt`
function query(document: object | string, query: string): AxsValue; // select/filter/project
function init(): Promise<void>; // see "Initialization"
class AxsError extends Error { code; kind; reason; line?; column?; length?; help?; path? }
type AxsValue = string | number | bigint | boolean | null | AxsValue[] | AxsObject;
interface AxsObject { [key: string]: AxsValue }
```

`parse` returns a plain object, because every AXS document has an object root.
Keys are returned in source order. Comments are discarded.

`stringify` produces canonical AXS: keys in insertion order, two-space
indentation, tables for arrays of uniform objects, inline arrays and small
inline objects, lengths on every array, minimal quoting, and LF newlines. An empty object produces `""`.
It accepts plain objects (including `Object.create(null)` objects) as the root.
It throws an `AxsError` for:

| Input | Code |
|---|---|
| Root that is an array, primitive, `null`, or class instance | `AXS004` |
| `undefined`, functions, symbols, sparse-array holes | `AXS004` |
| Class instances: `Date`, `Map`, `Set`, typed arrays, boxed primitives, user classes | `AXS004` |
| Cyclic references (shared, non-cyclic references are serialized at each use) | `AXS004` |
| `NaN`, `±Infinity`, integer-valued numbers outside the safe range, bigints outside i64 | `AXS008` |
| Enumerable symbol keys (keys that aren't bare AXS keys are written quoted) | `AXS003` |
| Strings or keys with unpaired UTF-16 surrogates | `AXS007` |
| Nesting deeper than 128 blocks | `AXS002` |
| Output over 8 MiB or more than 262,144 tokens | `AXS009` |

Getters are invoked while serializing, and exceptions they throw propagate
unchanged.

`check` validates a document without building JavaScript values and throws the
same `AxsError` as `parse`. `format` returns exactly what `axs fmt` writes. It
works on the Rust value tree, so unlike `stringify(parse(text))` it keeps `1.0`
as a float and keeps integral floats beyond 2^53. Like `axs fmt`, it drops
comments, sorts keys, and emits LF newlines. Both are tested byte-for-byte
against the Rust CLI.

## Numbers

AXS integers are signed 64-bit, and AXS floats are finite IEEE-754 doubles.
JavaScript `number` represents integers exactly only up to
`Number.MAX_SAFE_INTEGER` (2^53 − 1), so the SDK never silently rounds:

| AXS → JavaScript (`parse`) | |
|---|---|
| Integer within ±(2^53 − 1) | `number` |
| Integer outside that range (up to i64 limits) | `bigint` |
| Float | `number`, with `-0.0` kept as `-0`, and subnormals and boundaries kept bit-exact |

| JavaScript → AXS (`stringify`) | |
|---|---|
| `bigint` within i64 | Integer |
| `bigint` outside i64 | `AXS008` error |
| Safe integer `number` | Integer |
| Integer-valued `number` beyond the safe range (e.g. `2 ** 53`, `1e20`) | `AXS008` error: use `bigint` |
| `-0` | Float `-0.0` |
| Non-integer finite `number` | Float, written in decimal form that parses back to the same bits |
| `NaN`, `Infinity` | `AXS008` error |

Round-trip guarantees:

- `parse(stringify(value))` deep-equals `value` for every accepted value, with
  two exceptions: a bigint inside the safe range comes back as a `number`, and
  key order becomes sorted.
- `stringify(parse(text))` keeps every value, but AXS float values with no
  fractional part come back as integers (`x = 1.0` becomes `x = 1`), because
  JavaScript can't tell `1.0` from `1`.
- An integral float larger than 2^53 (such as `1e300`) parses exactly but can't
  be serialized from a plain `number`. Use a bigint if the value is an integer.

```ts
const decoded = parse(stringify({ id: 9223372036854775807n, name: "Oleh" }));
decoded.id; // 9223372036854775807n (bigint)
typeof parse("n = 42").n; // "number"
```

## Errors

```ts
import { AxsError, parse } from "@axs-lang/core";

try {
  parse("a = 1\na = 2");
} catch (error) {
  if (error instanceof AxsError) {
    error.code;    // "AXS005"
    error.kind;    // "DuplicateKey"
    error.line;    // 2
    error.column;  // 1, counted in Unicode code points, not UTF-16 units
    error.reason;  // the core's message without code or location
    error.message; // "AXS005: ... (line 2, column 1)"
  }
}
```

`line`, `column`, and `length` are present when the core reports a source
position. `path` (for example `$.server.skills[1]`) is present for values
rejected by `stringify`. Codes `AXS001`–`AXS009` are the same stable codes
used by the Rust crate. Errors other than `AxsError` (a `TypeError` for a
non-string `parse` argument, or exceptions from your getters) are not wrapped.

## Initialization

**Node.js:** the `node` export condition loads a CommonJS build that compiles the
WebAssembly module synchronously on first import. `parse` and `stringify` work
immediately, with both `import` and `require`. `init()` exists only so code shared
with browsers can `await init()`, and it resolves immediately.

**Browsers and bundlers:** the `browser`/`default` condition (or the explicit
`@axs-lang/core/web` subpath) is an ES module that needs asynchronous
initialization:

```ts
import { init, parse } from "@axs-lang/core";

await init(); // fetches axs_wasm_bg.wasm relative to the module
parse("a = 1");
```

`init()` is idempotent and safe to call concurrently. A failed initialization
can be retried. `init(input)` accepts a URL, `Response`, the module bytes, or a
`WebAssembly.Module`, and `initSync(bytesOrModule)` instantiates synchronously
when you already have the bytes. Calling `parse` or `stringify` before
initialization throws an `Error`. The `.wasm` file is also exported as
`@axs-lang/core/axs_wasm_bg.wasm` for setups that copy assets manually.

## Compatibility

| Environment | Status |
|---|---|
| Node.js 24 (CommonJS `require`, ESM `import`) | Tested: unit tests and a tarball-install consumer test |
| Node.js 22 | Declared minimum (`engines: >=22`, the oldest maintained LTS line; Node 20 reached end of life in April 2026). **Not tested locally.** |
| TypeScript 5.9 with `module` Node16/NodeNext, or `moduleResolution: Bundler` | Tested (type-check with `skipLibCheck: false`) |
| Browser via native ES modules (no bundler) | Manually verified in Chromium through `examples/browser`. No automated browser test. |
| Web build running in Node.js (bytes passed to `init`/`initSync`) | Tested |
| NestJS | The `examples/nestjs-config` loader matches `@nestjs/config`'s `load` factory shape and is tested without NestJS. NestJS itself is **not tested.** |
| Next.js (server) | **Not tested.** Next bundles server code, which breaks the Node build's file-relative `.wasm` lookup. Add `serverExternalPackages: ["@axs-lang/core"]` (Next 15) to load it from `node_modules` at runtime. |
| Bundlers (Vite, webpack 5, esbuild, Rollup) | **Not tested.** The web build uses the standard `new URL("…wasm", import.meta.url)` pattern. |
| Deno, Bun, Cloudflare Workers, React Native | **Not tested.** |

The WebAssembly module uses multi-value returns and reference types, which are
supported by current Node.js and evergreen browsers.

## Limits and known limitations

- The core's limits apply: 8 MiB of UTF-8 input or output, 262,144 tokens,
  and 128 nested block levels.
- WebAssembly memory grows to fit the largest document processed and is not
  returned to the operating system while the module stays loaded.
- `parse` copies the document into WebAssembly memory and builds the result
  object in JavaScript. Large documents briefly need several times their size
  in memory.
- No streaming, comment preservation, source-order preservation, or schema
  validation.
- A WebAssembly trap (which would indicate a bug) surfaces as a
  `WebAssembly.RuntimeError`. Reload the module after one.

## Building

Maintainers need Rust with the `wasm32-unknown-unknown` target, a
`wasm-bindgen` CLI matching the version locked in `Cargo.lock`, Node.js, and npm:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
cd packages/axs
npm ci
npm run build          # Rust -> wasm -> wasm-bindgen -> TypeScript, into dist/
npm test               # Node.js runtime tests against dist/
npm run test:package   # npm pack, install the tarball into fresh projects, compile and run
npm pack               # produces axs-lang-core-<version>.tgz (runs the build via prepack)
```

`dist/wasm/build-info.json` records the rustc and wasm-bindgen versions and the
SHA-256 of the shipped `.wasm`, so you can compare builds. Install the tarball
into any project with `npm install /path/to/axs-lang-core-<version>.tgz`.

## License

Apache-2.0. See [LICENSE](LICENSE).

## Queries

`query(document, text)` selects part of a document before it reaches a model
(see `spec/QUERY.md` in the repository). The document is a plain object or AXS
text:

```ts
import { query } from "@axs-lang/core";

query(data, "users[age > 22]{name}"); // [{ name: "Alex" }, { name: "Maria" }]
query(source, "users[-1].name");      // "Maria"
```

Invalid queries throw an `AxsError` with kind `InvalidQuery` (`AXS012`), a
column in the query, and the available field names as `help`.
