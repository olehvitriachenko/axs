// Runs inside the VS Code extension host (see scripts/smoke-test.mjs). It drives
// the AXS Language extension installed from the VSIX into an isolated
// extensions directory and writes a JSON report.
"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vscode = require("vscode");

const {
  AXS_SMOKE_EXTENSION_ID: extensionId,
  AXS_SMOKE_EXTENSIONS_DIR: extensionsDir,
  AXS_SMOKE_WORKSPACE: workspace,
  AXS_SMOKE_RESULT: resultFile
} = process.env;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitFor(condition, timeout = 5000) {
  const started = Date.now();
  for (;;) {
    const value = await condition();
    if (value) return value;
    if (Date.now() - started > timeout) throw new Error(`timed out after ${timeout} ms`);
    await sleep(25);
  }
}

const diagnosticsOf = (document) => vscode.languages.getDiagnostics(document.uri);

async function open(name) {
  const document = await vscode.workspace.openTextDocument(path.join(workspace, name));
  const editor = await vscode.window.showTextDocument(document);
  return { document, editor };
}

async function scratch(text) {
  const document = await vscode.workspace.openTextDocument({ language: "axs", content: text });
  const editor = await vscode.window.showTextDocument(document);
  return { document, editor };
}

async function replaceAll(editor, text) {
  const { document } = editor;
  const end = document.lineAt(document.lineCount - 1).range.end;
  await editor.edit((edit) => edit.replace(new vscode.Range(0, 0, end.line, end.character), text));
}

async function enterAt(editor, line, character) {
  const position = new vscode.Position(line, character);
  editor.selection = new vscode.Selection(position, position);
  await vscode.commands.executeCommand("axs.continueList");
  return editor.document.getText();
}

