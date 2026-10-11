import test from "node:test";
import assert from "node:assert/strict";
import { axsRange, formattingEdit, type LineSource } from "../src/positions";

const lines = (text: string): LineSource => {
  const all = text.split(/\r?\n/);
  return { lineCount: all.length, lineText: (line) => all[line] ?? "" };
};

test("converts code-point columns to UTF-16 offsets", () => {
  const source = lines('name = 🚀🚀\ntitle = "🚀\\q"');
  // Column 11 on line 2 is the backslash after one astral character.
  assert.deepEqual(axsRange(source, 2, 11, 2), {
    startLine: 1,
    startCharacter: 11,
    endLine: 1,
    endCharacter: 13
  });
  // Columns after two astral characters move by four code units.
  assert.deepEqual(axsRange(lines("ab🚀🚀x"), 1, 5, 1), {
    startLine: 0,
    startCharacter: 6,
    endLine: 0,
    endCharacter: 7
  });
  // A span covering an astral character covers both code units.
  assert.deepEqual(axsRange(lines("x🚀y"), 1, 2, 1).endCharacter, 3);
});

test("clamps positions and keeps empty spans visible", () => {
  assert.deepEqual(axsRange(lines("a = 1\nb"), 3, 1, 1), {
    startLine: 1,
    startCharacter: 0,
    endLine: 1,
    endCharacter: 1
  });
  assert.deepEqual(axsRange(lines("abc"), 1, 99, 1), {
    startLine: 0,
    startCharacter: 2,
    endLine: 0,
    endCharacter: 3
  });
  assert.deepEqual(axsRange(lines("abc"), undefined, undefined, undefined), {
    startLine: 0,
    startCharacter: 0,
    endLine: 0,
    endCharacter: 0
  });
  assert.deepEqual(axsRange(lines("abc"), 1, 1, 0).endCharacter, 1);
});

test("formatting edits are minimal and keep line endings", () => {
  assert.equal(formattingEdit("a = 1\n", "a = 1\n", "\n"), undefined);
  assert.equal(formattingEdit("a = 1\r\n", "a = 1\n", "\r\n"), undefined);
  assert.deepEqual(formattingEdit("a = 1\nc = 3\n", "a = 1\nb = 2\nc = 3\n", "\n"), {
    start: 6,
    end: 6,
    text: "b = 2\n"
  });
  const crlf = formattingEdit("z = 1\r\nb = 1.0\r\n", "b = 1.0\nz = 1\n", "\r\n");
  assert.ok(crlf);
  const applied = (original: string, edit: { start: number; end: number; text: string }) =>
    original.slice(0, edit.start) + edit.text + original.slice(edit.end);
  assert.equal(applied("z = 1\r\nb = 1.0\r\n", crlf), "b = 1.0\r\nz = 1\r\n");
  assert.ok(!crlf.text.includes("\n") || crlf.text.includes("\r\n"));
});

test("formatting edits never split surrogate pairs or CRLF", () => {
  const original = "a = 🚀\r\n";
  const target = "a = 🛸\n";
  const edit = formattingEdit(original, target, "\r\n");
  assert.ok(edit);
  assert.equal(original.slice(0, edit.start) + edit.text + original.slice(edit.end), "a = 🛸\r\n");
  // Both astral characters share a high surrogate; the edit must start before it.
  assert.equal(edit.start, 4);
  assert.equal(edit.text, "🛸");
});
