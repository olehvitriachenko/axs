// End to end: a real MCP client talks to the real `axs-mcp` process, which
// runs the fake upstream server as a child process.
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const cli = fileURLToPath(new URL("../dist/cli.js", import.meta.url));
const upstream = fileURLToPath(new URL("./fixtures/upstream.mjs", import.meta.url));

async function connect(flags = []) {
  const client = new Client({ name: "test-client", version: "1.0.0" });
  await client.connect(
    new StdioClientTransport({
      command: process.execPath,
      args: [cli, ...flags, "--", process.execPath, upstream]
    })
  );
  return client;
}

const text = (result) => result.content.map((block) => block.text).join("\n");

test("the proxy mirrors the upstream's identity, instructions, and capabilities", async () => {
  const client = await connect();
  try {
    assert.equal(client.getServerVersion().name, "fake-upstream");
    assert.equal(client.getInstructions(), "Fake upstream for tests");
    const capabilities = client.getServerCapabilities();
    assert.ok(capabilities.tools && capabilities.resources);
    assert.equal(capabilities.prompts, undefined);
  } finally {
    await client.close();
  }
});

test("tool listings gain axs_query and lose outputSchema", async () => {
  const client = await connect();
  try {
    const { tools } = await client.listTools();
    const byName = Object.fromEntries(tools.map((tool) => [tool.name, tool]));
    assert.equal(byName.list_issues.inputSchema.properties.axs_query.type, "string");
    assert.equal(byName.list_issues.inputSchema.properties.state.type, "string");
    assert.equal(byName.get_stats.outputSchema, undefined);
    assert.equal(byName.echo_args.inputSchema.additionalProperties, false);
    assert.equal(byName.no_properties.inputSchema.properties.axs_query.type, "string");
  } finally {
    await client.close();
  }
});

test("results come back as AXS and axs_query selects part of them", async () => {
  const client = await connect();
  try {
    const full = await client.callTool({ name: "list_issues", arguments: {} });
    assert.equal(
      text(full),
      "repository: axs-lang/axs\n" +
        "items[3]:\n" +
        "  - number: 1\n    title: Crash on start\n    state: open\n    comments: 3\n    labels[1]{name}:\n      bug\n" +
        "  - number: 2\n    title: Add tables\n    state: closed\n    comments: 9\n    labels[1]{name}:\n      enhancement\n" +
        "  - number: 3\n    title: Fix parser: edge case\n    state: open\n    comments: 0\n    labels[2]{name}:\n      bug\n      docs\n"
    );
    const open = await client.callTool({
      name: "list_issues",
      arguments: { axs_query: "items[state == open and labels.name ~ bug]{number,title}" }
    });
    assert.equal(
      text(open),
      "result[2]{number,title}:\n  1,Crash on start\n  3,Fix parser: edge case\n"
    );
    assert.ok(text(open).length < text(full).length / 3);
  } finally {
    await client.close();
  }
});

test("axs_query is removed before the upstream sees the arguments", async () => {
  const client = await connect();
  try {
    // The tool forbids additional properties, so a leak would be visible.
    const result = await client.callTool({
      name: "echo_args",
      arguments: { a: 1, axs_query: "a" }
    });
    assert.equal(text(result), "result: 1\n");
    const plain = await client.callTool({ name: "echo_args", arguments: { a: 2 } });
    assert.equal(text(plain), "a: 2\n");
  } finally {
    await client.close();
  }
});

test("structured output, arrays, plain text, and errors", async () => {
  const client = await connect();
  try {
    const stats = await client.callTool({ name: "get_stats", arguments: {} });
    assert.equal(text(stats), "total: 3\n");
    assert.equal(stats.structuredContent, undefined);
    assert.equal(text(await client.callTool({ name: "list_array", arguments: {} })), "result[2]{id,name}:\n  1,a\n  2,b\n");
    assert.equal(text(await client.callTool({ name: "say_hello", arguments: {} })), "hello world");
    const failed = await client.callTool({ name: "fail", arguments: {} });
    assert.equal(failed.isError, true);
    assert.equal(text(failed), '{"error":"boom"}');
    const big = await client.callTool({ name: "big_number", arguments: {} });
    assert.equal(text(big), '{"id":9007199254740993,"name":"x"}', "unsafe integers are left as JSON");
  } finally {
    await client.close();
  }
});

