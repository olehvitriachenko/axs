#!/usr/bin/env node
// Installed-extension smoke test in a real VS Code.
//
//   1. Packages the extension (`vsce package`) unless --vsix <file> is given.
//   2. Installs the VSIX into a temporary, isolated extensions directory.
//   3. Starts VS Code with a temporary profile, a temporary workspace containing
//      the test fixtures, and test/host as the test runner, which drives the
//      installed extension and writes a JSON report.
//
// It uses the locally installed VS Code; nothing is downloaded. Set
// VSCODE_EXECUTABLE to the VS Code binary (on macOS
// ".../Visual Studio Code.app/Contents/MacOS/Code") if it is not found. A VS Code
// window opens briefly while the test runs.
//
// Usage: node scripts/smoke-test.mjs [--vsix path/to/file.vsix]

import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const manifest = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const extensionId = `${manifest.publisher}.${manifest.name}`;

function findVSCode() {
  if (process.env.VSCODE_EXECUTABLE) return process.env.VSCODE_EXECUTABLE;
  const candidates = {
    darwin: [
      "/Applications/Visual Studio Code.app/Contents/MacOS/Code",
      "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"
    ],
    linux: ["/usr/share/code/code", "/usr/lib/code/code", "/snap/code/current/usr/share/code/code"],
    win32: [
      join(process.env.LOCALAPPDATA ?? "", "Programs", "Microsoft VS Code", "Code.exe"),
      join(process.env.ProgramFiles ?? "", "Microsoft VS Code", "Code.exe")
    ]
  }[process.platform] ?? [];
  return candidates.find((candidate) => existsSync(candidate));
}

const executable = findVSCode();
if (!executable) {
  console.error("VS Code not found; set VSCODE_EXECUTABLE. Smoke test NOT run.");
  process.exit(2);
}

const vsixIndex = process.argv.indexOf("--vsix");
let vsix = vsixIndex > 0 ? resolve(process.argv[vsixIndex + 1]) : undefined;
if (!vsix) {
  execFileSync("npx", ["vsce", "package", "--no-dependencies"], {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32"
  });
  vsix = join(root, `${manifest.name}-${manifest.version}.vsix`);
}

const scratch = mkdtempSync(join(tmpdir(), "axs-vscode-smoke-"));
const extensionsDir = join(scratch, "extensions");
const userDataDir = join(scratch, "user-data");
const workspace = join(scratch, "workspace");
const resultFile = join(scratch, "result.json");
cpSync(join(root, "test", "fixtures"), workspace, { recursive: true });

try {
  console.log(`VS Code: ${executable}`);
  console.log(`Installing ${vsix} into ${extensionsDir}`);
  // The VS Code binary acts as its own CLI when run as Node.js with cli.js.
  const cli =
    process.platform === "darwin"
      ? join(dirname(dirname(executable)), "Resources", "app", "out", "cli.js")
      : join(dirname(executable), "resources", "app", "out", "cli.js");
  const install = spawnSync(
    executable,
    [cli, "--extensions-dir", extensionsDir, "--user-data-dir", userDataDir, "--install-extension", vsix],
    { encoding: "utf8", env: { ...process.env, ELECTRON_RUN_AS_NODE: "1" } }
  );
  process.stdout.write(install.stdout ?? "");
  process.stderr.write(install.stderr ?? "");
  if (install.status !== 0) throw new Error("installing the VSIX failed");
  const installed = readdirSync(extensionsDir).filter((entry) => entry.startsWith(extensionId));
  console.log(`Installed: ${installed.join(", ")}`);
  if (installed.length !== 1) throw new Error(`expected one installed ${extensionId}`);

  console.log("Running the in-host smoke test...");
  const run = spawnSync(
    executable,
    [
      workspace,
      "--extensions-dir",
      extensionsDir,
      "--user-data-dir",
      userDataDir,
      `--extensionDevelopmentPath=${join(root, "test", "host")}`,
      `--extensionTestsPath=${join(root, "test", "host", "run.js")}`,
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
      "--disable-telemetry",
      "--new-window"
    ],
    {
      encoding: "utf8",
      timeout: 180_000,
      env: {
        ...process.env,
        ELECTRON_RUN_AS_NODE: undefined,
        AXS_SMOKE_EXTENSION_ID: extensionId,
        AXS_SMOKE_EXTENSIONS_DIR: extensionsDir,
        AXS_SMOKE_WORKSPACE: workspace,
        AXS_SMOKE_RESULT: resultFile
      }
    }
  );
  if (!existsSync(resultFile)) {
    process.stdout.write(run.stdout ?? "");
    process.stderr.write(run.stderr ?? "");
    throw new Error(`VS Code exited with ${run.status ?? run.signal} before writing a report`);
  }
  const report = JSON.parse(readFileSync(resultFile, "utf8"));
  console.log(`\nVS Code ${report.vscode} (${process.platform}/${process.arch})`);
  for (const result of report.results) {
    const detail = result.detail ? ` ${JSON.stringify(result.detail)}` : "";
    console.log(`  ${result.ok ? "ok  " : "FAIL"} ${result.name}${result.ok ? detail : ""}`);
    if (!result.ok) console.log(`       ${result.error.split("\n").join("\n       ")}`);
  }
  const failed = report.results.filter((result) => !result.ok).length;
  console.log(`\n${report.results.length - failed}/${report.results.length} smoke steps passed`);
  process.exitCode = failed === 0 && run.status === 0 ? 0 : 1;
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
