"use strict";
// Runtime tests for the Node.js (CommonJS) entry point, loaded through the
// package's own name so the `exports` map is exercised as consumers see it.

const test = require("node:test");
const assert = require("node:assert/strict");
const { parse, stringify, init, AxsError } = require("@axs-lang/core");

function expectAxsError(callback, expected) {
  let caught;
  try {
    callback();
  } catch (error) {
    caught = error;
  }
  assert.ok(caught instanceof AxsError, `expected AxsError, got ${caught}`);
  assert.ok(caught instanceof Error);
  assert.equal(caught.name, "AxsError");
  for (const [key, value] of Object.entries(expected)) {
    assert.deepEqual(caught[key], value, `error.${key}`);
  }
  return caught;
}

test("parses and stringifies the documented example", () => {
  const source = `
server:
  port = 3000
  enabled = true
  skills:
    - Rust
    - TypeScript
`;
  const config = parse(source);
  assert.deepEqual(config, {
    server: { enabled: true, port: 3000, skills: ["Rust", "TypeScript"] }
  });
  const encoded = stringify(config);
  assert.equal(
    encoded,
    "server:\n  enabled = true\n  port = 3000\n  skills:\n    - Rust\n    - TypeScript\n"
  );
  assert.deepEqual(parse(encoded), config);
});

test("preserves i64 maximum as bigint through a round trip", () => {
  const original = { id: 9223372036854775807n, name: "Oleh", enabled: true };
  const encoded = stringify(original);
  assert.equal(encoded, "enabled = true\nid = 9223372036854775807\nname = Oleh\n");
  const decoded = parse(encoded);
  assert.equal(typeof decoded.id, "bigint");
  assert.equal(decoded.id, 9223372036854775807n);
  assert.deepEqual(decoded, original);
});

test("maps integers to number inside the safe range and bigint outside it", () => {
  const decoded = parse(
    [
      "max = 9223372036854775807",
      "min = -9223372036854775808",
      "safe = 9007199254740991",
      "safeNegative = -9007199254740991",
      "unsafe = 9007199254740992",
      "unsafeNegative = -9007199254740992",
      "zero = 0"
    ].join("\n")
  );
  assert.equal(decoded.max, 9223372036854775807n);
  assert.equal(decoded.min, -9223372036854775808n);
  assert.equal(decoded.safe, Number.MAX_SAFE_INTEGER);
  assert.equal(decoded.safeNegative, Number.MIN_SAFE_INTEGER);
  assert.equal(decoded.unsafe, 9007199254740992n);
  assert.equal(decoded.unsafeNegative, -9007199254740992n);
  assert.equal(decoded.zero, 0);
  assert.deepEqual(parse(stringify(decoded)), decoded);
});

test("accepts bigint only within i64 and normalizes small bigints to numbers", () => {
  assert.equal(stringify({ a: 5n }), "a = 5\n");
  assert.equal(parse(stringify({ a: 5n })).a, 5);
  assert.equal(stringify({ a: -(2n ** 63n) }), "a = -9223372036854775808\n");
  expectAxsError(() => stringify({ a: 2n ** 63n }), {
    code: "AXS008",
    kind: "InvalidNumber",
    path: "$.a"
  });
  expectAxsError(() => stringify({ a: -(2n ** 63n) - 1n }), { code: "AXS008" });
});

test("rejects unsafe integer numbers instead of losing precision", () => {
  const error = expectAxsError(() => stringify({ id: 2 ** 53 }), {
    code: "AXS008",
    kind: "InvalidNumber",
    path: "$.id"
  });
  assert.match(error.help, /bigint/);
  expectAxsError(() => stringify({ id: -(2 ** 53) }), { code: "AXS008" });
  expectAxsError(() => stringify({ huge: Number.MAX_VALUE }), { code: "AXS008" });
  assert.equal(stringify({ id: Number.MAX_SAFE_INTEGER }), "id = 9007199254740991\n");
});

test("preserves negative zero and float boundaries", () => {
  assert.equal(stringify({ z: -0 }), "z = -0.0\n");
  assert.ok(Object.is(parse("z = -0.0").z, -0));
  assert.ok(Object.is(parse(stringify({ z: -0 })).z, -0));
  assert.ok(Object.is(parse("z = 0.0").z, 0));

  const floats = [
    0.1,
    -0.5,
    1.5,
    Math.PI,
    Number.EPSILON,
    Number.MIN_VALUE, // smallest subnormal
    2.2250738585072014e-308, // smallest normal
    2.225073858507201e-308, // largest subnormal
    1e-300,
    123456.789,
    4503599627370495.5 // largest double with a fractional part
  ];
  for (const value of floats) {
    const decoded = parse(stringify({ value })).value;
    assert.ok(Object.is(decoded, value), `${value} round-trips, got ${decoded}`);
  }

  // Integral floats beyond 2^53 parse exactly (stringify would require bigint).
  const max = parse(`max = ${BigInt(Number.MAX_VALUE)}.0`).max;
  assert.equal(max, Number.MAX_VALUE);
  // Integral floats within the safe range become JavaScript integers.
  assert.equal(stringify(parse("x = 1.0")), "x = 1\n");
});

