// A small MCP server that stands in for any upstream server in the tests.
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListResourcesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema
} from "@modelcontextprotocol/sdk/types.js";

export const ISSUES = {
  repository: "axs-lang/axs",
  items: [
    { number: 1, title: "Crash on start", state: "open", comments: 3, labels: [{ name: "bug" }] },
    { number: 2, title: "Add tables", state: "closed", comments: 9, labels: [{ name: "enhancement" }] },
    { number: 3, title: "Fix parser: edge case", state: "open", comments: 0, labels: [{ name: "bug" }, { name: "docs" }] }
  ]
};

const tools = [
  {
    name: "list_issues",
    description: "List issues",
    inputSchema: { type: "object", properties: { state: { type: "string" } } }
  },
  {
    name: "get_stats",
    description: "Structured output",
    inputSchema: { type: "object", properties: {} },
    outputSchema: { type: "object", properties: { total: { type: "integer" } }, required: ["total"] }
  },
  {
    name: "echo_args",
    description: "Returns the arguments it received",
    inputSchema: {
      type: "object",
      properties: { a: { type: "number" } },
      additionalProperties: false
    }
  },
  { name: "say_hello", description: "Plain text", inputSchema: { type: "object", properties: {} } },
  { name: "fail", description: "Always fails", inputSchema: { type: "object", properties: {} } },
  { name: "big_number", description: "Unsafe integer", inputSchema: { type: "object", properties: {} } },
  { name: "list_array", description: "Root array", inputSchema: { type: "object", properties: {} } },
  { name: "no_properties", description: "Schema without properties", inputSchema: { type: "object" } }
];

const text = (value) => ({ type: "text", text: typeof value === "string" ? value : JSON.stringify(value) });

const server = new Server(
  { name: "fake-upstream", version: "9.9.9" },
  { capabilities: { tools: {}, resources: {} }, instructions: "Fake upstream for tests" }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools }));

server.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
  const args = params.arguments ?? {};
  switch (params.name) {
    case "list_issues":
      return { content: [text(ISSUES)] };
    case "get_stats":
      return { content: [text({ total: 3 })], structuredContent: { total: 3 } };
    case "echo_args":
      return { content: [text(args)] };
    case "say_hello":
      return { content: [text("hello world")] };
    case "fail":
      return { isError: true, content: [text({ error: "boom" })] };
    case "big_number":
      return { content: [text('{"id":9007199254740993,"name":"x"}')] };
    case "list_array":
      return { content: [text([{ id: 1, name: "a" }, { id: 2, name: "b" }])] };
    default:
      throw new Error(`Unknown tool ${params.name}`);
  }
});

server.setRequestHandler(ListResourcesRequestSchema, async () => ({
  resources: [{ uri: "mem://note", name: "note", mimeType: "application/json" }]
}));
server.setRequestHandler(ReadResourceRequestSchema, async ({ params }) => ({
  contents: [{ uri: params.uri, mimeType: "application/json", text: '{"kept":"as is"}' }]
}));

await server.connect(new StdioServerTransport());
