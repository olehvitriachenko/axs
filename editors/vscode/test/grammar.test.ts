// Tokenizes AXS with the extension's TextMate grammar using vscode-textmate,
// the engine VS Code uses, and checks the scopes of each value.
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import * as oniguruma from "vscode-oniguruma";
import * as textmate from "vscode-textmate";

const root = join(__dirname, "..");

async function loadGrammar(): Promise<textmate.IGrammar> {
  const wasm = readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm"));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
      createOnigString: (text) => new oniguruma.OnigString(text)
    }),
    loadGrammar: async () =>
      textmate.parseRawGrammar(
        readFileSync(join(root, "syntaxes", "axs.tmLanguage.json"), "utf8"),
        "axs.tmLanguage.json"
      )
  });
  const grammar = await registry.loadGrammar("source.axs");
  assert.ok(grammar);
  return grammar;
}

/** Returns [text, innermost scope] pairs for one line. */
function tokens(grammar: textmate.IGrammar, line: string): Array<[string, string]> {
  const result = grammar.tokenizeLine(line, textmate.INITIAL);
  return result.tokens
    .map((token): [string, string] => [
      line.slice(token.startIndex, token.endIndex),
      token.scopes[token.scopes.length - 1] ?? ""
    ])
    .filter(([text]) => text.trim() !== "");
}

function scopeOf(grammar: textmate.IGrammar, line: string, text: string): string | undefined {
  return tokens(grammar, line).find(([token]) => token === text)?.[1];
}

test("highlights headers, keys, operators, and list markers", async () => {
  const grammar = await loadGrammar();
  assert.equal(scopeOf(grammar, "server:", "server"), "entity.name.tag.axs");
  assert.equal(scopeOf(grammar, "  - UI:", "UI"), "entity.name.tag.axs");
  assert.equal(scopeOf(grammar, "  - UI:", "- "), "punctuation.definition.list.axs");
  assert.equal(scopeOf(grammar, "  port = 3000", "port"), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, "  port = 3000", " ="), "keyword.operator.assignment.axs");
  assert.equal(scopeOf(grammar, "  -", "-"), "punctuation.definition.list.axs");
  assert.equal(scopeOf(grammar, "# note: x = 1", "# note: x = 1"), "comment.line.number-sign.axs");
});

test("classifies whole values like the AXS primitive parser", async () => {
  const grammar = await loadGrammar();
  assert.equal(scopeOf(grammar, "a = true", "true"), "constant.language.axs");
  assert.equal(scopeOf(grammar, "a = null", "null"), "constant.language.axs");
  assert.equal(scopeOf(grammar, "a = -12.5", "-12.5"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, "  - 42", "42"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, "a = []", "[]"), "constant.language.empty-collection.axs");
  // Literals inside longer unquoted strings are plain text.
  assert.deepEqual(tokens(grammar, "title = Hello 42 true").at(-1), [
    "Hello 42 true",
    "string.unquoted.axs"
  ]);
  assert.equal(scopeOf(grammar, "a = 007", "007"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, "  - Rust", "Rust"), "string.unquoted.axs");
});

test("highlights quoted strings and escapes", async () => {
  const grammar = await loadGrammar();
  const line = 'a = "x\\n\\u{1F680}"';
  assert.equal(scopeOf(grammar, line, "\\n"), "constant.character.escape.axs");
  assert.equal(scopeOf(grammar, line, "\\u{1F680}"), "constant.character.escape.axs");
  assert.equal(scopeOf(grammar, line, "x"), "string.quoted.double.axs");
  assert.equal(scopeOf(grammar, '  - "true"', "true"), "string.quoted.double.axs");
});