test("rejects non-finite numbers", () => {
  for (const value of [NaN, Infinity, -Infinity]) {
    expectAxsError(() => stringify({ value }), {
      code: "AXS008",
      kind: "InvalidNumber",
      path: "$.value"
    });
  }
});

test("handles nested objects, arrays, and Grammar v2 inline objects", () => {
  const source = [
    "user:",
    "  name = Oleh",
    "  tags:",
    "    - a",
    "    -",
    "      - nested",
    "categories:",
    "  - UI:",
    "      - MUI",
    "      - Tailwind",
    "    weight = 2",
    "  -",
    "    name = plain"
  ].join("\n");
  const value = parse(source);
  assert.deepEqual(value, {
    categories: [{ UI: ["MUI", "Tailwind"], weight: 2 }, { name: "plain" }],
    user: { name: "Oleh", tags: ["a", ["nested"]] }
  });
  assert.deepEqual(parse(stringify(value)), value);
});

test("rejects the legacy bare block syntax", () => {
  expectAxsError(() => parse("user\n  name = Oleh"), {});
});

test("round-trips empty collections and empty documents", () => {
  const value = { a: [], b: {}, c: [[], {}], d: { e: [] } };
  assert.deepEqual(parse(stringify(value)), value);
  assert.equal(stringify({}), "");
  assert.deepEqual(parse(""), {});
  assert.deepEqual(parse("\n\n"), {});
});

test("quotes reserved and ambiguous strings so they stay strings", () => {
  const strings = [
    "true",
    "false",
    "null",
    "123",
    "-7",
    "1.5",
    "[]",
    "{}",
    "",
    " padded ",
    "UI:",
    "http://host",
    "a = b",
    "- item",
    '"quoted"',
    "#comment"
  ];
  const value = { items: strings, single: "true", empty: "" };
  const decoded = parse(stringify(value));
  assert.deepEqual(decoded, value);
  for (const item of decoded.items) assert.equal(typeof item, "string");
});

test("round-trips Unicode and escaped strings", () => {
  const value = {
    cyrillic: "Олег",
    emoji: "🚀 launch",
    cjk: "設定",
    combining: "e\u0301",
    newline: "line one\nline two",
    tab: "a\tb",
    quote: 'say "hi"',
    backslash: "C:\\path\\file",
    controls: "\u0000\u0001\u001f\u007f",
    separators: "\u2028\u2029"
  };
  assert.deepEqual(parse(stringify(value)), value);
  assert.equal(parse('s = "a\\nb\\t\\"c\\""').s, 'a\nb\t"c"');
});

test("reports structured syntax errors with code-point columns", () => {
  const error = expectAxsError(() => parse('name = "🚀\\q"'), {
    code: "AXS007",
    kind: "InvalidString",
    line: 1,
    column: 8
  });
  assert.match(error.message, /^AXS007: /);
  assert.match(error.message, /line 1, column 8/);
  assert.equal(typeof error.reason, "string");
  assert.equal(typeof error.length, "number");
});

test("reports duplicate keys", () => {
  expectAxsError(() => parse("a = 1\na = 2"), {
    code: "AXS005",
    kind: "DuplicateKey",
    line: 2
  });
});

test("reports invalid indentation", () => {
  expectAxsError(() => parse("a:\n   b = 1"), { code: "AXS002", kind: "InvalidIndentation" });
  expectAxsError(() => parse("a:\n\tb = 1"), { kind: "InvalidIndentation" });
});

test("reports invalid keys from the core serializer", () => {
  expectAxsError(() => stringify({ "bad key": 1 }), { code: "AXS003", kind: "InvalidKey" });
  expectAxsError(() => stringify({ "1abc": 1 }), { code: "AXS003" });
  expectAxsError(() => stringify({ ok: { "a.b": 1 } }), { code: "AXS003" });
});

test("enforces the nesting depth limit in both directions", () => {
  const nest = (levels) => {
    let value = { leaf: 1 };
    for (let index = 0; index < levels; index += 1) value = { n: value };
    return value;
  };
  // Root plus 128 nested blocks is the maximum.
  const deepest = nest(128);
  assert.deepEqual(parse(stringify(deepest)), deepest);
  expectAxsError(() => stringify(nest(129)), { kind: "InvalidIndentation" });
  // An empty collection may sit inline below the deepest block.
  let withEmpty = { leaf: {} };
  for (let index = 0; index < 128; index += 1) withEmpty = { n: withEmpty };
  assert.deepEqual(parse(stringify(withEmpty)), withEmpty);
  expectAxsError(() => stringify(nest(100_000)), {
    kind: "InvalidIndentation",
    code: "AXS002"
  });

  let source = "";
  for (let depth = 0; depth <= 130; depth += 1) source += `${"  ".repeat(depth)}n:\n`;
  source += `${"  ".repeat(131)}leaf = 1\n`;
  expectAxsError(() => parse(source), { kind: "InvalidIndentation" });
});

