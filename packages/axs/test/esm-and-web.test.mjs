// Runtime tests for ESM consumers in Node.js and for the web entry point.
// The web build is exercised in Node by supplying the WebAssembly bytes
// explicitly; fetching it by URL is covered by the browser example.

import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";

import * as nodeEntry from "@axs-lang/core";
import * as web from "@axs-lang/core/web";

const require = createRequire(import.meta.url);
const wasmPath = require.resolve("@axs-lang/core/axs_wasm_bg.wasm");

test("ESM named imports resolve to the Node.js build", () => {
  assert.equal(typeof nodeEntry.parse, "function");
  assert.equal(typeof nodeEntry.stringify, "function");
  assert.deepEqual(nodeEntry.parse(nodeEntry.stringify({ id: 2n ** 63n - 1n })), {
    id: 2n ** 63n - 1n
  });
});

test("web build requires initialization and supports retry and repeated init", async () => {
  assert.throws(() => web.parse("a = 1"), /not initialized/);
  assert.throws(() => web.stringify({ a: 1 }), /not initialized/);

  await assert.rejects(web.init(new Uint8Array([0, 1, 2, 3])));
  assert.throws(() => web.parse("a = 1"), /not initialized/);

  const bytes = await readFile(wasmPath);
  await Promise.all([web.init(bytes), web.init(bytes), web.init()]);
  await web.init(bytes);
  web.initSync(bytes);

  const original = { id: 9223372036854775807n, name: "Oleh", enabled: true };
  const decoded = web.parse(web.stringify(original));
  assert.deepEqual(decoded, original);
  assert.equal(typeof decoded.id, "bigint");
  assert.throws(() => web.parse("a = 1\na = 2"), web.AxsError);
});
