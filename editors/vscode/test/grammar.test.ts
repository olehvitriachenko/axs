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

/** Tokenizes several lines in sequence, carrying the grammar state. */
function scopesPerLine(grammar: textmate.IGrammar, lines: string[]): string[][] {
  let state = textmate.INITIAL;
  return lines.map((line) => {
    const result = grammar.tokenizeLine(line, state);
    state = result.ruleStack;
    return result.tokens.map((token) => token.scopes[token.scopes.length - 1] ?? "");
  });
}

test("highlights headers, keys, separators, and list markers", async () => {
  const grammar = await loadGrammar();
  assert.equal(scopeOf(grammar, "server:", "server"), "entity.name.tag.axs");
  assert.equal(scopeOf(grammar, "  - UI:", "UI"), "entity.name.tag.axs");
  assert.equal(scopeOf(grammar, "  - UI:", "- "), "punctuation.definition.list.axs");
  assert.equal(scopeOf(grammar, "  port: 3000", "port"), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, "  port: 3000", ":"), "punctuation.separator.key-value.axs");
  assert.equal(scopeOf(grammar, "  -", "-"), "punctuation.definition.list.axs");
  assert.equal(scopeOf(grammar, "# note: x: 1", "# note: x: 1"), "comment.line.number-sign.axs");
  assert.equal(scopeOf(grammar, '"@id": x', '"@id"'), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, "user.name: x", "user.name"), "variable.other.property.axs");
});

test("highlights array lengths and table headers", async () => {
  const grammar = await loadGrammar();
  assert.equal(scopeOf(grammar, "tags[3]: [a,b,c]", "[3]"), "constant.numeric.length.axs");
  assert.equal(scopeOf(grammar, "users[2]{id,name}:", "users"), "entity.name.tag.axs");
  assert.equal(scopeOf(grammar, "users[2]{id,name}:", "[2]"), "constant.numeric.length.axs");
  assert.equal(scopeOf(grammar, "users[2]{id,name}:", "id"), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, "users[2]{id,name}:", "name"), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, "  - members{name}:", "members"), "entity.name.tag.axs");
});

test("classifies whole values like the AXS primitive parser", async () => {
  const grammar = await loadGrammar();
  assert.equal(scopeOf(grammar, "a: true", "true"), "constant.language.axs");
  assert.equal(scopeOf(grammar, "a: null", "null"), "constant.language.axs");
  assert.equal(scopeOf(grammar, "a: -12.5", "-12.5"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, "a: 1.5e-7", "1.5e-7"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, "  - 42", "42"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, "a: []", "[]"), "constant.language.empty-collection.axs");
  // Literals inside longer unquoted strings are plain text.
  assert.deepEqual(tokens(grammar, "title: Hello 42 true").at(-1), [
    "Hello 42 true",
    "string.unquoted.axs"
  ]);
  assert.equal(scopeOf(grammar, "a: 007", "007"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, "  - Rust", "Rust"), "string.unquoted.axs");
  // A colon without a following space is part of the string.
  assert.equal(scopeOf(grammar, "  - http://host", "http://host"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, "t: 10:30", "10:30"), "string.unquoted.axs");
});

test("highlights inline arrays, inline objects, and table rows", async () => {
  const grammar = await loadGrammar();
  const array = "tags: [rust,2,true,\"a, b\"]";
  assert.equal(scopeOf(grammar, array, "rust"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, array, "2"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, array, "true"), "constant.language.axs");
  assert.equal(scopeOf(grammar, array, "a, b"), "string.quoted.double.axs");
  const object = "user: {login: ivan,id: 10003}";
  assert.equal(scopeOf(grammar, object, "login"), "variable.other.property.axs");
  assert.equal(scopeOf(grammar, object, "ivan"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, object, "10003"), "constant.numeric.axs");
  const row = "  1,Oleh,true,\"x, y\"";
  assert.equal(scopeOf(grammar, row, "1"), "constant.numeric.axs");
  assert.equal(scopeOf(grammar, row, "Oleh"), "string.unquoted.axs");
  assert.equal(scopeOf(grammar, row, "true"), "constant.language.axs");
  assert.equal(scopeOf(grammar, row, "x, y"), "string.quoted.double.axs");
});

test("text blocks run until the indentation returns", async () => {
  const grammar = await loadGrammar();
  const scopes = scopesPerLine(grammar, [
    "body: |",
    "  not a key: value",
    "  # not a comment",
    "",
    "    indented",
    "next: 1",
    "items:",
    "  - |",
    "    first",
    "  - x"
  ]);
  assert.equal(scopes[0]?.at(-1), "keyword.control.text-block.axs");
  for (const line of [1, 2, 4]) assert.deepEqual(scopes[line], ["string.unquoted.block.axs"]);
  assert.equal(scopes[5]?.[0], "variable.other.property.axs");
  assert.deepEqual(scopes[8], ["string.unquoted.block.axs"]);
  assert.equal(scopes[9]?.at(-1), "string.unquoted.axs");
});

test("highlights quoted strings and escapes", async () => {
  const grammar = await loadGrammar();
  const line = 'a: "x\\n\\u{1F680}"';
  assert.equal(scopeOf(grammar, line, "\\n"), "constant.character.escape.axs");
  assert.equal(scopeOf(grammar, line, "\\u{1F680}"), "constant.character.escape.axs");
  assert.equal(scopeOf(grammar, line, "x"), "string.quoted.double.axs");
  assert.equal(scopeOf(grammar, '  - "true"', "true"), "string.quoted.double.axs");
});
