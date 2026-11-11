#!/usr/bin/env node
// Bundles test/*.test.ts with esbuild (the same way the extension is bundled,
// including @axs-lang/core and its WebAssembly loader) and runs them with the
// Node.js test runner. Tests never import the `vscode` module; editor behavior
// inside VS Code is covered by scripts/smoke-test.mjs.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const require = createRequire(import.meta.url);
const out = join(root, "out-test");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
copyFileSync(require.resolve("@axs-lang/core/axs_wasm_bg.wasm"), join(out, "axs_wasm_bg.wasm"));

const tests = readdirSync(join(root, "test")).filter((file) => file.endsWith(".test.ts"));
await esbuild.build({
  entryPoints: tests.map((file) => join(root, "test", file)),
  outdir: out,
  bundle: true,
  platform: "node",
  format: "cjs",
  target: "node20",
  conditions: ["node"],
  external: ["vscode", "vscode-textmate", "vscode-oniguruma"],
  logLevel: "warning"
});

execFileSync(
  process.execPath,
  ["--test", ...tests.map((file) => join(out, file.replace(/\.ts$/, ".js")))],
  { cwd: root, stdio: "inherit" }
);