test("enforces document size and token limits", () => {
  const huge = "x".repeat(8 * 1024 * 1024 + 1);
  expectAxsError(() => parse(`a = ${huge}`), { code: "AXS009", kind: "SyntaxError" });
  expectAxsError(() => parse(huge), { code: "AXS009" });
  expectAxsError(() => stringify({ a: huge }), { code: "AXS009", path: "$.a" });
  const nearLimit = "y".repeat(8 * 1024 * 1024 - 16);
  expectAxsError(() => stringify({ a: nearLimit, b: nearLimit }), { code: "AXS009" });

  const many = {};
  for (let index = 0; index < 300_000; index += 1) many[`k${index}`] = 1;
  expectAxsError(() => stringify(many), { code: "AXS009" });
  expectAxsError(() => stringify({ list: new Array(300_000).fill(1) }), { code: "AXS009" });
});

test("rejects cyclic references but accepts shared ones", () => {
  const cyclic = { name: "loop" };
  cyclic.self = cyclic;
  expectAxsError(() => stringify(cyclic), { code: "AXS004", kind: "InvalidValue", path: "$.self" });

  const list = [];
  list.push(list);
  expectAxsError(() => stringify({ list }), { path: "$.list[0]" });

  const shared = { x: 1 };
  assert.equal(stringify({ a: shared, b: shared }), "a:\n  x = 1\nb:\n  x = 1\n");
});

test("rejects unsupported JavaScript values with a path", () => {
  class Point {
    constructor() {
      this.x = 1;
    }
  }
  const cases = [
    [{ a: undefined }, "$.a"],
    [{ a: Symbol("s") }, "$.a"],
    [{ a: () => 1 }, "$.a"],
    [{ a: new Date(0) }, "$.a"],
    [{ a: new Map() }, "$.a"],
    [{ a: new Set() }, "$.a"],
    [{ a: new Point() }, "$.a"],
    [{ a: new String("boxed") }, "$.a"],
    [{ a: new Uint8Array(2) }, "$.a"],
    [{ a: /regex/ }, "$.a"],
    [{ a: [1, undefined] }, "$.a[1]"],
    // eslint-disable-next-line no-sparse-arrays
    [{ a: [1, , 3] }, "$.a[1]"],
    [{ nested: { "key with space": undefined } }, '$.nested["key with space"]']
  ];
  for (const [value, path] of cases) {
    expectAxsError(() => stringify(value), { code: "AXS004", kind: "InvalidValue", path });
  }
  expectAxsError(() => stringify({ [Symbol("k")]: 1 }), { code: "AXS003" });
});

test("rejects unsupported roots", () => {
  for (const value of [[], [1], "text", 1, 1n, true, null, undefined, new Date(0), new Map()]) {
    expectAxsError(() => stringify(value), { code: "AXS004", kind: "InvalidValue", path: "$" });
  }
  assert.throws(() => parse(42), TypeError);
  assert.throws(() => parse(undefined), TypeError);
});

test("accepts null-prototype objects and keeps __proto__ as data", () => {
  const bare = Object.create(null);
  bare.name = "bare";
  assert.equal(stringify(bare), "name = bare\n");

  const parsed = parse("__proto__:\n  polluted = true\nconstructor = x");
  assert.equal(Object.getPrototypeOf(parsed), Object.prototype);
  assert.deepEqual(Object.keys(parsed), ["__proto__", "constructor"]);
  assert.equal(parsed.polluted, undefined);
  assert.equal({}.polluted, undefined);
  assert.equal(stringify(parsed), "__proto__:\n  polluted = true\nconstructor = x\n");
});

test("rejects unpaired surrogates instead of replacing them", () => {
  expectAxsError(() => parse("a = \uD800"), { code: "AXS007", kind: "InvalidString" });
  expectAxsError(() => stringify({ a: "x\uDC00" }), { code: "AXS007", path: "$.a" });
  assert.equal(parse(stringify({ a: "\uD83D\uDE80" })).a, "🚀");
});

test("returns keys in sorted, deterministic order", () => {
  assert.deepEqual(Object.keys(parse("z = 1\na = 2\nm = 3")), ["a", "m", "z"]);
  assert.equal(stringify({ z: 1, a: 2 }), stringify({ a: 2, z: 1 }));
});

test("keeps working after errors and repeated calls", async () => {
  for (let round = 0; round < 50; round += 1) {
    assert.throws(() => parse("bad"), AxsError);
    assert.throws(() => stringify({ a: undefined }), AxsError);
    assert.deepEqual(parse(stringify({ round })), { round });
  }
  await init();
  await Promise.all([init(), init()]);
  assert.deepEqual(parse("a = 1"), { a: 1 });
});

test("does not call back into stringify for getter exceptions", () => {
  const value = {
    get broken() {
      throw new RangeError("getter failed");
    }
  };
  assert.throws(() => stringify(value), RangeError);
  assert.deepEqual(parse(stringify({ ok: true })), { ok: true });
});
