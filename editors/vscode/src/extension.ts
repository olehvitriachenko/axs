
import * as vscode from "vscode";
import * as path from "node:path";
import * as os from "node:os";
import * as fs from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const COMMAND_ID = "axs.continueList";
const DEBOUNCE_MS = 400;

const BLOCK_HEADER =
  /^[A-Za-z_][A-Za-z0-9_-]*\s*:\s*$/;

const INLINE_BLOCK_HEADER =
  /^-\s+[A-Za-z_][A-Za-z0-9_-]*\s*:\s*$/;

const validationTimers = new Map<string, ReturnType<typeof setTimeout>>();
const validationRequests = new Map<string, number>();

function isAxs(document: vscode.TextDocument): boolean {
  return document.languageId === "axs";
}

function getCliPath(context: vscode.ExtensionContext): string {
  const configured = vscode.workspace
    .getConfiguration("axs")
    .get<string>("cliPath");

  if (configured) {
    return configured;
  }

  return path.resolve(
    context.extensionPath,
    "../../target/debug/axs"
  );
}

async function runCli(
  context: vscode.ExtensionContext,
  args: string[]
): Promise<{ stdout: string; stderr: string; code: number }> {
  try {
    const result = await execFileAsync(getCliPath(context), args, {
      timeout: 10000,
      maxBuffer: 1024 * 1024,
      encoding: "utf8"
    });

    return {
      stdout: result.stdout,
      stderr: result.stderr,
      code: 0
    };
  } catch (error) {
    const failure = error as Error & {
      code?: string | number;
      stdout?: string;
      stderr?: string;
    };

    if (typeof failure.code !== "number") {
      throw new Error(`Cannot execute AXS CLI: ${failure.message}`);
    }

    return {
      stdout: failure.stdout ?? "",
      stderr: failure.stderr ?? "",
      code: failure.code
    };
  }
}

async function withTempAxs<T>(
  source: string,
  callback: (file: string) => Promise<T>
): Promise<T> {
  const directory = await fs.mkdtemp(
    path.join(os.tmpdir(), "axs-vscode-")
  );

  const file = path.join(directory, "document.axs");

  try {
    await fs.writeFile(file, source, "utf8");
    return await callback(file);
  } finally {
    await fs.rm(directory, {
      recursive: true,
      force: true
    });
  }
}

function parseDiagnostics(
  document: vscode.TextDocument,
  output: string
): vscode.Diagnostic[] {
  const header = output.match(
    /error\[(AXS\d+)\]:\s*([^\r\n]+)/
  );

  if (!header) {
    return [];
  }

  const location = output.match(
    /-->\s+.+:(\d+):(\d+)(?:\r?\n|$)/
  );

  const help = output.match(/^help:\s*(.+)$/m);

  const code = header[1];
  const message = header[2];

  let range = new vscode.Range(0, 0, 0, 0);

  if (location) {
    const lineNumber = Math.max(
      0,
      Math.min(Number(location[1]) - 1, document.lineCount - 1)
    );

    const text = document.lineAt(lineNumber).text;
    const rawColumn = Number(location[2]) - 1;

    // AXS reports character-based columns.
    // Convert them to VS Code's UTF-16 positions.
    const characters = Array.from(text);
    const codePointOffset = Math.max(
      0,
      Math.min(rawColumn, characters.length)
    );

    const startColumn =
      characters.slice(0, codePointOffset).join("").length;

    const endColumn = Math.min(startColumn + 1, text.length);

    range = new vscode.Range(
      lineNumber,
      startColumn,
      lineNumber,
      endColumn
    );
  }

  const diagnostic = new vscode.Diagnostic(
    range,
    help ? `${message}\n${help[1]}` : message,
    vscode.DiagnosticSeverity.Error
  );

  diagnostic.code = code;
  diagnostic.source = "AXS";

  return [diagnostic];
}

function invalidateRequest(key: string): number {
  const next = (validationRequests.get(key) ?? 0) + 1;
  validationRequests.set(key, next);
  return next;
}

function cancelTimer(key: string): void {
  const timer = validationTimers.get(key);

  if (timer !== undefined) {
    clearTimeout(timer);
    validationTimers.delete(key);
  }
}

async function validateDocument(
  context: vscode.ExtensionContext,
  diagnostics: vscode.DiagnosticCollection,
  document: vscode.TextDocument,
  requestId: number
): Promise<void> {
  if (!isAxs(document) || document.uri.scheme !== "file") {
    return;
  }

  const version = document.version;
  const key = document.uri.toString();

  const isCurrent = () =>
    !document.isClosed &&
    document.version === version &&
    validationRequests.get(key) === requestId;

  try {
    const result = await withTempAxs(
      document.getText(),
      (file) => runCli(context, ["check", file])
    );

    if (!isCurrent()) {
      return;
    }

    if (result.code === 0) {
      diagnostics.delete(document.uri);
      return;
    }

    const output = result.stderr || result.stdout;
    const issues = parseDiagnostics(document, output);

    if (issues.length > 0) {
      diagnostics.set(document.uri, issues);
    } else {
      diagnostics.delete(document.uri);
      console.warn("AXS validation failed:", output);
    }
  } catch (error) {
    if (!isCurrent()) {
      return;
    }

    diagnostics.delete(document.uri);
    console.error("AXS validation error:", error);
  }
}

