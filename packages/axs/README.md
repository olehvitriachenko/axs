# @axs-lang/core

Parse and serialize [AXS](https://github.com/olehvitriachenko/axs) documents
from JavaScript and TypeScript. The package runs the Rust AXS core compiled to
WebAssembly. Grammar, validation, limits, and canonical formatting are the same
as the Rust crate and the `axs` CLI, and you don't need Rust installed to use it.

> **Status:** unpublished pre-release (0.1.0). The package name
> `@axs-lang/core` is provisional, and the npm scope's ownership hasn't been
> established. `"private": true` prevents accidental publication. Install it
> from a locally built tarball (see [Building](#building)).

```ts
import { parse, stringify } from "@axs-lang/core";

const source = `
server:
  port = 3000
  enabled = true
  skills:
    - Rust
    - TypeScript
`;

const config = parse(source);
// { server: { enabled: true, port: 3000, skills: ["Rust", "TypeScript"] } }

const encoded = stringify(config);
// "server:\n  enabled = true\n  port = 3000\n  skills:\n    - Rust\n    - TypeScript\n"
```

## API

```ts
function parse(source: string): AxsObject;
function stringify(value: AxsObject | object): string;
function init(): Promise<void>; // see "Initialization"
class AxsError extends Error { code; kind; reason; line?; column?; length?; help?; path? }
type AxsValue = string | number | bigint | boolean | null | AxsValue[] | AxsObject;
interface AxsObject { [key: string]: AxsValue }
```

`parse` returns a plain object, because every AXS document has an object root.
Keys are returned in sorted order, since source key order isn't retained by the
core. Comments are discarded.

`stringify` produces canonical AXS: sorted keys, two-space indentation, colon
block headers, minimal quoting, and LF newlines. An empty object produces `""`.
It accepts plain objects (including `Object.create(null)` objects) as the root.
It throws an `AxsError` for:

| Input | Code |
|---|---|
| Root that is an array, primitive, `null`, or class instance | `AXS004` |
| `undefined`, functions, symbols, sparse-array holes | `AXS004` |
| Class instances: `Date`, `Map`, `Set`, typed arrays, boxed primitives, user classes | `AXS004` |
| Cyclic references (shared, non-cyclic references are serialized at each use) | `AXS004` |
| `NaN`, `±Infinity`, integer-valued numbers outside the safe range, bigints outside i64 | `AXS008` |
| Keys that aren't valid AXS keys (`[A-Za-z_][A-Za-z0-9_-]*`), enumerable symbol keys | `AXS003` |
| Strings or keys with unpaired UTF-16 surrogates | `AXS007` |
| Nesting deeper than 128 blocks | `AXS002` |
| Output over 8 MiB or more than 262,144 tokens | `AXS009` |

Getters are invoked while serializing, and exceptions they throw propagate
unchanged.

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
npm pack               # produces axs-lang-core-0.1.0.tgz (runs the build via prepack)
```

`dist/wasm/build-info.json` records the rustc and wasm-bindgen versions and the
SHA-256 of the shipped `.wasm`, so you can compare builds. Install the tarball
into any project with `npm install /path/to/axs-lang-core-0.1.0.tgz`.

## License

Apache-2.0. See [LICENSE](LICENSE).
