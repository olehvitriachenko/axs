#!/usr/bin/env node
// Bundles the extension with esbuild into dist/extension.js and copies the AXS
// WebAssembly core next to it. The bundled WebAssembly loader reads
// `${__dirname}/axs_wasm_bg.wasm`, so the extension never depends on the
// repository layout or on node_modules at runtime.
//
// Usage: node scripts/build.mjs [--watch]

import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const require = createRequire(import.meta.url);
const watch = process.argv.includes("--watch");

let wasm;
try {
  wasm = require.resolve("@axs-lang/core/axs_wasm_bg.wasm");
} catch {
  wasm = undefined;
}
if (wasm === undefined || !existsSync(wasm)) {
  console.error(
    "error: @axs-lang/core is not built. Run `npm ci && npm run build` in packages/axs first."
  );
  process.exit(1);
}

const dist = join(root, "dist");
rmSync(dist, { recursive: true, force: true });
mkdirSync(dist, { recursive: true });
copyFileSync(wasm, join(dist, "axs_wasm_bg.wasm"));

const options = {
  entryPoints: [join(root, "src", "extension.ts")],
  outfile: join(dist, "extension.js"),
  bundle: true,
  platform: "node",
  format: "cjs",
  // VS Code 1.90 ships Node.js 20.
  target: "node20",
  external: ["vscode"],
  // Map @axs-lang/core to its Node.js (CommonJS, synchronous) build.
  conditions: ["node"],
  mainFields: ["main"],
  sourcemap: false,
  minify: false,
  legalComments: "inline",
  metafile: true,
  logLevel: "info"
};

if (watch) {
  const context = await esbuild.context(options);
  await context.watch();
} else {
  const result = await esbuild.build(options);
  const inputs = Object.keys(result.metafile.inputs);
  console.log(`bundled ${inputs.length} modules:\n  ${inputs.join("\n  ")}`);
  console.log(`copied ${wasm} -> dist/axs_wasm_bg.wasm`);
}
