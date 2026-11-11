#!/usr/bin/env node
/**
 * `axs-mcp`: run an MCP server through the AXS proxy.
 *
 *   axs-mcp [options] -- <command> [args...]
 *
 * stdout carries only MCP protocol messages; diagnostics go to stderr.
 */
import { readFileSync } from "node:fs";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { type ProxyConfig, startProxy } from "./proxy.js";

const USAGE = `Usage: axs-mcp [options] -- <command> [args...]

Runs an MCP server and returns its tool results as compact AXS instead of
JSON. Every tool also accepts an optional "axs_query" argument that selects
part of the result, e.g. items[state == open]{number,title}.

Options:
  --config <file>        JSON config: {"queryArgument": true,
                         "tools": {"tool_name": {"query": "<AXS query>"}}}
  --rule <tool>=<query>  Query applied to a tool's result when the model passes
                         none (repeatable)
  --no-query-arg         Do not add the "axs_query" argument to tools
  -h, --help             Show this help
  -v, --version          Show the version
`;

class UsageError extends Error {}

interface Parsed {
  config: ProxyConfig;
  command: string;
  args: string[];
}

function parseConfigFile(path: string): ProxyConfig {
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new UsageError(`Cannot read config ${path}: ${(error as Error).message}`);
  }
  const fail = (message: string): never => {
    throw new UsageError(`Invalid config ${path}: ${message}`);
  };
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("expected an object");
  const object = raw as Record<string, unknown>;
  const config: ProxyConfig = {};
  if (object.queryArgument !== undefined) {
    if (typeof object.queryArgument !== "boolean") fail('"queryArgument" must be a boolean');
    config.queryArgument = object.queryArgument as boolean;
  }
  if (object.tools !== undefined) {
    const tools = object.tools;
    if (typeof tools !== "object" || tools === null || Array.isArray(tools)) {
      fail('"tools" must be an object');
    }
    config.tools = {};
    for (const [name, rule] of Object.entries(tools as Record<string, unknown>)) {
      const query = (rule as { query?: unknown } | null)?.query;
      if (typeof rule !== "object" || rule === null || typeof query !== "string") {
        fail(`tool "${name}" needs a string "query"`);
      }
      config.tools[name] = { query: query as string };
    }
  }
  return config;
}

export function parseArguments(argv: string[]): Parsed | "help" | "version" {
  const config: ProxyConfig = {};
  const rules: Record<string, { query: string }> = {};
  let index = 0;
  for (; index < argv.length; index += 1) {
    const argument = argv[index] as string;
    if (argument === "--") {
      index += 1;
      break;
    }
    if (argument === "-h" || argument === "--help") return "help";
    if (argument === "-v" || argument === "--version") return "version";
    if (argument === "--no-query-arg") {
      config.queryArgument = false;
    } else if (argument === "--config" || argument === "--rule") {
      const value = argv[index + 1];
      if (value === undefined) throw new UsageError(`${argument} needs a value`);
      index += 1;
      if (argument === "--config") {
        const loaded = parseConfigFile(value);
        Object.assign(config, loaded, {
          tools: { ...config.tools, ...loaded.tools }
        });
      } else {
        const split = value.indexOf("=");
        if (split <= 0 || split === value.length - 1) {
          throw new UsageError(`--rule expects <tool>=<query>, got "${value}"`);
        }
        rules[value.slice(0, split)] = { query: value.slice(split + 1) };
      }
    } else {
      throw new UsageError(`Unknown option ${argument}`);
    }
  }
  const [command, ...args] = argv.slice(index);
  if (command === undefined) throw new UsageError("Missing the upstream command after --");
  if (Object.keys(rules).length > 0) config.tools = { ...config.tools, ...rules };
  return { config, command, args };
}

async function main(): Promise<void> {
  let parsed: ReturnType<typeof parseArguments>;
  try {
    parsed = parseArguments(process.argv.slice(2));
  } catch (error) {
    if (error instanceof UsageError) {
      process.stderr.write(`axs-mcp: ${error.message}\n\n${USAGE}`);
      process.exitCode = 2;
      return;
    }
    throw error;
  }
  if (parsed === "help") {
    process.stdout.write(USAGE);
    return;
  }
  if (parsed === "version") {
    const manifest = JSON.parse(
      readFileSync(new URL("../package.json", import.meta.url), "utf8")
    ) as { version: string };
    process.stdout.write(`${manifest.version}\n`);
    return;
  }

  const proxy = await startProxy({
    upstream: { command: parsed.command, args: parsed.args },
    downstream: new StdioServerTransport(),
    config: parsed.config
  });
  const stop = () => void proxy.close().finally(() => process.exit(0));
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
}

main().catch((error: unknown) => {
  process.stderr.write(`axs-mcp: ${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
