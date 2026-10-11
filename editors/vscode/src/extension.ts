import * as vscode from "vscode";
import * as path from "node:path";
import { AxsError, check, format } from "@axs-lang/core";

import { axsRange, formattingEdit } from "./positions";
import { isSkippableLine, planEnter } from "./smartEnter";

// Validation and formatting run the AXS Rust core compiled to WebAssembly and
// bundled with the extension; no Rust toolchain or `axs` executable is used.

const LANGUAGE_ID = "axs";
const COMMAND_ID = "axs.continueList";
const DEBOUNCE_MS = 400;

// Schemes whose documents are validated. Others (git, output, ...) show
// historical or generated content where diagnostics would be noise.
const VALIDATED_SCHEMES = new Set(["file", "untitled", "vscode-remote", "vscode-vfs"]);

const timers = new Map<string, ReturnType<typeof setTimeout>>();
let output: vscode.OutputChannel | undefined;

function isValidated(document: vscode.TextDocument): boolean {
  return document.languageId === LANGUAGE_ID && VALIDATED_SCHEMES.has(document.uri.scheme);
}

function cancel(key: string): void {
  const timer = timers.get(key);
  if (timer !== undefined) {
    clearTimeout(timer);
    timers.delete(key);
  }
}

function toRange(document: vscode.TextDocument, error: AxsError): vscode.Range {
  const range = axsRange(
    { lineCount: document.lineCount, lineText: (line) => document.lineAt(line).text },
    error.line,
    error.column,
    error.length
  );
  return new vscode.Range(
    range.startLine,
    range.startCharacter,
    range.endLine,
    range.endCharacter
  );
}

/**
 * Validates the document's current text synchronously. Because the result is
 * computed from the text at the moment it is published, an older validation
 * can never overwrite a newer one.
 */
function validate(
  diagnostics: vscode.DiagnosticCollection,
  document: vscode.TextDocument
): void {
  if (document.isClosed || !isValidated(document)) {
    diagnostics.delete(document.uri);
    return;
  }
  try {
    check(document.getText());
    diagnostics.delete(document.uri);
  } catch (error) {
    if (!(error instanceof AxsError)) {
      diagnostics.delete(document.uri);
      output?.appendLine(`Validation failed unexpectedly: ${String(error)}`);
      return;
    }
    const diagnostic = new vscode.Diagnostic(
      toRange(document, error),
      error.help ? `${error.reason}\n${error.help}` : error.reason,
      vscode.DiagnosticSeverity.Error
    );
    diagnostic.code = error.code;
    diagnostic.source = "AXS";
    diagnostics.set(document.uri, [diagnostic]);
  }
}

function schedule(
  diagnostics: vscode.DiagnosticCollection,
  document: vscode.TextDocument,
  delay: number
): void {
  if (!isValidated(document)) return;
  const key = document.uri.toString();
  cancel(key);
  if (delay === 0) {
    validate(diagnostics, document);
    return;
  }
  timers.set(
    key,
    setTimeout(() => {
      timers.delete(key);
      validate(diagnostics, document);
    }, delay)
  );
}

function nextContentLine(document: vscode.TextDocument, line: number): string | undefined {
  for (let index = line + 1; index < document.lineCount; index += 1) {
    const text = document.lineAt(index).text;
    if (!isSkippableLine(text)) return text;
  }
  return undefined;
}

async function smartEnter(editor: vscode.TextEditor): Promise<void> {
  const fallback = () => vscode.commands.executeCommand("default:type", { text: "\n" });
  const { document, selection, selections } = editor;

  // Multiple cursors and non-empty selections keep the built-in behavior.
  if (document.languageId !== LANGUAGE_ID || selections.length !== 1 || !selection.isEmpty) {
    await fallback();
    return;
  }

  const position = selection.active;
  const plan = planEnter(
    document.lineAt(position.line).text,
    position.character,
    nextContentLine(document, position.line)
  );

  switch (plan.kind) {
    case "default":
      await fallback();
      return;
    case "insert": {
      const applied = await editor.edit((edit) => edit.insert(position, plan.text));
      if (applied) {
        const lines = plan.text.split("\n");
        const cursor = new vscode.Position(
          position.line + lines.length - 1,
          lines[lines.length - 1]?.length ?? 0
        );
        editor.selection = new vscode.Selection(cursor, cursor);
        editor.revealRange(new vscode.Range(cursor, cursor));
      }
      return;
    }
    case "replaceLine": {
      const line = document.lineAt(position.line);
      const applied = await editor.edit((edit) => edit.replace(line.range, plan.text));
      if (applied) {
        const cursor = new vscode.Position(position.line, plan.text.length);
        editor.selection = new vscode.Selection(cursor, cursor);
      }
      return;
    }
  }
}

function formatDocument(document: vscode.TextDocument): vscode.TextEdit[] {
  const original = document.getText();
  let formatted: string;
  try {
    formatted = format(original);
  } catch (error) {
    // The problem is already shown as a diagnostic; avoid a popup on format-on-save.
    const message = error instanceof AxsError ? error.message : String(error);
    vscode.window.setStatusBarMessage(`$(warning) AXS: cannot format — ${message}`, 5000);
    return [];
  }
  const edit = formattingEdit(
    original,
    formatted,
    document.eol === vscode.EndOfLine.CRLF ? "\r\n" : "\n"
  );
  if (edit === undefined) return [];
  return [
    vscode.TextEdit.replace(
      new vscode.Range(document.positionAt(edit.start), document.positionAt(edit.end)),
      edit.text
    )
  ];
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("AXS");
  // The bundled WebAssembly loader reads this file next to the extension bundle.
  output.appendLine(`AXS core: ${path.join(__dirname, "axs_wasm_bg.wasm")}`);

  const diagnostics = vscode.languages.createDiagnosticCollection(LANGUAGE_ID);

  context.subscriptions.push(
    output,
    diagnostics,
    vscode.workspace.onDidOpenTextDocument((document) => schedule(diagnostics, document, 0)),
    vscode.workspace.onDidChangeTextDocument((event) => {
      if (event.contentChanges.length > 0) {
        schedule(diagnostics, event.document, DEBOUNCE_MS);
      }
    }),
    // Also fires when a document's language changes away from AXS.
    vscode.workspace.onDidCloseTextDocument((document) => {
      cancel(document.uri.toString());
      diagnostics.delete(document.uri);
    }),
    vscode.languages.registerDocumentFormattingEditProvider(LANGUAGE_ID, {
      provideDocumentFormattingEdits: formatDocument
    }),
    // A plain command rather than registerTextEditorCommand: the latter runs
    // its callback inside its own edit transaction, which silently drops the
    // nested editor.edit and default:type calls Smart Enter needs.
    vscode.commands.registerCommand(COMMAND_ID, async () => {
      const editor = vscode.window.activeTextEditor;
      if (editor) await smartEnter(editor);
    }),
    {
      dispose() {
        for (const timer of timers.values()) clearTimeout(timer);
        timers.clear();
      }
    }
  );

  for (const document of vscode.workspace.textDocuments) {
    schedule(diagnostics, document, 0);
  }
}

export function deactivate(): void {}
