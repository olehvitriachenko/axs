/**
 * `@axs-lang/mcp`: use AXS as the payload format of MCP tool results.
 *
 * - {@link startProxy} / the `axs-mcp` command put any MCP server behind an
 *   AXS-speaking proxy.
 * - {@link axsContent} and {@link axsResult} let a server author return AXS
 *   directly.
 */
import { query as runQuery } from "@axs-lang/core";
import { toAxs } from "./transform.js";

export { startProxy } from "./proxy.js";
export type { ProxyConfig, ProxyOptions, RunningProxy, UpstreamCommand } from "./proxy.js";
export { QUERY_ARGUMENT, transformResult, transformTool } from "./transform.js";
export type { ToolRule } from "./transform.js";

export interface AxsContentOptions {
  /** AXS query applied to `data` before it is written. */
  query?: string;
}

/** A text content block, as used in an MCP tool result. */
export interface TextContent {
  type: "text";
  text: string;
  [key: string]: unknown;
}

/** A minimal MCP tool result. */
export interface AxsToolResult {
  content: TextContent[];
  [key: string]: unknown;
}

/**
 * Writes data as AXS and returns it as an MCP text content block. Objects are
 * written as a document; arrays and primitives are wrapped under `result`.
 * With `options.query` only the selected part is written.
 *
 * @throws {AxsError} for invalid queries and for values AXS cannot represent
 * (for example integer numbers beyond 2^53; use `bigint`).
 */
export function axsContent(data: unknown, options: AxsContentOptions = {}): TextContent {
  const selected =
    options.query === undefined ? data : runQuery(wrap(data), options.query);
  return { type: "text", text: toAxs(selected) };
}

/** Like {@link axsContent}, but returns a complete tool result. */
export function axsResult(data: unknown, options: AxsContentOptions = {}): AxsToolResult {
  return { content: [axsContent(data, options)] };
}

function wrap(data: unknown): object {
  return typeof data === "object" && data !== null && !Array.isArray(data) ? data : { result: data };
}
