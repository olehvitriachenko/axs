// A NestJS-style configuration loader built on @axs-lang/core.
//
// `loadAxsConfig` returns a factory with the shape expected by
// `ConfigModule.forRoot({ load: [...] })` from @nestjs/config, so it can be
// used there unchanged. This example does not depend on NestJS itself.
import { readFileSync } from "node:fs";
import { AxsError, parse, type AxsObject, type AxsValue } from "@axs-lang/core";

export interface AppConfig {
  server: { port: number; enabled: boolean; skills: string[] };
  limits: { maxUploadBytes: bigint };
}

function fail(path: string, expected: string, value: AxsValue | undefined): never {
  throw new Error(`Invalid configuration: ${path} must be ${expected}, got ${String(value)}`);
}

function object(value: AxsValue | undefined, path: string): AxsObject {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(path, "an object", value);
  }
  return value;
}

/** Validates the untyped parse result into the application's config type. */
export function toAppConfig(document: AxsObject): AppConfig {
  const server = object(document.server, "server");
  const limits = object(document.limits, "limits");
  const { port, enabled, skills } = server;
  if (typeof port !== "number" || !Number.isInteger(port)) fail("server.port", "an integer", port);
  if (typeof enabled !== "boolean") fail("server.enabled", "a boolean", enabled);
  if (!Array.isArray(skills) || !skills.every((skill) => typeof skill === "string")) {
    fail("server.skills", "a list of strings", skills);
  }
  // Integers are numbers inside the safe range and bigints outside it.
  const max = limits.maxUploadBytes;
  if (typeof max !== "bigint" && typeof max !== "number") {
    fail("limits.maxUploadBytes", "an integer", max);
  }
  return {
    server: { port, enabled, skills: skills as string[] },
    limits: { maxUploadBytes: BigInt(max) }
  };
}

/** Factory for `ConfigModule.forRoot({ load: [loadAxsConfig(path)] })`. */
export function loadAxsConfig(path: string): () => AppConfig {
  return () => {
    const source = readFileSync(path, "utf8");
    try {
      return toAppConfig(parse(source));
    } catch (error) {
      if (error instanceof AxsError) {
        throw new Error(`${path}:${error.line ?? "?"}:${error.column ?? "?"}: ${error.message}`, {
          cause: error
        });
      }
      throw error;
    }
  };
}
