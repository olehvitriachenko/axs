/**
 * The proxy: an MCP server (for the model's client) in front of any other MCP
 * server (the upstream). Tool listings gain an optional `axs_query` argument;
 * tool results come back as compact AXS. Resources and prompts are forwarded
 * untouched.
 */
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import type { Transport } from "@modelcontextprotocol/sdk/shared/transport.js";
import {
  CallToolRequestSchema,
  GetPromptRequestSchema,
  ListPromptsRequestSchema,
  ListResourceTemplatesRequestSchema,
  ListResourcesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema
} from "@modelcontextprotocol/sdk/types.js";
import {
  type ResultLike,
  type ToolLike,
  type ToolRule,
  errorResult,
  splitArguments,
  transformResult,
  transformTool
} from "./transform.js";

export interface ProxyConfig {
  /** Add the optional `axs_query` argument to every tool. Default: true. */
  queryArgument?: boolean;
  /** Static rules by tool name, applied when the model passes no `axs_query`. */
  tools?: Record<string, ToolRule>;
}

export interface UpstreamCommand {
  command: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
}

export interface ProxyOptions {
  /** How to start the upstream server (stdio), or an existing transport. */
  upstream: UpstreamCommand | Transport;
  /** Transport towards the model's client, usually stdio. */
  downstream: Transport;
  config?: ProxyConfig;
  /** Name reported to the client. */
  name?: string;
  version?: string;
}

export interface RunningProxy {
  close(): Promise<void>;
}

function isCommand(value: UpstreamCommand | Transport): value is UpstreamCommand {
  return typeof (value as UpstreamCommand).command === "string";
}

/** Starts the proxy; resolves once both sides are connected. */
export async function startProxy(options: ProxyOptions): Promise<RunningProxy> {
  const config = options.config ?? {};
  const addQueryArgument = config.queryArgument !== false;

  const upstream = new Client({ name: options.name ?? "axs-mcp", version: options.version ?? "0.1.0" });
  const transport = isCommand(options.upstream)
    ? new StdioClientTransport({
        command: options.upstream.command,
        args: options.upstream.args ?? [],
        ...(options.upstream.env ? { env: { ...process.env, ...options.upstream.env } as Record<string, string> } : {}),
        ...(options.upstream.cwd ? { cwd: options.upstream.cwd } : {}),
        stderr: "inherit"
      })
    : options.upstream;
  await upstream.connect(transport);

  const capabilities = upstream.getServerCapabilities() ?? {};
  const info = upstream.getServerVersion();
  const instructions = upstream.getInstructions();

  const server = new Server(
    {
      name: info?.name ?? "axs-mcp",
      version: info?.version ?? options.version ?? "0.1.0"
    },
    {
      capabilities: {
        ...(capabilities.tools ? { tools: {} } : {}),
        ...(capabilities.resources ? { resources: {} } : {}),
        ...(capabilities.prompts ? { prompts: {} } : {})
      },
      ...(instructions ? { instructions } : {})
    }
  );

  if (capabilities.tools) {
    server.setRequestHandler(ListToolsRequestSchema, async (request) => {
      const page = await upstream.listTools(request.params);
      return {
        ...page,
        tools: page.tools.map((tool) => transformTool(tool as ToolLike, addQueryArgument))
      } as never;
    });

    server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name } = request.params;
      const split = splitArguments(request.params.arguments);
      if (split.invalid) {
        return errorResult(`${"axs_query"} must be a string`) as never;
      }
      const result = (await upstream.callTool({
        name,
        ...(split.args === undefined ? {} : { arguments: split.args })
      })) as ResultLike;
      return transformResult(result, { query: split.query, rule: config.tools?.[name] }) as never;
    });
  }

  if (capabilities.resources) {
    server.setRequestHandler(ListResourcesRequestSchema, (request) =>
      upstream.listResources(request.params)
    );
    server.setRequestHandler(ListResourceTemplatesRequestSchema, (request) =>
      upstream.listResourceTemplates(request.params)
    );
    server.setRequestHandler(ReadResourceRequestSchema, (request) =>
      upstream.readResource(request.params)
    );
  }

  if (capabilities.prompts) {
    server.setRequestHandler(ListPromptsRequestSchema, (request) =>
      upstream.listPrompts(request.params)
    );
    server.setRequestHandler(GetPromptRequestSchema, (request) =>
      upstream.getPrompt(request.params)
    );
  }

  // If either side goes away, the proxy has nothing left to do.
  upstream.onclose = () => void server.close();
  server.onclose = () => void upstream.close();

  await server.connect(options.downstream);

  return {
    async close() {
      await server.close();
      await upstream.close();
    }
  };
}
