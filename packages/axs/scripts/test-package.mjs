#!/usr/bin/env node
// Packs @axs-lang/core with `npm pack`, checks the tarball contents, installs it
// into fresh consumer projects (no Rust toolchain involved), compiles the
// examples and small consumers with TypeScript, and runs them with Node.js.
//
// Consumers are checked as ESM (NodeNext), CommonJS (Node16), and bundler
// resolution (type-check only), all with skipLibCheck disabled.

import { execFileSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageDir = dirname(dirname(fileURLToPath(import.meta.url)));
const tsc = join(packageDir, "node_modules", "typescript", "bin", "tsc");
const typeRoots = join(packageDir, "node_modules", "@types");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

function run(command, args, cwd, capture = false) {
  console.log(`    $ ${command} ${args.join(" ")}`);
  return execFileSync(command, args, {
    cwd,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
    shell: process.platform === "win32"
  });
}

function check(condition, message) {
  if (!condition) throw new Error(message);
  console.log(`    ok: ${message}`);
}

const root = await mkdtemp(join(tmpdir(), "axs-package-test-"));
try {
  console.log("\n==> build");
  run(process.execPath, [join(packageDir, "scripts", "build.mjs")], packageDir);

  // The build already ran; skipping prepack keeps its log out of the JSON output.
  console.log("\n==> npm pack");
  const packed = JSON.parse(
    run(npm, ["pack", "--json", "--ignore-scripts", "--pack-destination", root], packageDir, true)
  )[0];
  const tarball = join(root, packed.filename);
  const files = packed.files.map((file) => file.path).sort();
  console.log(`    ${packed.filename}: ${files.length} files, ${packed.size} bytes packed, ${packed.unpackedSize} unpacked`);
  for (const file of files) console.log(`      ${file}`);

  const required = [
    "LICENSE",
    "README.md",
    "package.json",
    "dist/node/node.js",
    "dist/node/node.d.ts",
    "dist/node/core.js",
    "dist/node/core.d.ts",
    "dist/web/web.js",
    "dist/web/web.d.ts",
    "dist/web/core.js",
    "dist/web/package.json",
    "dist/wasm/axs_wasm_bg.wasm",
    "dist/wasm/node.cjs",
    "dist/wasm/web.mjs",
    "dist/wasm/build-info.json"
  ];
  for (const file of required) check(files.includes(file), `tarball contains ${file}`);
  const unexpected = files.filter(
    (file) =>
      !file.startsWith("dist/") && !["LICENSE", "README.md", "package.json"].includes(file)
  );
  check(unexpected.length === 0, `no files outside dist/ besides metadata (${unexpected})`);
  check(!files.some((file) => /\.(ts|map|tsbuildinfo)$/.test(file) && !file.endsWith(".d.ts")),
    "no TypeScript sources, source maps, or build info");
  check(files.filter((file) => file.endsWith(".wasm")).length === 1, "exactly one .wasm binary");

  const consumers = [
    {
      name: "esm",
      type: "module",
      compilerOptions: { module: "NodeNext", moduleResolution: "NodeNext" },
      examples: true
    },
    {
      name: "commonjs",
      type: "commonjs",
      compilerOptions: { module: "Node16", moduleResolution: "Node16" }
    },
    {
      name: "bundler",
      type: "module",
      compilerOptions: { module: "ESNext", moduleResolution: "Bundler", noEmit: true }
    }
  ];

  const consumerSource = `import { AxsError, init, parse, stringify, type AxsObject, type AxsValue } from "@axs-lang/core";

interface Config { name: string; id: bigint; enabled: boolean }
const typed: Config = { name: "Oleh", id: 9223372036854775807n, enabled: true };
const text: string = stringify(typed);
const decoded: AxsObject = parse(text);
const id: AxsValue = decoded.id;
if (id !== 9223372036854775807n) throw new Error("bigint lost");
try {
  parse("a = 1\\na = 2");
  throw new Error("expected AxsError");
} catch (error) {
  if (!(error instanceof AxsError) || error.code !== "AXS005") throw error;
}
void init().then(() => console.log("consumer ok", text.trim().split("\\n").join(" | ")));
`;

  for (const consumer of consumers) {
    console.log(`\n==> ${consumer.name} consumer`);
    const dir = join(root, consumer.name);
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(
      join(dir, "package.json"),
      JSON.stringify({ name: `axs-consumer-${consumer.name}`, private: true, type: consumer.type }, null, 2)
    );
    run(npm, ["install", "--offline", "--no-audit", "--no-fund", tarball], dir);
    const installed = await readdir(join(dir, "node_modules"));
    check(
      installed.every((entry) => entry === "@axs-lang" || entry.startsWith(".")),
      `installs with no runtime dependencies (${installed})`
    );

    await writeFile(join(dir, "src", "consumer.ts"), consumerSource);
    if (consumer.examples) {
      for (const example of ["node-typescript", "nestjs-config"]) {
        await cp(join(packageDir, "examples", example), join(dir, "src", example), {
          recursive: true
        });
      }
    }
    await writeFile(
      join(dir, "tsconfig.json"),
      JSON.stringify(
        {
          compilerOptions: {
            target: "ES2022",
            strict: true,
            skipLibCheck: false,
            types: ["node"],
            typeRoots: [typeRoots],
            rootDir: "src",
            outDir: "out",
            ...consumer.compilerOptions
          },
          include: ["src/**/*.ts"]
        },
        null,
        2
      )
    );
    run(process.execPath, [tsc, "-p", dir], dir);
    if (consumer.compilerOptions.noEmit) continue;

    const output = run(process.execPath, [join(dir, "out", "consumer.js")], dir, true);
    console.log(output.trimEnd().replace(/^/gm, "    | "));
    check(output.includes("consumer ok"), `${consumer.name} consumer runs`);

    if (consumer.examples) {
      for (const example of ["node-typescript", "nestjs-config"]) {
        await cp(join(packageDir, "examples", example, "config.axs"), join(dir, "out", example, "config.axs"));
      }
      const plain = run(process.execPath, [join(dir, "out", "node-typescript", "index.js")], dir, true);
      console.log(plain.trimEnd().replace(/^/gm, "    | "));
      check(plain.includes("maxUploadBytes: 9223372036854775807 (bigint)"), "node-typescript example runs");
      check(plain.includes("AXS005 DuplicateKey at 3:3"), "node-typescript example reports errors");
      const nest = run(process.execPath, [join(dir, "out", "nestjs-config", "main.js")], dir, true);
      console.log(nest.trimEnd().replace(/^/gm, "    | "));
      check(nest.includes("listening on 3000"), "nestjs-config example runs");
    }
  }
  console.log("\nPackage test passed.");
} finally {
  await rm(root, { recursive: true, force: true });
}
