/**
 * Browser and bundler entry point. The WebAssembly module must be initialized
 * with `await init()` (or `initSync`) before `parse` or `stringify` are called.
 */
import initWasm, * as wasm from "../wasm/web.mjs";
import { type AxsObject, type AxsValue, createApi } from "./core.js";

export {
  AxsError,
  type AxsArray,
  type AxsErrorCode,
  type AxsErrorDetails,
  type AxsErrorKind,
  type AxsObject,
  type AxsPrimitive,
  type AxsValue
} from "./core.js";

/**
 * Where to load the WebAssembly module from: a URL (fetched), a `Response`,
 * the module bytes, or a compiled `WebAssembly.Module`.
 */
export type InitInput =
  | string
  | URL
  | Request
  | Response
  | ArrayBuffer
  | ArrayBufferView
  | WebAssembly.Module;

let ready = false;
let pending: Promise<void> | undefined;

const api = createApi(() => {
  if (!ready) {
    throw new Error(
      "AXS WebAssembly is not initialized; call `await init()` before parse() or stringify()"
    );
  }
  return wasm;
});

/**
 * Loads and instantiates the WebAssembly module. Safe to call repeatedly and
 * concurrently; later calls reuse the first successful initialization. A failed
 * initialization can be retried.
 *
 * Without an argument, the module is fetched from `axs_wasm_bg.wasm` next to
 * this file (resolved via `import.meta.url`, which bundlers such as Vite and
 * webpack 5 rewrite to an emitted asset).
 */
export function init(input?: InitInput | Promise<InitInput>): Promise<void> {
  if (ready) return Promise.resolve();
  pending ??= initWasm(input === undefined ? undefined : { module_or_path: input })
    .then(() => {
      ready = true;
    })
    .catch((error: unknown) => {
      pending = undefined;
      throw error;
    });
  return pending;
}

/**
 * Instantiates the WebAssembly module synchronously from its bytes or a compiled
 * module. Browsers may restrict synchronous compilation of large modules on the
 * main thread; prefer {@link init} there.
 */
export function initSync(module: ArrayBuffer | ArrayBufferView | WebAssembly.Module): void {
  if (ready) return;
  wasm.initSync({ module });
  ready = true;
}

/**
 * Parses an AXS document. Requires prior initialization.
 *
 * @throws {AxsError} for invalid documents.
 */
export function parse(source: string): AxsObject {
  return api.parse(source);
}

/**
 * Validates an AXS document without building a JavaScript value, like
 * `axs check`. Returns normally for valid documents.
 *
 * @throws {AxsError} for invalid documents, with the core's location and code.
 */
export function check(source: string): void {
  api.check(source);
}

/**
 * Canonically formats an AXS document, producing the same text as `axs fmt`
 * (key order kept, canonical quoting, LF newlines). Values never pass through
 * JavaScript numbers, so `1.0` stays a float and large floats are kept exactly.
 * Comments and original quoting are not preserved.
 *
 * @throws {AxsError} for invalid documents.
 */
export function format(source: string): string {
  return api.format(source);
}

/**
 * Serializes a plain object to canonical AXS text. Requires prior initialization.
 *
 * @throws {AxsError} for unsupported values or exceeded limits.
 */
export function stringify(value: AxsObject): string;
export function stringify(value: object): string;
export function stringify(value: object): string {
  return api.stringify(value);
}

/**
 * Runs an AXS query (`users[age > 20]{name}`; see spec/QUERY.md) on a
 * document, given as AXS text or a plain object, and returns the selected
 * value. Requires prior initialization.
 *
 * @throws {AxsError} with kind `InvalidQuery` (AXS012) for invalid queries,
 * missing fields, or out-of-range indexes, or for invalid documents.
 */
export function query(document: object | string, query: string): AxsValue {
  return api.query(document, query);
}
