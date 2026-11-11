#!/usr/bin/env node
// Builds the @axs-lang/core package from the Rust sources:
//
//   1. cargo build of crates/axs-wasm for wasm32-unknown-unknown (wasm-release profile)
//   2. wasm-bindgen glue for Node.js (CommonJS, synchronous load) and the web (ESM, async init)
//   3. TypeScript compilation of the Node.js (CommonJS) and web (ESM) entry points
//
// Requirements (maintainers only; package consumers need none of these):
//   rustup target add wasm32-unknown-unknown
//   cargo install wasm-bindgen-cli --version <version locked in Cargo.lock> --locked
//
// The wasm-bindgen CLI version must equal the wasm-bindgen crate version in
// Cargo.lock, and the build fails otherwise.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = dirname(dirname(fileURLToPath(import.meta.url)));
const repository = dirname(dirname(packageDir));
const wasmDir = join(packageDir, "wasm");
const distDir = join(packageDir, "dist");

// rustup installs into ~/.cargo/bin, which non-login shells may not have on PATH.
const cargoBin = join(process.env.CARGO_HOME ?? join(homedir(), ".cargo"), "bin");
const env = { ...process.env };
if (existsSync(cargoBin)) {
  env.PATH = `${env.PATH ?? ""}${delimiter}${cargoBin}`;
}

function step(name) {
  console.log(`\n==> ${name}`);
}

function run(command, args, options = {}) {
  console.log(`    $ ${command} ${args.join(" ")}`);
  return execFileSync(command, args, {
    cwd: repository,
    env,
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
    encoding: "utf8",
    shell: process.platform === "win32"
  });
}

function lockedVersion(crate) {
  const lock = readFileSync(join(repository, "Cargo.lock"), "utf8");
  const match = lock.match(
    new RegExp(`\\[\\[package\\]\\]\\nname = "${crate}"\\nversion = "([^"]+)"`)
  );
  if (!match) throw new Error(`${crate} is missing from Cargo.lock`);
  return match[1];
}

step("Toolchain");
const rustc = run("rustc", ["--version"], { capture: true }).trim();
console.log(`    ${rustc}`);
const expectedBindgen = lockedVersion("wasm-bindgen");
let bindgen;
try {
  bindgen = run("wasm-bindgen", ["--version"], { capture: true }).trim();
} catch {
  throw new Error(
    `wasm-bindgen CLI not found. Install it with:\n  cargo install wasm-bindgen-cli --version ${expectedBindgen} --locked`
  );
}
console.log(`    ${bindgen}`);
if (bindgen.split(/\s+/)[1] !== expectedBindgen) {
  throw new Error(
    `wasm-bindgen CLI ${bindgen} does not match the locked crate ${expectedBindgen}. Install it with:\n  cargo install wasm-bindgen-cli --version ${expectedBindgen} --locked --force`
  );
}

step("Rust to WebAssembly");
run("cargo", [
  "build",
  "--locked",
  "-p",
  "axs-wasm",
  "--target",
  "wasm32-unknown-unknown",
  "--profile",
  "wasm-release"
]);
const rawWasm = join(
  repository,
  "target",
  "wasm32-unknown-unknown",
  "wasm-release",
  "axs_wasm.wasm"
);

step("wasm-bindgen glue");
const scratch = await mkdtemp(join(tmpdir(), "axs-wasm-bindgen-"));
try {
  for (const target of ["nodejs", "web"]) {
    run("wasm-bindgen", ["--target", target, "--out-dir", join(scratch, target), rawWasm]);
  }
  const nodeWasm = await readFile(join(scratch, "nodejs", "axs_wasm_bg.wasm"));
  const webWasm = await readFile(join(scratch, "web", "axs_wasm_bg.wasm"));
  if (!nodeWasm.equals(webWasm)) {
    throw new Error("wasm-bindgen produced different binaries for nodejs and web targets");
  }

  // One shared binary; both loaders resolve `axs_wasm_bg.wasm` next to themselves.
  await rm(wasmDir, { recursive: true, force: true });
  await mkdir(wasmDir, { recursive: true });
  await writeFile(join(wasmDir, "axs_wasm_bg.wasm"), nodeWasm);
  const glue = [
    ["nodejs", "axs_wasm.js", "node.cjs"],
    ["nodejs", "axs_wasm.d.ts", "node.d.cts"],
    ["web", "axs_wasm.js", "web.mjs"],
    ["web", "axs_wasm.d.ts", "web.d.mts"]
  ];
  for (const [target, source, output] of glue) {
    const text = await readFile(join(scratch, target, source), "utf8");
    // The declarations are only used to compile the SDK and are not shipped, so
    // drop the Deno-style pointer to them from the loaders.
    await writeFile(join(wasmDir, output), text.replace(/^\/\* @ts-self-types=.*\*\/\n/m, ""));
  }

  step("TypeScript");
  await rm(distDir, { recursive: true, force: true });
  const tsc = join(packageDir, "node_modules", "typescript", "bin", "tsc");
  if (!existsSync(tsc)) {
    throw new Error("TypeScript is not installed; run `npm ci` in packages/axs first");
  }
  for (const project of ["tsconfig.node.json", "tsconfig.web.json"]) {
    run(process.execPath, [tsc, "-p", join(packageDir, project)]);
  }

  step("Assemble dist");
  await mkdir(join(distDir, "wasm"), { recursive: true });
  for (const file of ["axs_wasm_bg.wasm", "node.cjs", "web.mjs"]) {
    await cp(join(wasmDir, file), join(distDir, "wasm", file));
  }
  // The web build is ESM inside a CommonJS-typed package.
  await writeFile(join(distDir, "web", "package.json"), '{\n  "type": "module"\n}\n');
  const info = {
    rustc,
    wasmBindgen: expectedBindgen,
    wasmSha256: createHash("sha256").update(nodeWasm).digest("hex"),
    wasmBytes: nodeWasm.length
  };
  writeFileSync(join(distDir, "wasm", "build-info.json"), `${JSON.stringify(info, null, 2)}\n`);
  console.log(`    axs_wasm_bg.wasm: ${info.wasmBytes} bytes, sha256 ${info.wasmSha256}`);
} finally {
  await rm(scratch, { recursive: true, force: true });
}

step("Smoke test");
run(process.execPath, [
  "-e",
  `const axs = require(${JSON.stringify(join(distDir, "node", "node.js"))});
   const text = axs.stringify({ id: 9223372036854775807n });
   if (axs.parse(text).id !== 9223372036854775807n) throw new Error("smoke test failed");`
]);
console.log("\nBuild complete.");