exports.run = async function run() {
  const results = [];
  const step = async (name, body) => {
    const started = Date.now();
    try {
      const detail = await body();
      results.push({ name, ok: true, ms: Date.now() - started, ...(detail ? { detail } : {}) });
    } catch (error) {
      results.push({ name, ok: false, error: String(error && error.stack ? error.stack : error) });
    }
  };

  // Record file reads so we can see where the WebAssembly core comes from.
  const reads = [];
  const originalRead = fs.readFileSync;
  fs.readFileSync = function (file, ...rest) {
    reads.push(String(file));
    return originalRead.call(this, file, ...rest);
  };

  const extension = vscode.extensions.getExtension(extensionId);

  await step("extension is installed from the VSIX and inactive before any AXS file", () => {
    assert.ok(extension, `${extensionId} not found`);
    assert.ok(
      fs.realpathSync(extension.extensionPath).startsWith(fs.realpathSync(extensionsDir)),
      `${extension.extensionPath} is not inside ${extensionsDir}`
    );
    assert.equal(extension.isActive, false);
    return { extensionPath: extension.extensionPath, version: vscode.version };
  });

  await step("opening an .axs file activates it and loads the bundled .wasm", async () => {
    const { document } = await open("valid.axs");
    assert.equal(document.languageId, "axs");
    await waitFor(() => extension.isActive);
    fs.readFileSync = originalRead;
    const wasm = reads.filter((file) => file.endsWith(".wasm"));
    const expected = path.join(extension.extensionPath, "dist", "axs_wasm_bg.wasm");
    // Compare real paths: macOS temp directories are reached through /private.
    assert.deepEqual(wasm.map((file) => fs.realpathSync(file)), [fs.realpathSync(expected)]);
    return { wasm: wasm[0] };
  });
  fs.readFileSync = originalRead;

  await step("valid document has no diagnostics", async () => {
    const { document } = await open("valid.axs");
    await sleep(600);
    assert.deepEqual(diagnosticsOf(document), []);
  });

  await step("invalid escape is reported with code and UTF-16 range", async () => {
    const { document } = await open("invalid-escape.axs");
    const [diagnostic] = await waitFor(() => diagnosticsOf(document).length && diagnosticsOf(document));
    assert.equal(diagnostic.code, "AXS007");
    assert.equal(diagnostic.source, "AXS");
    assert.equal(diagnostic.severity, vscode.DiagnosticSeverity.Error);
    assert.equal(diagnostic.range.start.line, 1);
    // `title = "🚀\q"`: the string starts at UTF-16 offset 8.
    assert.equal(diagnostic.range.start.character, 8);
    return { message: diagnostic.message, range: `${diagnostic.range.start.line}:${diagnostic.range.start.character}-${diagnostic.range.end.character}` };
  });

  await step("duplicate key is reported on its line", async () => {
    const { document } = await open("invalid-duplicate.axs");
    const [diagnostic] = await waitFor(() => diagnosticsOf(document).length && diagnosticsOf(document));
    assert.equal(diagnostic.code, "AXS005");
    assert.equal(diagnostic.range.start.line, 1);
    return { message: diagnostic.message };
  });

  await step("live diagnostics are debounced (~400 ms)", async () => {
    const { document, editor } = await scratch("a = 1\n");
    await sleep(100);
    assert.deepEqual(diagnosticsOf(document), []);
    const edited = Date.now();
    await replaceAll(editor, "a = 1\na = 2\n");
    await sleep(200);
    assert.equal(diagnosticsOf(document).length, 0, "reported before the debounce elapsed");
    await waitFor(() => diagnosticsOf(document).length === 1, 3000);
    const latency = Date.now() - edited;
    assert.ok(latency >= 350, `reported after ${latency} ms`);
    await replaceAll(editor, "a = 1\n");
    await waitFor(() => diagnosticsOf(document).length === 0, 3000);
    return { latencyMs: latency };
  });

  await step("rapid edits: the newest text always wins", async () => {
    const { document, editor } = await scratch("a = 1\n");
    for (let round = 0; round < 10; round += 1) {
      await replaceAll(editor, round % 2 === 0 ? "a = 1\na = 2\n" : "a = 1\n");
      await sleep(30);
    }
    await sleep(800);
    assert.deepEqual(diagnosticsOf(document), [], "final valid text must have no diagnostics");
    await replaceAll(editor, "a = 1\n");
    for (let round = 0; round < 10; round += 1) {
      await replaceAll(editor, round % 2 === 0 ? "a = 1\n" : "b:\n   c = 1\n");
      await sleep(30);
    }
    await waitFor(() => diagnosticsOf(document).length === 1, 3000);
    await sleep(600);
    assert.equal(diagnosticsOf(document)[0].code, "AXS002");
  });

  await step("Format Document matches axs fmt and keeps CRLF", async () => {
    const { document } = await open("unformatted-crlf.axs");
    assert.equal(document.eol, vscode.EndOfLine.CRLF);
    await vscode.commands.executeCommand("editor.action.formatDocument");
    await waitFor(() => document.getText() !== "z = 1\r\nb = 1.0\r\na:\r\n  - x\r\n", 3000);
    assert.equal(document.getText(), "a:\r\n  - x\r\nb = 1.0\r\nz = 1\r\n");
    const again = await vscode.commands.executeCommand(
      "vscode.executeFormatDocumentProvider",
      document.uri,
      { tabSize: 2, insertSpaces: true }
    );
    assert.equal((again ?? []).length, 0, "formatted document must be stable");
  });

  await step("Format Document leaves invalid documents unchanged", async () => {
    const { document } = await open("invalid-duplicate.axs");
    const edits = await vscode.commands.executeCommand(
      "vscode.executeFormatDocumentProvider",
      document.uri,
      { tabSize: 2, insertSpaces: true }
    );
    assert.equal((edits ?? []).length, 0);
    assert.equal(document.getText(), "a = 1\na = 2\n");
  });

  await step("Smart Enter: objects, lists, nesting, and ending lists", async () => {
    const { editor } = await scratch("server:");
    assert.equal(await enterAt(editor, 0, 7), "server:\n  - ", "new header starts a list");
    assert.equal(editor.selection.active.character, 4, "cursor follows the marker");
    await vscode.commands.executeCommand("undo");
    assert.equal(editor.document.getText(), "server:", "Enter is one undo step");
    await vscode.commands.executeCommand("redo");
    assert.equal(editor.document.getText(), "server:\n  - ");

    await replaceAll(editor, "server:\n  port = 1");
    assert.equal(await enterAt(editor, 0, 7), "server:\n  \n  port = 1", "existing object keeps its style");

    await replaceAll(editor, "skills:\n  - Rust");
    assert.equal(await enterAt(editor, 1, 8), "skills:\n  - Rust\n  - ", "list continues");
    assert.equal(await enterAt(editor, 2, 4), "skills:\n  - Rust\n", "empty item ends the list");
    assert.equal(editor.selection.active.character, 0);

    await replaceAll(editor, "skills:\n  - Rust");
    assert.equal(await enterAt(editor, 0, 7), "skills:\n  - \n  - Rust", "header mirrors list child");

    await replaceAll(editor, "categories:\n  - UI:");
    assert.equal(await enterAt(editor, 1, 7), "categories:\n  - UI:\n      - ", "inline header +4");

    await replaceAll(editor, "matrix:\n  -");
    assert.equal(await enterAt(editor, 1, 3), "matrix:\n  -\n    - ", "bare dash opens nested");

    await replaceAll(editor, "people:\n  - name = Oleh");
    assert.equal(await enterAt(editor, 1, 15), "people:\n  - name = Oleh\n    ", "inline object field");

    await replaceAll(editor, "  port = 1");
    assert.equal(await enterAt(editor, 0, 10), "  port = 1\n  ", "assignment keeps indent");
  });

  await step("Smart Enter object preference is read from configuration", async () => {
    const { editor } = await scratch("data:");
    const settings = vscode.workspace.getConfiguration("axs", editor.document.uri);
    assert.equal(settings.get("smartEnter.defaultCollection"), "array");
    try {
      await settings.update("smartEnter.defaultCollection", "object", vscode.ConfigurationTarget.Global);
      assert.equal(await enterAt(editor, 0, 5), "data:\n  ");
      await replaceAll(editor, "data:\n  - x");
      assert.equal(await enterAt(editor, 0, 5), "data:\n  - \n  - x", "existing list overrides object preference");
    } finally {
      await settings.update("smartEnter.defaultCollection", undefined, vscode.ConfigurationTarget.Global);
    }
  });

  await step("Smart Enter keeps default behavior for multiple cursors", async () => {
    const { editor } = await scratch("- a\n- b");
    editor.selections = [new vscode.Selection(0, 3, 0, 3), new vscode.Selection(1, 3, 1, 3)];
    await vscode.commands.executeCommand("axs.continueList");
    const lines = editor.document.getText().split("\n");
    assert.equal(lines.length, 4);
    assert.ok(!lines.some((line) => line.trim() === "-"), `unexpected list items: ${JSON.stringify(lines)}`);
  });

  await step("Enter keybinding and AXS editor defaults are contributed", () => {
    const manifest = extension.packageJSON;
    const binding = manifest.contributes.keybindings.find((item) => item.command === "axs.continueList");
    assert.equal(binding.key, "enter");
    assert.match(binding.when, /editorLangId == axs/);
    const settings = vscode.workspace.getConfiguration("editor", { languageId: "axs" });
    assert.equal(settings.get("tabSize"), 2);
    assert.equal(settings.get("insertSpaces"), true);
  });

  await step("diagnostics are removed when the language changes", async () => {
    const { document } = await open("invalid-duplicate.axs");
    await waitFor(() => diagnosticsOf(document).length === 1);
    await vscode.languages.setTextDocumentLanguage(document, "plaintext");
    await waitFor(() => diagnosticsOf(document).length === 0, 3000);
  });

  await step("syntax highlighting scopes (internal token capture)", async () => {
    const { document } = await open("valid.axs");
    let tokens;
    try {
      tokens = await vscode.commands.executeCommand("_workbench.captureSyntaxTokens", document.uri);
    } catch (error) {
      return { skipped: `token capture unavailable: ${error.message}` };
    }
    if (!Array.isArray(tokens)) return { skipped: "token capture returned no data" };
    const scopeOf = (text) => tokens.find((token) => token.c === text)?.t;
    assert.match(scopeOf("server") ?? "", /entity\.name\.tag\.axs/);
    assert.match(scopeOf("3000") ?? "", /constant\.numeric\.axs/);
    assert.match(scopeOf("true") ?? "", /constant\.language\.axs/);
    assert.match(scopeOf("Олег 🚀 says true") ?? "", /string\.unquoted\.axs/);
    assert.match(scopeOf("# Application settings") ?? "", /comment\.line/);
    return { tokens: tokens.length };
  });

  await vscode.commands.executeCommand("workbench.action.closeAllEditors");
  fs.writeFileSync(resultFile, JSON.stringify({ vscode: vscode.version, results }, null, 2));
  const failed = results.filter((result) => !result.ok);
  if (failed.length > 0) {
    throw new Error(`${failed.length} smoke test step(s) failed`);
  }
  // Shut down only this isolated test instance after its report is persisted.
  await vscode.commands.executeCommand("workbench.action.quit");
};
