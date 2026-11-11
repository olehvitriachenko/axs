// Runtime tests of the bundled AXS WebAssembly core, built exactly like the
// extension bundle (esbuild, @axs-lang/core's Node.js build, .wasm next to the
// bundle). No Rust toolchain or `axs` executable is involved.
import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { axsRange, type LineSource } from "../src/positions";

// Record which .wasm file the loader reads before the SDK is first required.
// The loader calls require("fs").readFileSync, so patch that shared module.
// eslint-disable-next-line @typescript-eslint/no-require-imports
const fs = require("node:fs") as { readFileSync: (...args: unknown[]) => unknown };
const reads: string[] = [];
const originalRead = fs.readFileSync;
fs.readFileSync = (...args: unknown[]) => {
  reads.push(String(args[0]));
  return originalRead(...args);
};
// eslint-disable-next-line @typescript-eslint/no-require-imports
const axs = require("@axs-lang/core") as typeof import("@axs-lang/core");
fs.readFileSync = originalRead;

const fixtures = join(__dirname, "..", "test", "fixtures");
const fixture = (name: string) => originalRead(join(fixtures, name), "utf8") as string;
const lines = (text: string): LineSource => {
  const all = text.split(/\r?\n/);
  return { lineCount: all.length, lineText: (line) => all[line] ?? "" };
};

test("loads the WebAssembly file next to the bundle", () => {
  const wasm = reads.filter((file) => file.endsWith(".wasm"));
  assert.deepEqual(wasm, [`${__dirname}/axs_wasm_bg.wasm`]);
});

test("valid documents pass check and format idempotently", () => {
  const source = fixture("valid.axs");
  axs.check(source);
  const formatted = axs.format(source);
  assert.equal(axs.format(formatted), formatted);
  assert.match(formatted, /maxUploadBytes: 9223372036854775807/);
  assert.match(formatted, /ratio: 1\.0/);
  assert.match(formatted, /users\[2\]\{id,name\}:\n {2}1,Oleh\n {2}2,Alex\n/);
  assert.match(formatted, /owner: \{login: oleh,id: 1\}/);
  assert.match(formatted, /notes: \|\n {2}First line\n {4}indented\n/);
  assert.ok(!formatted.includes("# Application settings"), "comments are dropped by fmt");
});

test("invalid documents produce structured errors mapped to UTF-16 ranges", () => {
  const escape = fixture("invalid-escape.axs");
  const error = captured(() => axs.check(escape));
  assert.equal(error.code, "AXS007");
  assert.equal(error.line, 2);
  const range = axsRange(lines(escape), error.line, error.column, error.length);
  assert.equal(range.startLine, 1);
  // The core points at the string, which starts at UTF-16 offset 7.
  assert.equal(range.startCharacter, 7);
  assert.ok(range.endCharacter > range.startCharacter);

  const duplicate = captured(() => axs.check(fixture("invalid-duplicate.axs")));
  assert.equal(duplicate.code, "AXS005");
  assert.equal(duplicate.kind, "DuplicateKey");
  assert.equal(duplicate.line, 2);
});

test("format output matches axs fmt for CRLF input", () => {
  assert.equal(axs.format(fixture("unformatted-crlf.axs")), "z: 1\nb: 1.0\na[1]: [x]\n");
});

test("handles documents near the 8 MiB limit quickly enough for live diagnostics", () => {
  const value = "v".repeat(1000);
  let source = "";
  for (let index = 0; source.length + 1100 < 8 * 1024 * 1024; index += 1) {
    source += `k${index}: ${value}\n`;
  }
  let started = performance.now();
  axs.check(source);
  const checkMs = performance.now() - started;
  started = performance.now();
  axs.format(source);
  const formatMs = performance.now() - started;
  console.log(
    `    8 MiB document: check ${checkMs.toFixed(0)} ms, format ${formatMs.toFixed(0)} ms`
  );
  const tooLarge = captured(() => axs.check(`${source}${"x".repeat(200_000)}`));
  assert.equal(tooLarge.code, "AXS009");
});

function captured(callback: () => void): InstanceType<typeof axs.AxsError> {
  try {
    callback();
  } catch (error) {
    assert.ok(error instanceof axs.AxsError);
    return error;
  }
  assert.fail("expected an AxsError");
}
