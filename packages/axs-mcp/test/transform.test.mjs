import test from "node:test";
import assert from "node:assert/strict";
import {
  QUERY_ARGUMENT,
  transformResult,
  transformTool
} from "../dist/index.js";
import { splitArguments } from "../dist/transform.js";

const json = (value) => ({ type: "text", text: JSON.stringify(value) });
const issues = {
  items: [
    { number: 1, state: "open", title: "a" },
    { number: 2, state: "closed", title: "b" }
  ]
};

test("tools gain an optional axs_query string and lose outputSchema", () => {
  const tool = {
    name: "t",
    description: "d",
    inputSchema: { type: "object", properties: { x: { type: "string" } }, required: ["x"] },
    outputSchema: { type: "object" }
  };
  const result = transformTool(tool);
  assert.equal(result.outputSchema, undefined);
  assert.equal(result.description, "d");
  assert.deepEqual(result.inputSchema.required, ["x"]);
  assert.equal(result.inputSchema.properties.x.type, "string");
  assert.equal(result.inputSchema.properties[QUERY_ARGUMENT].type, "string");
  assert.match(result.inputSchema.properties[QUERY_ARGUMENT].description, /AXS query/);
  assert.equal(tool.inputSchema.properties[QUERY_ARGUMENT], undefined, "input is not mutated");
});

test("tools without properties, with their own axs_query, or without an object schema are respected", () => {
  assert.ok(transformTool({ name: "t", inputSchema: { type: "object" } }).inputSchema.properties[QUERY_ARGUMENT]);
  const own = { name: "t", inputSchema: { type: "object", properties: { [QUERY_ARGUMENT]: { type: "number" } } } };
  assert.equal(transformTool(own).inputSchema.properties[QUERY_ARGUMENT].type, "number");
  const array = { name: "t", inputSchema: { type: "array" }, outputSchema: {} };
  assert.deepEqual(transformTool(array), { name: "t", inputSchema: { type: "array" } });
  assert.equal(transformTool({ name: "t", inputSchema: { type: "object" } }, false).inputSchema.properties, undefined);
});

test("axs_query is split from the arguments", () => {
  assert.deepEqual(splitArguments({ a: 1, axs_query: "x" }), { args: { a: 1 }, query: "x", invalid: false });
  assert.deepEqual(splitArguments({ a: 1, axs_query: "" }), { args: { a: 1 }, query: undefined, invalid: false });
  assert.deepEqual(splitArguments({ axs_query: 5 }), { args: {}, query: undefined, invalid: true });
  assert.deepEqual(splitArguments(undefined), { args: undefined, query: undefined, invalid: false });
  assert.deepEqual(splitArguments({ a: 1 }), { args: { a: 1 }, query: undefined, invalid: false });
});

test("JSON text results become AXS", () => {
  const result = transformResult({ content: [json(issues)] });
  assert.equal(
    result.content[0].text,
    "items[2]{number,state,title}:\n  1,open,a\n  2,closed,b\n"
  );
});

test("queries select part of the result; rules apply only without a model query", () => {
  const options = { rule: { query: "items[state == open].number" } };
  assert.equal(transformResult({ content: [json(issues)] }, options).content[0].text, "result[1]: [1]\n");
  const own = { ...options, query: "items.title" };
  assert.equal(transformResult({ content: [json(issues)] }, own).content[0].text, "result[2]: [a,b]\n");
});

test("structuredContent is the source and is removed", () => {
  const result = transformResult({
    content: [json({ stale: true })],
    structuredContent: { total: 3, ok: true }
  });
  assert.equal(result.structuredContent, undefined);
  assert.deepEqual(result.content, [{ type: "text", text: "total: 3\nok: true\n" }]);
});

test("arrays are wrapped under result; other content passes through", () => {
  const array = transformResult({ content: [json([{ id: 1 }, { id: 2 }])] });
  assert.equal(array.content[0].text, "result[2]{id}:\n  1\n  2\n");
  const mixed = { content: [{ type: "image", data: "AA==", mimeType: "image/png" }, { type: "text", text: "plain" }] };
  assert.deepEqual(transformResult(mixed), mixed);
  const scalar = { content: [{ type: "text", text: "42" }] };
  assert.deepEqual(transformResult(scalar), scalar);
});

test("error results and unrepresentable data are never changed", () => {
  const failed = { isError: true, content: [json({ error: "boom" })] };
  assert.deepEqual(transformResult(failed, { query: "error" }), failed);
  const big = { content: [{ type: "text", text: '{"id":9007199254740993}' }] };
  assert.deepEqual(transformResult(big), big);
});

test("an invalid query becomes an error result that names the problem", () => {
  const missing = transformResult({ content: [json(issues)] }, { query: "items[0].nmae" });
  assert.equal(missing.isError, true);
  assert.match(missing.content[0].text, /AXS012: No field 'nmae'/);
  assert.match(missing.content[0].text, /Available fields: number, state, title/);
  const syntax = transformResult({ content: [json(issues)] }, { query: "items[state = open]" });
  assert.equal(syntax.isError, true);
  assert.match(syntax.content[0].text, /comparison operator/);
});

test("a model query on non-JSON output is reported, not silently dropped", () => {
  const result = transformResult({ content: [{ type: "text", text: "plain" }] }, { query: "x" });
  assert.equal(result.content[0].text, "plain");
  assert.match(result.content[1].text, /axs_query was ignored/);
  const rule = transformResult({ content: [{ type: "text", text: "plain" }] }, { rule: { query: "x" } });
  assert.equal(rule.content.length, 1);
});
