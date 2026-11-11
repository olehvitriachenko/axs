/**
 * Pure transformations used by the proxy: how a tool listing gains the
 * `axs_query` argument, and how a tool result becomes compact AXS.
 *
 * Nothing here talks to MCP or to a server, so it can be tested directly.
 */
import { AxsError, query as runQuery, stringify } from "@axs-lang/core";

/** Name of the optional argument the proxy adds to every tool. */
export const QUERY_ARGUMENT = "axs_query";

const QUERY_DESCRIPTION =
  "Optional AXS query selecting part of this tool's result, e.g. " +
  "items[state == open]{number,title}. Steps: .field  [N]  [*]  " +
  "[field > 5 and name ~ text]  {a,b}.";

/** A JSON-Schema-ish object; only the parts the proxy touches are typed. */
export interface JsonSchema {
  type?: string | string[];
  properties?: Record<string, unknown>;
  [key: string]: unknown;
}

/** The fields of an MCP tool definition that the proxy reads or changes. */
export interface ToolLike {
  name: string;
  inputSchema?: JsonSchema;
  outputSchema?: unknown;
  [key: string]: unknown;
}

/** A content block of a tool result. */
export interface ContentBlock {
  type: string;
  text?: string;
  [key: string]: unknown;
}

/** The fields of an MCP tool result that the proxy reads or changes. */
export interface ResultLike {
  content?: ContentBlock[];
  structuredContent?: unknown;
  isError?: boolean;
  [key: string]: unknown;
}

/** Per-tool static rule from the proxy configuration. */
export interface ToolRule {
  /** Query applied when the model does not pass `axs_query`. */
  query?: string;
}

export interface TransformOptions {
  /** Query passed by the model, already removed from the tool arguments. */
  query?: string | undefined;
  /** Static rule for this tool. */
  rule?: ToolRule | undefined;
}

/**
 * Adds the optional `axs_query` argument to a tool's input schema and drops
 * `outputSchema`, because proxied results are rewritten as AXS text and no
 * longer match the server's structured output.
 *
 * Tools whose schema is not an object schema, or that already define
 * `axs_query`, are left alone apart from `outputSchema`.
 */
export function transformTool<T extends ToolLike>(tool: T, addQueryArgument = true): T {
  const { outputSchema: _dropped, ...rest } = tool;
  const result = rest as unknown as T;
  const schema = tool.inputSchema;
  if (!addQueryArgument || !schema || (schema.type !== undefined && schema.type !== "object")) {
    return result;
  }
  if (schema.properties && QUERY_ARGUMENT in schema.properties) {
    return result;
  }
  return {
    ...result,
    inputSchema: {
      ...schema,
      type: "object",
      properties: {
        ...schema.properties,
        [QUERY_ARGUMENT]: { type: "string", description: QUERY_DESCRIPTION }
      }
    }
  };
}

/** Removes `axs_query` from call arguments and returns it separately. */
export function splitArguments(args: Record<string, unknown> | undefined): {
  args: Record<string, unknown> | undefined;
  query: string | undefined;
  invalid: boolean;
} {
  if (!args || !(QUERY_ARGUMENT in args)) {
    return { args, query: undefined, invalid: false };
  }
  const { [QUERY_ARGUMENT]: value, ...rest } = args;
  if (value === undefined || value === null || value === "") {
    return { args: rest, query: undefined, invalid: false };
  }
  if (typeof value !== "string") {
    return { args: rest, query: undefined, invalid: true };
  }
  return { args: rest, query: value, invalid: false };
}

/** An MCP error result carrying a message the model can act on. */
export function errorResult(message: string): ResultLike {
  return { isError: true, content: [{ type: "text", text: message }] };
}

function describe(error: unknown): string {
  if (error instanceof AxsError) {
    return error.help ? `${error.message}\n${error.help}` : error.message;
  }
  return error instanceof Error ? error.message : String(error);
}

function isStructured(value: unknown): value is object {
  return typeof value === "object" && value !== null;
}

/** Writes a value as AXS; values that are not objects are wrapped in `result`. */
export function toAxs(value: unknown): string {
  const document = isStructured(value) && !Array.isArray(value) ? value : { result: value };
  return stringify(document);
}

/**
 * Converts one piece of structured data to AXS text, applying the query first.
 * Returns `undefined` when the data cannot be represented exactly (for
 * example integers beyond 2^53, which JSON.parse has already rounded), so the
 * caller keeps the original text instead of changing the data.
 */
function convert(data: object, query: string | undefined): string | undefined {
  const selected = query === undefined ? data : runQuery(data, query);
  try {
    return toAxs(selected);
  } catch (error) {
    if (error instanceof AxsError && query === undefined) return undefined;
    if (error instanceof AxsError && error.kind !== "InvalidQuery") return undefined;
    throw error;
  }
}

/**
 * Rewrites a tool result as AXS.
 *
 * - Text blocks holding a JSON object or array are replaced by AXS text.
 *   `structuredContent`, when present, is the source instead, and is removed.
 * - The query (the model's `axs_query`, else the rule's) is applied to each
 *   converted value. An invalid query produces an error result that names the
 *   problem and the available fields, so the model can correct it.
 * - Anything else (images, plain text, error results) passes through
 *   unchanged. Data that cannot be converted exactly is also left unchanged.
 *
 * JSON cannot tell `1.0` from `1`, so a float written with a zero fraction
 * comes back as an integer.
 */
export function transformResult(result: ResultLike, options: TransformOptions = {}): ResultLike {
  if (result.isError) return result;
  const query = options.query ?? options.rule?.query;
  const content = result.content ?? [];
  const { structuredContent, ...rest } = result;

  try {
    if (isStructured(structuredContent)) {
      const text = convert(structuredContent, query);
      if (text !== undefined) {
        return { ...rest, content: [{ type: "text", text }] };
      }
    }

    let converted = 0;
    const blocks = content.map((block): ContentBlock => {
      if (block.type !== "text" || typeof block.text !== "string") return block;
      let data: unknown;
      try {
        data = JSON.parse(block.text);
      } catch {
        return block;
      }
      if (!isStructured(data)) return block;
      const text = convert(data, query);
      if (text === undefined) return block;
      converted += 1;
      return { ...block, text };
    });

    if (query !== undefined && converted === 0 && options.query !== undefined) {
      blocks.push({
        type: "text",
        text: "axs_query was ignored: the result is not structured data."
      });
    }
    return { ...(structuredContent === undefined ? result : rest), content: blocks };
  } catch (error) {
    if (error instanceof AxsError) return errorResult(describe(error));
    throw error;
  }
}
