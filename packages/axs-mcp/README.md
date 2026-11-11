# @axs-lang/mcp

Return [MCP](https://modelcontextprotocol.io) tool results as compact
[AXS](../../README.md) instead of JSON, and let the model ask for only the part
of a result it needs.

AXS is a payload format, not a replacement for MCP: the protocol stays
JSON-RPC, and the text inside a tool result is what reaches the model's
context.

Not published yet. In this repository it depends on the local
`@axs-lang/core` (`file:../axs`), which must be built first
(`npm run build` in `packages/axs`).

## The proxy

`axs-mcp` runs any stdio MCP server and sits between it and the client:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["@axs-lang/mcp", "--", "npx", "-y", "@modelcontextprotocol/server-github"]
    }
  }
}
```

(Until the package is published, use `node /path/to/packages/axs-mcp/dist/cli.js`
in place of `npx @axs-lang/mcp`.)

What the proxy changes:

- **Results as AXS.** A tool result whose text is a JSON object or array (or
  whose `structuredContent` is present) is rewritten as AXS. Arrays of uniform
  objects become tables, which is where most of the savings are.
- **An optional `axs_query` argument on every tool.** The model can pass an
  [AXS query](../../spec/QUERY.md) such as
  `items[state == open and labels.name ~ bug]{number,title}`; the proxy removes
  the argument, calls the real tool, and returns only the selected part. A bad
  query comes back as an error that names the problem and the available fields,
  so the model can correct it in the next call.
- **Static default queries** per tool, for results you always want trimmed.

Everything else passes through unchanged: plain-text and image results, error
results (`isError`), and data that AXS cannot represent exactly. Resources and
prompts are forwarded untouched. The upstream's name, version, instructions,
and capabilities are mirrored.

### Measured effect

On the token benchmark's GitHub issues dataset (40 issues, `o200k_base`), as
returned by a server that pretty-prints its JSON:

| Result | Tokens | vs server output |
|---|---:|---:|
| Server output (JSON, indent 2) | 5,767 | |
| Through the proxy | 4,447 | −22.9% |
| Proxy with `axs_query: items[state == open]{number,title,comments}` | 310 | −94.6% |

The conversion alone only pays off against pretty-printed JSON or tabular
data. Minified JSON of this nested dataset is 3,937 tokens, so the 4,447-token
AXS form is 13% *larger* (see the [token benchmark](../../bench/tokens/README.md):
AXS wins on arrays of uniform objects, not on deeply nested objects). The query
saving does not depend on how the server formats its JSON.

### Options

```text
axs-mcp [options] -- <command> [args...]

  --config <file>        JSON config (below)
  --rule <tool>=<query>  Default query for a tool when the model passes none
                         (repeatable)
  --no-query-arg         Do not add the "axs_query" argument to tools
```

```json
{
  "queryArgument": true,
  "tools": {
    "list_issues": { "query": "items[state == open]{number,title}" }
  }
}
```

A model-supplied `axs_query` wins over the tool's default query.

### Limits to know about

- **Only stdio upstreams** are supported, and only tools are transformed.
  `list_changed` notifications and sampling/elicitation requests from the
  upstream are not forwarded.
- **`outputSchema` is removed** from proxied tools and `structuredContent` from
  their results, because results are rewritten as text that no longer matches
  the server's schema.
- **JSON cannot tell `1.0` from `1`**, so a float written with a zero fraction
  arrives as an integer. Integers beyond 2^53 are already rounded by JSON
  parsing; such results are left as the server sent them.
- A tool that declares its own `axs_query` argument is left alone.
- The `axs_query` description added to each tool costs 60 tokens per tool in
  the tool list (measured, `o200k_base`). `--no-query-arg` removes it if you only want the conversion.

## For server authors

Return AXS directly, without the proxy:

```ts
import { axsContent, axsResult } from "@axs-lang/mcp";

// as a content block
const block = axsContent({ users }, { query: "users[age > 20]{name}" });

// or as a whole tool result
server.setRequestHandler(CallToolRequestSchema, async () => axsResult(data));
```

Objects are written as documents; arrays and primitives are wrapped under
`result`, which is also how queries address them. Invalid queries and values AXS
cannot represent (integer `number`s beyond 2^53; use `bigint`) throw `AxsError`.

## Development

```bash
npm install
npm test        # builds, then runs the unit and end-to-end tests
```

The end-to-end tests start the real `axs-mcp` process in front of a fake
upstream server and talk to it with the official MCP client.
