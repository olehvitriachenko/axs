import test from "node:test";
import assert from "node:assert/strict";
import { AxsError } from "@axs-lang/core";
import { axsContent, axsResult } from "../dist/index.js";

test("axsContent writes objects as documents and wraps other values", () => {
  assert.deepEqual(axsContent({ a: 1, b: [1, 2] }), { type: "text", text: "a: 1\nb[2]: [1,2]\n" });
  assert.equal(axsContent([{ id: 1 }, { id: 2 }]).text, "result[2]{id}:\n  1\n  2\n");
  assert.equal(axsContent("hi").text, "result: hi\n");
});

test("axsContent applies a query first; arrays are addressed under result", () => {
  const users = { users: [{ name: "Oleh", age: 22 }, { name: "Alex", age: 25 }] };
  assert.equal(axsContent(users, { query: "users[age > 22].name" }).text, "result[1]: [Alex]\n");
  assert.equal(axsContent([{ id: 1 }, { id: 2 }], { query: "result[-1]" }).text, "id: 2\n");
});

test("axsResult returns a complete tool result", () => {
  assert.deepEqual(axsResult({ ok: true }), { content: [{ type: "text", text: "ok: true\n" }] });
});

test("errors are AxsErrors", () => {
  assert.throws(() => axsContent({ a: 1 }, { query: "b" }), (e) => e instanceof AxsError && e.code === "AXS012");
  assert.throws(() => axsContent({ id: 2 ** 60 }), AxsError);
  assert.equal(axsContent({ id: 2n ** 60n }).text, "id: 1152921504606846976\n");
});
