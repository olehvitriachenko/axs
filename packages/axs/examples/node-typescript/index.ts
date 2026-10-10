// Node.js + TypeScript: read, validate, modify, and write an AXS file.
import { readFileSync } from "node:fs";
import { AxsError, parse, stringify, type AxsObject } from "@axs-lang/core";

const source = readFileSync(new URL("./config.axs", import.meta.url), "utf8");
const config = parse(source);

const server = config.server as AxsObject;
console.log(`port: ${server.port} (${typeof server.port})`);

// i64 values outside Number.MAX_SAFE_INTEGER arrive as bigint, exactly.
const limits = config.limits as AxsObject;
console.log(`maxUploadBytes: ${limits.maxUploadBytes} (${typeof limits.maxUploadBytes})`);

server.port = 8080;
console.log(stringify(config));

try {
  parse("server:\n  port = 1\n  port = 2\n");
} catch (error) {
  if (error instanceof AxsError) {
    console.log(`${error.code} ${error.kind} at ${error.line}:${error.column}`);
  } else {
    throw error;
  }
}