test("bad queries are errors the model can act on", async () => {
  const client = await connect();
  try {
    const result = await client.callTool({
      name: "list_issues",
      arguments: { axs_query: "items[0].nmae" }
    });
    assert.equal(result.isError, true);
    assert.match(text(result), /AXS012: No field 'nmae'/);
    assert.match(text(result), /Available fields: number, title, state, comments, labels/);
    const wrongType = await client.callTool({ name: "list_issues", arguments: { axs_query: 5 } });
    assert.equal(wrongType.isError, true);
    assert.match(text(wrongType), /must be a string/);
  } finally {
    await client.close();
  }
});

test("--rule and --config apply a default query per tool", async () => {
  const directory = mkdtempSync(join(tmpdir(), "axs-mcp-"));
  const config = join(directory, "axs-mcp.json");
  writeFileSync(config, JSON.stringify({ tools: { list_issues: { query: "items.number" } } }));
  const fromConfig = await connect(["--config", config]);
  try {
    assert.equal(text(await fromConfig.callTool({ name: "list_issues", arguments: {} })), "result[3]: [1,2,3]\n");
    // The model's own query wins over the rule.
    const own = await fromConfig.callTool({ name: "list_issues", arguments: { axs_query: "items.state" } });
    assert.equal(text(own), "result[3]: [open,closed,open]\n");
  } finally {
    await fromConfig.close();
  }
  const fromRule = await connect(["--rule", "list_issues=repository"]);
  try {
    assert.equal(text(await fromRule.callTool({ name: "list_issues", arguments: {} })), "result: axs-lang/axs\n");
  } finally {
    await fromRule.close();
  }
});

test("--no-query-arg leaves tool schemas alone but still converts results", async () => {
  const client = await connect(["--no-query-arg"]);
  try {
    const { tools } = await client.listTools();
    assert.equal(tools.find((tool) => tool.name === "list_issues").inputSchema.properties.axs_query, undefined);
    assert.match(text(await client.callTool({ name: "get_stats", arguments: {} })), /^total: 3/);
  } finally {
    await client.close();
  }
});

test("resources are forwarded untouched", async () => {
  const client = await connect();
  try {
    const { resources } = await client.listResources();
    assert.equal(resources[0].uri, "mem://note");
    const read = await client.readResource({ uri: "mem://note" });
    assert.equal(read.contents[0].text, '{"kept":"as is"}');
  } finally {
    await client.close();
  }
});

test("command line errors exit with status 2 and keep stdout clean", () => {
  const run = (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: "utf8" });
  for (const args of [[], ["--rule", "nope", "--", "x"], ["--bogus"], ["--config"], ["--config", "/no/such/file", "--", "x"]]) {
    const result = run(...args);
    assert.equal(result.status, 2, JSON.stringify(args));
    assert.equal(result.stdout, "");
    assert.match(result.stderr, /Usage: axs-mcp/);
  }
  const help = run("--help");
  assert.equal(help.status, 0);
  assert.match(help.stdout, /axs_query/);
  assert.match(run("--version").stdout, /^\d+\.\d+\.\d+\n$/);
});

test("a missing upstream command fails with a message, not a hang", () => {
  const result = spawnSync(process.execPath, [cli, "--", "definitely-not-a-command-axs"], {
    encoding: "utf8",
    timeout: 20000
  });
  assert.notEqual(result.status, 0);
  assert.equal(result.stdout, "");
  assert.ok(result.stderr.length > 0);
});
