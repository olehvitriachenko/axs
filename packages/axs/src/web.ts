/**
 * Browser and bundler entry point. The WebAssembly module must be initialized
 * with `await init()` (or `initSync`) before `parse` or `stringify` are called.
 */
import initWasm, * as wasm from "../wasm/web.mjs";
import { type AxsObject, createApi } from "./core.js";

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
 * Serializes a plain object to canonical AXS text. Requires prior initialization.
 *
 * @throws {AxsError} for unsupported values or exceeded limits.
 */
export function stringify(value: AxsObject): string;
export function stringify(value: object): string;
export function stringify(value: object): string {
  return api.stringify(value);
}
