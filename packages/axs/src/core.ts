/**
 * Environment-independent part of the AXS SDK: public types, the error class,
 * and the binary payload codec shared with `crates/axs-wasm/src/codec.rs`.
 *
 * AXS grammar is never interpreted here. Parsing and serialization run in the
 * Rust core compiled to WebAssembly; this module only validates JavaScript
 * values and moves them across the boundary without losing precision.
 */

/** A primitive AXS value as represented in JavaScript. */
export type AxsPrimitive = string | number | bigint | boolean | null;

/** Any AXS value as represented in JavaScript. */
export type AxsValue = AxsPrimitive | AxsArray | AxsObject;

/** An AXS array. */
export type AxsArray = AxsValue[];

/** An AXS object. Every AXS document has an object root. */
export interface AxsObject {
  [key: string]: AxsValue;
}

/** Structured error categories reported by the AXS core. */
export type AxsErrorKind =
  | "UnexpectedToken"
  | "InvalidIndentation"
  | "InvalidKey"
  | "InvalidValue"
  | "DuplicateKey"
  | "UnexpectedEof"
  | "InvalidString"
  | "InvalidNumber"
  | "SyntaxError";

/** Stable AXS error codes, one per {@link AxsErrorKind}. */
export type AxsErrorCode =
  | "AXS001"
  | "AXS002"
  | "AXS003"
  | "AXS004"
  | "AXS005"
  | "AXS006"
  | "AXS007"
  | "AXS008"
  | "AXS009";

const CODES: Record<AxsErrorKind, AxsErrorCode> = {
  UnexpectedToken: "AXS001",
  InvalidIndentation: "AXS002",
  InvalidKey: "AXS003",
  InvalidValue: "AXS004",
  DuplicateKey: "AXS005",
  UnexpectedEof: "AXS006",
  InvalidString: "AXS007",
  InvalidNumber: "AXS008",
  SyntaxError: "AXS009"
};

export interface AxsErrorDetails {
  /** Unformatted message from the AXS core or SDK validation. */
  reason: string;
  /** 1-based line of the problem in the parsed source, when known. */
  line?: number;
  /**
   * 1-based column in Unicode code points (not UTF-16 code units), when known.
   * Astral characters such as emoji count as one column.
   */
  column?: number;
  /** Length of the reported span in Unicode code points, when known. */
  length?: number;
  /** Suggested fix, when the core provides one. */
  help?: string;
  /**
   * Location of the offending value for `stringify` validation errors, such as
   * `$.server.skills[1]`.
   */
  path?: string;
}

/** Error thrown for invalid AXS documents and unsupported JavaScript values. */
export class AxsError extends Error {
  override readonly name = "AxsError";
  readonly kind: AxsErrorKind;
  readonly code: AxsErrorCode;
  readonly reason: string;
  readonly line?: number;
  readonly column?: number;
  readonly length?: number;
  readonly help?: string;
  readonly path?: string;

  constructor(kind: AxsErrorKind, details: AxsErrorDetails) {
    super(formatMessage(CODES[kind], details));
    this.kind = kind;
    this.code = CODES[kind];
    this.reason = details.reason;
    if (details.line !== undefined) this.line = details.line;
    if (details.column !== undefined) this.column = details.column;
    if (details.length !== undefined) this.length = details.length;
    if (details.help !== undefined) this.help = details.help;
    if (details.path !== undefined) this.path = details.path;
  }
}

function formatMessage(code: string, details: AxsErrorDetails): string {
  let message = `${code}: ${details.reason}`;
  if (details.line !== undefined && details.column !== undefined) {
    message += ` (line ${details.line}, column ${details.column})`;
  }
  if (details.path !== undefined) {
    message += ` at ${details.path}`;
  }
  return message;
}

/** Low-level functions exported by the wasm-bindgen glue. */
export interface WasmBindings {
  parse(source: string): Uint8Array;
  stringify(payload: Uint8Array): Uint8Array;
  payloadVersion(): number;
}

// Limits mirrored from the Rust core (src/lexer.rs) so oversized or deeply
// nested input fails fast in JavaScript; the core enforces them again.
const MAX_INPUT_BYTES = 8 * 1024 * 1024;
const MAX_NESTING_DEPTH = 128;
const MAX_TOKENS = 262_144;
const PAYLOAD_VERSION = 1;

