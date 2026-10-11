"use strict";
// check() and format() must agree with the Rust CLI (`axs check`, `axs fmt`).
// The parity test runs the CLI from this repository's Cargo target directory
// and is skipped when that binary has not been built.

const test = require("node:test");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { check, format, AxsError } = require("@axs-lang/core");

const repository = join(__dirname, "..", "..", "..");
const cli = join(
  repository,
  "target",
  "debug",
  process.platform === "win32" ? "axs.exe" : "axs"
);

const documents = {
  floats: "b = 1.0\na = 100000000000000000000.0\nc = 2.50\nd = -0.0\ne = 0.000000001\n",
  integers: "max = 9223372036854775807\nmin = -9223372036854775808\nsmall = 7\n",
  nested:
    "user:\n  name = Oleh\n  skills:\n    - Rust\n    -\n      - nested\ncategories:\n  - UI:\n      - MUI\n    weight = 2\n",
  comments: "# heading\nz = 1\n\n# trailing\na = 2\n",
  strings:
    'quoted = "true"\nspaces = " padded "\nescapes = "a\\nb\\t\\"c\\""\nunicode = Олег 🚀\nlist:\n  - "UI:"\n  - \"http://host\"\n',
  empties: "a = []\nb = {}\nc = \"\"\n",
  crlf: "b = 1\r\na:\r\n  - x\r\n",
  canonical: "a = 1\nb:\n  c = true\n",
  empty: "",
  profile: readFileSync(join(repository, "profile.axs"), "utf8")
};

test("format matches documented canonical output", () => {
  assert.equal(
    format(documents.floats),
    "a = 100000000000000000000.0\nb = 1.0\nc = 2.5\nd = -0.0\ne = 0.000000001\n"
  );
  assert.equal(format(documents.canonical), documents.canonical);
  assert.equal(format(""), "");
  assert.equal(format(format(documents.nested)), format(documents.nested));
});

test("check accepts valid documents and reports structured errors", () => {
  for (const source of Object.values(documents)) check(source);
  assert.throws(
    () => check("a = 1\na = 2"),
    (error) => error instanceof AxsError && error.code === "AXS005" && error.line === 2
  );
  assert.throws(
    () => format('name = "🚀\\q"'),
    (error) => error instanceof AxsError && error.code === "AXS007" && error.column === 8
  );
  assert.throws(() => check("x".repeat(8 * 1024 * 1024 + 1)), AxsError);
  assert.throws(() => check("a = \uD800"), (error) => error.code === "AXS007");
  assert.throws(() => check(42), TypeError);
});

test("check and format handle documents near the 8 MiB limit", () => {
  // Long values keep the token count well below the 262,144-token limit.
  const value = "v".repeat(1000);
  let source = "";
  for (let index = 0; source.length + 1100 < 8 * 1024 * 1024; index += 1) {
    source += `k${index} = ${value}\n`;
  }
  check(source);
  const formatted = format(source);
  assert.ok(formatted.length > 7 * 1024 * 1024);
  check(formatted);
});

test("format and check agree with the Rust CLI", { skip: !existsSync(cli) && "CLI not built" }, () => {
  const directory = mkdtempSync(join(tmpdir(), "axs-format-parity-"));
  try {
    for (const [name, source] of Object.entries(documents)) {
      const file = join(directory, `${name}.axs`);
      writeFileSync(file, source);
      execFileSync(cli, ["check", file]);
      execFileSync(cli, ["fmt", file]);
      assert.equal(format(source), readFileSync(file, "utf8"), `${name} formats like axs fmt`);
    }
    const invalid = join(directory, "invalid.axs");
    writeFileSync(invalid, "a = 1\na = 2");
    assert.throws(() => execFileSync(cli, ["check", invalid], { stdio: "pipe" }));
    assert.throws(() => check("a = 1\na = 2"), AxsError);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