function scheduleValidation(
  context: vscode.ExtensionContext,
  diagnostics: vscode.DiagnosticCollection,
  document: vscode.TextDocument,
  immediate = false
): void {
  if (!isAxs(document) || document.uri.scheme !== "file") {
    return;
  }

  const key = document.uri.toString();

  cancelTimer(key);
  const requestId = invalidateRequest(key);

  if (immediate) {
    void validateDocument(context, diagnostics, document, requestId);
    return;
  }

  const timer = setTimeout(() => {
    validationTimers.delete(key);
    void validateDocument(context, diagnostics, document, requestId);
  }, DEBOUNCE_MS);

  validationTimers.set(key, timer);
}

async function insertLine(
  editor: vscode.TextEditor,
  position: vscode.Position,
  value: string
): Promise<void> {
  const success = await editor.edit((edit) => {
    edit.insert(position, value);
  });

  if (!success) {
    return;
  }

  const lines = value.split("\n");
  const lastLine = lines[lines.length - 1];

  const cursor = new vscode.Position(
    position.line + lines.length - 1,
    lastLine.length
  );

  editor.selection = new vscode.Selection(cursor, cursor);
}

async function defaultEnter(): Promise<void> {
  await vscode.commands.executeCommand("default:type", {
    text: "\n"
  });
}

async function handleSmartEnter(
  editor: vscode.TextEditor
): Promise<void> {
  if (!isAxs(editor.document)) {
    await defaultEnter();
    return;
  }

  const selection = editor.selection;

  if (!selection.isEmpty) {
    await defaultEnter();
    return;
  }

  const position = selection.active;
  const line = editor.document.lineAt(position.line);

  const before = line.text.slice(0, position.character);
  const after = line.text.slice(position.character);

  if (after.trim().length > 0) {
    await defaultEnter();
    return;
  }

  const indent = before.match(/^ */)?.[0] ?? "";
  const content = before.slice(indent.length);

  // - UI:
  //     - |
  if (INLINE_BLOCK_HEADER.test(content)) {
    await insertLine(
      editor,
      position,
      "\n" + indent + "    - "
    );
    return;
  }

  // skills:
  //   - |
  //
  // admin:
  //   - |
  if (BLOCK_HEADER.test(content)) {
    await insertLine(
      editor,
      position,
      "\n" + indent + "  - "
    );
    return;
  }

  // - Rust
  // - |
  const list = content.match(/^-\s*(.*)$/);

  if (list) {
    const value = list[1];

    // Empty '-' ends the current list item.
    if (value.trim() === "") {
      const range = new vscode.Range(
        position.line,
        indent.length,
        position.line,
        position.character
      );

      const success = await editor.edit((edit) => {
        edit.delete(range);
      });

      if (success) {
        const cursor = new vscode.Position(
          position.line,
          indent.length
        );

        editor.selection = new vscode.Selection(cursor, cursor);
      }

      return;
    }

    await insertLine(
      editor,
      position,
      "\n" + indent + "- "
    );
    return;
  }

  await defaultEnter();
}

export function activate(context: vscode.ExtensionContext): void {
  const diagnostics =
    vscode.languages.createDiagnosticCollection("axs");

  context.subscriptions.push(diagnostics);

  // Live diagnostics with 400 ms debounce.
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((event) => {
      scheduleValidation(
        context,
        diagnostics,
        event.document
      );
    })
  );

  // Immediate validation on save.
  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      scheduleValidation(
        context,
        diagnostics,
        document,
        true
      );
    })
  );

  // Validate opened files.
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => {
      scheduleValidation(
        context,
        diagnostics,
        document,
        true
      );
    })
  );

  // Cleanup when the document closes.
  context.subscriptions.push(
    vscode.workspace.onDidCloseTextDocument((document) => {
      const key = document.uri.toString();

      cancelTimer(key);
      invalidateRequest(key);
      validationRequests.delete(key);
      diagnostics.delete(document.uri);
    })
  );

  // Cleanup timers on extension shutdown.
  context.subscriptions.push({
    dispose() {
      for (const timer of validationTimers.values()) {
        clearTimeout(timer);
      }

      validationTimers.clear();
      validationRequests.clear();
    }
  });

  // Validate already-open documents.
  for (const document of vscode.workspace.textDocuments) {
    if (isAxs(document)) {
      scheduleValidation(
        context,
        diagnostics,
        document,
        true
      );
    }
  }

  // Format Document.
  const formatter =
    vscode.languages.registerDocumentFormattingEditProvider(
      { language: "axs" },
      {
        async provideDocumentFormattingEdits(document) {
          try {
            const original = document.getText();

            const formatted = await withTempAxs(
              original,
              async (file) => {
                const result = await runCli(
                  context,
                  ["fmt", file]
                );

                if (result.code !== 0) {
                  throw new Error(
                    result.stderr || "AXS formatting failed"
                  );
                }

                return fs.readFile(file, "utf8");
              }
            );

            if (original === formatted) {
              return [];
            }

            const range = new vscode.Range(
              document.positionAt(0),
              document.positionAt(original.length)
            );

            return [
              vscode.TextEdit.replace(range, formatted)
            ];
          } catch (error) {
            const message = error instanceof Error
              ? error.message
              : String(error);

            void vscode.window.showErrorMessage(message);
            return [];
          }
        }
      }
    );

  context.subscriptions.push(formatter);

  // Smart Enter.
  const smartEnter =
    vscode.commands.registerTextEditorCommand(
      COMMAND_ID,
      async (editor) => {
        await handleSmartEnter(editor);
      }
    );

  context.subscriptions.push(smartEnter);
}

export function deactivate(): void {}