const TAG_NULL = 0;
const TAG_FALSE = 1;
const TAG_TRUE = 2;
const TAG_INTEGER = 3;
const TAG_FLOAT = 4;
const TAG_STRING = 5;
const TAG_ARRAY = 6;
const TAG_OBJECT = 7;

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const SAFE_MIN = BigInt(Number.MIN_SAFE_INTEGER);
const SAFE_MAX = BigInt(Number.MAX_SAFE_INTEGER);

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

const LONE_SURROGATE =
  /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;

function isWellFormed(text: string): boolean {
  const native = (text as { isWellFormed?: () => boolean }).isWellFormed;
  return typeof native === "function"
    ? native.call(text)
    : !LONE_SURROGATE.test(text);
}

/** Creates `parse` and `stringify` over a lazily supplied set of bindings. */
export function createApi(bindings: () => WasmBindings): {
  parse(source: string): AxsObject;
  stringify(value: object): string;
} {
  let checked = false;
  const load = (): WasmBindings => {
    const wasm = bindings();
    if (!checked) {
      const version = wasm.payloadVersion();
      if (version !== PAYLOAD_VERSION) {
        throw new Error(
          `AXS WebAssembly payload version ${version} does not match SDK version ${PAYLOAD_VERSION}`
        );
      }
      checked = true;
    }
    return wasm;
  };

  return {
    parse(source: string): AxsObject {
      if (typeof source !== "string") {
        throw new TypeError("AXS source must be a string");
      }
      // UTF-8 never needs fewer bytes than UTF-16 code units, so this rejects
      // oversized input before copying it into WebAssembly memory.
      if (source.length > MAX_INPUT_BYTES) {
        throw sizeError();
      }
      // wasm-bindgen would silently replace unpaired surrogates with U+FFFD.
      if (!isWellFormed(source)) {
        throw new AxsError("InvalidString", {
          reason: "Source contains an unpaired UTF-16 surrogate"
        });
      }
      const result = load().parse(source);
      return new Reader(result).object();
    },

    stringify(value: object): string {
      const payload = encodeDocument(value);
      const result = load().stringify(payload);
      if (result[0] !== 0) {
        throw new Reader(result).failure();
      }
      return decoder.decode(result.subarray(1));
    }
  };
}

function sizeError(): AxsError {
  return new AxsError("SyntaxError", {
    reason: "Input too large",
    help: `AXS supports at most ${MAX_INPUT_BYTES} input bytes`
  });
}

// ---------------------------------------------------------------------------
// Decoding (WebAssembly -> JavaScript)

class Reader {
  private offset = 0;
  private readonly view: DataView;

