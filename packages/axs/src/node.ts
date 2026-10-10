/**
 * Node.js entry point. The WebAssembly module is compiled synchronously when
 * this module is first loaded, so `parse` and `stringify` work immediately.
 */
import * as wasm from "../wasm/node.cjs";
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

const api = createApi(() => wasm);

/**
 * Parses an AXS document.
 *
 * Integers within `Number.MIN_SAFE_INTEGER..Number.MAX_SAFE_INTEGER` become
 * `number`; larger i64 values become `bigint`.
 *
 * @throws {AxsError} for invalid documents.
 */
export function parse(source: string): AxsObject {
  return api.parse(source);
}

/**
 * Serializes a plain object to canonical AXS text (sorted keys, LF newlines).
 *
 * @throws {AxsError} for unsupported values: undefined, functions, symbols,
 * cycles, non-finite numbers, unsafe integer numbers (use bigint), bigints
 * outside i64, class instances, invalid keys, or exceeded limits.
 */
export function stringify(value: AxsObject): string;
export function stringify(value: object): string;
export function stringify(value: object): string {
  return api.stringify(value);
}

/**
 * Resolves immediately: the Node.js build initializes synchronously. Provided
 * so code shared with the browser build can always `await init()`.
 */
export function init(): Promise<void> {
  return Promise.resolve();
}