  constructor(private readonly bytes: Uint8Array) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  /** Reads a parse envelope: returns the object or throws its AxsError. */
  object(): AxsObject {
    if (this.u8() !== 0) {
      throw this.error();
    }
    const value = this.value();
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new Error("AXS WebAssembly returned a non-object document");
    }
    return value;
  }

  /** Reads a failed envelope's AxsError. */
  failure(): AxsError {
    this.u8();
    return this.error();
  }

  private error(): AxsError {
    this.string(); // code: derived from the kind on the JavaScript side
    const kind = this.string() as AxsErrorKind;
    const reason = this.string();
    const details: AxsErrorDetails = { reason };
    if (this.u8() === 1) {
      details.line = this.u32();
      details.column = this.u32();
      details.length = this.u32();
    }
    if (this.u8() === 1) {
      details.help = this.string();
    }
    if (!(kind in CODES)) {
      throw new Error(`AXS WebAssembly returned an unknown error kind: ${kind}`);
    }
    return new AxsError(kind, details);
  }

  private u8(): number {
    const value = this.view.getUint8(this.offset);
    this.offset += 1;
    return value;
  }

  private u32(): number {
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  private string(): string {
    const length = this.u32();
    const text = decoder.decode(
      this.bytes.subarray(this.offset, this.offset + length)
    );
    this.offset += length;
    return text;
  }

  private value(): AxsValue {
    const tag = this.u8();
    switch (tag) {
      case TAG_NULL:
        return null;
      case TAG_FALSE:
        return false;
      case TAG_TRUE:
        return true;
      case TAG_INTEGER: {
        const value = this.view.getBigInt64(this.offset, true);
        this.offset += 8;
        return value >= SAFE_MIN && value <= SAFE_MAX ? Number(value) : value;
      }
      case TAG_FLOAT: {
        const value = this.view.getFloat64(this.offset, true);
        this.offset += 8;
        return value;
      }
      case TAG_STRING:
        return this.string();
      case TAG_ARRAY: {
        const count = this.u32();
        const items: AxsValue[] = new Array(count);
        for (let index = 0; index < count; index += 1) {
          items[index] = this.value();
        }
        return items;
      }
      case TAG_OBJECT: {
        const count = this.u32();
        const object: AxsObject = {};
        for (let index = 0; index < count; index += 1) {
          const key = this.string();
          const value = this.value();
          if (key === "__proto__") {
            // Plain assignment would replace the prototype instead.
            Object.defineProperty(object, key, {
              value,
              writable: true,
              enumerable: true,
              configurable: true
            });
          } else {
            object[key] = value;
          }
        }
        return object;
      }
      default:
        throw new Error(`AXS WebAssembly returned an unknown value tag: ${tag}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Encoding (JavaScript -> WebAssembly)

class Writer {
  bytes = new Uint8Array(1024);
  view = new DataView(this.bytes.buffer);
  length = 0;

  private reserve(extra: number): void {
    const needed = this.length + extra;
    if (needed <= this.bytes.length) return;
    let capacity = this.bytes.length * 2;
    while (capacity < needed) capacity *= 2;
    const next = new Uint8Array(capacity);
    next.set(this.bytes.subarray(0, this.length));
    this.bytes = next;
    this.view = new DataView(next.buffer);
  }

  u8(value: number): void {
    this.reserve(1);
    this.view.setUint8(this.length, value);
    this.length += 1;
  }

  u32(value: number): void {
    this.reserve(4);
    this.view.setUint32(this.length, value, true);
    this.length += 4;
  }

  i64(value: bigint): void {
    this.reserve(8);
    this.view.setBigInt64(this.length, value, true);
    this.length += 8;
  }

  f64(value: number): void {
    this.reserve(8);
    this.view.setFloat64(this.length, value, true);
    this.length += 8;
  }

  /** Writes a length-prefixed UTF-8 string and returns its byte length. */
  string(text: string): number {
    // Worst case is three UTF-8 bytes per UTF-16 code unit.
    this.reserve(4 + text.length * 3);
    const start = this.length + 4;
    const { written } = encoder.encodeInto(
      text,
      this.bytes.subarray(start, start + text.length * 3)
    );
    this.view.setUint32(this.length, written, true);
    this.length = start + written;
    return written;
  }
}

interface EncodeState {
  writer: Writer;
  ancestors: Set<object>;
  nodes: number;
  bytes: number;
}

function valueError(
  kind: AxsErrorKind,
  reason: string,
  path: string,
  help?: string
): AxsError {
  const details: AxsErrorDetails = { reason, path };
  if (help !== undefined) details.help = help;
  return new AxsError(kind, details);
}

function describe(value: unknown): string {
  if (value === undefined) return "undefined";
  if (typeof value === "function") return "a function";
  if (typeof value === "symbol") return "a symbol";
  const prototype = Object.getPrototypeOf(value) as { constructor?: { name?: unknown } } | null;
  const name = prototype?.constructor?.name;
  return typeof name === "string" && name !== "" ? `a ${name} instance` : "an object";
}

function isPlainObject(value: object): boolean {
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function propertyPath(parent: string, key: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key)
    ? `${parent}.${key}`
    : `${parent}[${JSON.stringify(key)}]`;
}

function encodeDocument(value: unknown): Uint8Array {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    !isPlainObject(value)
  ) {
    const what = Array.isArray(value)
      ? "an array"
      : value === null
        ? "null"
        : typeof value === "object"
          ? describe(value)
          : `a ${typeof value}`;
    throw valueError(
      "InvalidValue",
      `AXS documents must have a plain object root, not ${what}`,
      "$",
      "Wrap the value in an object, for example { items: [...] }"
    );
  }
  const state: EncodeState = {
    writer: new Writer(),
    ancestors: new Set(),
    nodes: 0,
    bytes: 0
  };
  encodeObject(value, 0, "$", state);
  return state.writer.bytes.subarray(0, state.writer.length);
}

function countNode(state: EncodeState, path: string, extra = 1): void {
  state.nodes += extra;
  if (state.nodes > MAX_TOKENS) {
    throw valueError(
      "SyntaxError",
      "Maximum token count exceeded",
      path,
      `AXS supports at most ${MAX_TOKENS} tokens including EOF`
    );
  }
}

function encodeText(text: string, path: string, state: EncodeState): void {
  if (!isWellFormed(text)) {
    throw valueError(
      "InvalidString",
      "String contains an unpaired UTF-16 surrogate",
      path
    );
  }
  if (text.length > MAX_INPUT_BYTES - state.bytes) {
    throw valueError("SyntaxError", "Maximum output size exceeded", path);
  }
  state.bytes += state.writer.string(text);
  if (state.bytes > MAX_INPUT_BYTES) {
    throw valueError("SyntaxError", "Maximum output size exceeded", path);
  }
}

function enterCollection(
  value: object,
  depth: number,
  path: string,
  state: EncodeState
): void {
  if (state.ancestors.has(value)) {
    throw valueError("InvalidValue", "Cyclic reference", path);
  }
  // The root object is depth 0; an empty collection may sit one level below
  // the deepest block, matching the Rust serializer and payload decoder.
  if (depth > MAX_NESTING_DEPTH + 1) {
    throw valueError(
      "InvalidIndentation",
      `Maximum nesting depth exceeded (at most ${MAX_NESTING_DEPTH} levels)`,
      path
    );
  }
  state.ancestors.add(value);
}

function encodeObject(
  object: object,
  depth: number,
  path: string,
  state: EncodeState
): void {
  enterCollection(object, depth, path, state);
  for (const symbol of Object.getOwnPropertySymbols(object)) {
    if (Object.prototype.propertyIsEnumerable.call(object, symbol)) {
      throw valueError(
        "InvalidKey",
        `Symbol keys are not supported (${String(symbol)})`,
        path
      );
    }
  }
  const keys = Object.keys(object);
  countNode(state, path, keys.length);
  const { writer } = state;
  writer.u8(TAG_OBJECT);
  writer.u32(keys.length);
  for (const key of keys) {
    const childPath = propertyPath(path, key);
    encodeText(key, childPath, state);
    encodeValue((object as Record<string, unknown>)[key], depth + 1, childPath, state);
  }
  state.ancestors.delete(object);
}

function encodeArray(
  array: unknown[],
  depth: number,
  path: string,
  state: EncodeState
): void {
  enterCollection(array, depth, path, state);
  const length = array.length;
  countNode(state, path, length);
  const { writer } = state;
  writer.u8(TAG_ARRAY);
  writer.u32(length);
  for (let index = 0; index < length; index += 1) {
    const childPath = `${path}[${index}]`;
    if (!(index in array)) {
      throw valueError(
        "InvalidValue",
        "Sparse array holes are not supported",
        childPath
      );
    }
    encodeValue(array[index], depth + 1, childPath, state);
  }
  state.ancestors.delete(array);
}

function encodeValue(
  value: unknown,
  depth: number,
  path: string,
  state: EncodeState
): void {
  const { writer } = state;
  switch (typeof value) {
    case "string":
      writer.u8(TAG_STRING);
      encodeText(value, path, state);
      return;
    case "boolean":
      writer.u8(value ? TAG_TRUE : TAG_FALSE);
      return;
    case "number":
      encodeNumber(value, path, writer);
      return;
    case "bigint":
      if (value < I64_MIN || value > I64_MAX) {
        throw valueError(
          "InvalidNumber",
          `Integer ${value} is outside the signed 64-bit range`,
          path
        );
      }
      writer.u8(TAG_INTEGER);
      writer.i64(value);
      return;
    case "object":
      if (value === null) {
        writer.u8(TAG_NULL);
      } else if (Array.isArray(value)) {
        encodeArray(value, depth, path, state);
      } else if (isPlainObject(value)) {
        encodeObject(value, depth, path, state);
      } else {
        throw valueError(
          "InvalidValue",
          `Unsupported value: ${describe(value)}`,
          path,
          "Only plain objects, arrays, strings, finite numbers, bigints, booleans, and null are supported"
        );
      }
      return;
    default:
      throw valueError(
        "InvalidValue",
        `Unsupported value: ${describe(value)}`,
        path,
        "Only plain objects, arrays, strings, finite numbers, bigints, booleans, and null are supported"
      );
  }
}

function encodeNumber(value: number, path: string, writer: Writer): void {
  if (!Number.isFinite(value)) {
    throw valueError(
      "InvalidNumber",
      `Non-finite number ${value} is not supported`,
      path
    );
  }
  if (Object.is(value, -0) || !Number.isInteger(value)) {
    writer.u8(TAG_FLOAT);
    writer.f64(value);
    return;
  }
  if (!Number.isSafeInteger(value)) {
    throw valueError(
      "InvalidNumber",
      `Integer-valued number ${value} exceeds Number.MAX_SAFE_INTEGER and may already have lost precision`,
      path,
      "Use a bigint for integers outside the safe range"
    );
  }
  writer.u8(TAG_INTEGER);
  writer.i64(BigInt(value));
}
