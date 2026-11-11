import test from "node:test";
import assert from "node:assert/strict";
import { isSkippableLine, planEnter, type EnterPlan } from "../src/smartEnter";

const atEnd = (line: string, next?: string): EnterPlan => planEnter(line, line.length, next);
const insert = (text: string): EnterPlan => ({ kind: "insert", text });

test("new block headers insert a list marker by default", () => {
  assert.deepEqual(atEnd("server:"), insert("\n  - "));
  assert.deepEqual(atEnd("  nested:"), insert("\n    - "));
  assert.deepEqual(atEnd("server :  "), insert("\n  - "));
});

test("block headers mirror an existing first child", () => {
  assert.deepEqual(atEnd("skills:", "  - Rust"), insert("\n  - "));
  assert.deepEqual(atEnd("skills:", "  -"), insert("\n  - "));
  assert.deepEqual(atEnd("server:", "  port: 1"), insert("\n  "));
  // A list at another indentation is not this header's child.
  assert.deepEqual(atEnd("skills:", "- other"), insert("\n  - "));
  assert.deepEqual(atEnd("skills:", "    - deeper"), insert("\n  - "));
});

test("inline object headers place children four spaces below the dash", () => {
  assert.deepEqual(atEnd("  - UI:"), insert("\n      - "));
  assert.deepEqual(atEnd("  - UI:", "      - MUI"), insert("\n      - "));
  assert.deepEqual(atEnd("- UI:"), insert("\n    - "));
});

test("a bare dash opens a nested collection", () => {
  assert.deepEqual(atEnd("  -"), insert("\n    - "));
  assert.deepEqual(atEnd("  -", "    - x"), insert("\n    - "));
});

test("scalar list items continue the list", () => {
  assert.deepEqual(atEnd("  - Rust"), insert("\n  - "));
  assert.deepEqual(atEnd("- 42"), insert("\n- "));
  assert.deepEqual(atEnd('    - "UI:"'), insert("\n    - "));
  assert.deepEqual(atEnd("  - Rust   "), insert("\n  - "));
});

test("an empty list item ends the list and outdents", () => {
  assert.deepEqual(atEnd("  - "), { kind: "replaceLine", text: "" });
  assert.deepEqual(atEnd("      -   "), { kind: "replaceLine", text: "    " });
  assert.deepEqual(atEnd("- "), { kind: "replaceLine", text: "" });
});

test("inline object fields continue the object", () => {
  assert.deepEqual(atEnd("  - name: Oleh"), insert("\n    "));
});

test("assignments keep the indentation", () => {
  assert.deepEqual(atEnd("  port: 3000"), insert("\n  "));
  assert.deepEqual(atEnd("port: 3000"), insert("\n"));
});

test("other cases use the default Enter", () => {
  const fallback: EnterPlan = { kind: "default" };
  assert.deepEqual(planEnter("skills:", 3), fallback, "text after the cursor");
  assert.deepEqual(planEnter("  - Rust", 4), fallback);
  assert.deepEqual(atEnd("# comment"), fallback);
  assert.deepEqual(atEnd("  # comment:"), fallback);
  assert.deepEqual(atEnd(""), fallback);
  assert.deepEqual(atEnd("    "), fallback);
  assert.deepEqual(atEnd("\tkey:"), fallback, "tabs are invalid indentation");
});

test("cursor offsets are UTF-16 code units", () => {
  const line = "  - Олег 🚀";
  assert.deepEqual(planEnter(line, line.length), insert("\n  - "));
  assert.deepEqual(planEnter(line, line.length - 1), { kind: "default" });
});

test("blank and comment lines are skipped when mirroring", () => {
  assert.ok(isSkippableLine(""));
  assert.ok(isSkippableLine("   "));
  assert.ok(isSkippableLine("  # note"));
  assert.ok(!isSkippableLine("  - x"));
});

test("object preference indents new blocks but respects existing arrays", () => {
  const object = (line: string, next?: string) => planEnter(line, line.length, next, "object");
  assert.deepEqual(object("data:"), insert("\n  "));
  assert.deepEqual(object("  - UI:"), insert("\n      "));
  assert.deepEqual(object("  -"), insert("\n    "));
  assert.deepEqual(object("data:", "  - value"), insert("\n  - "));
  assert.deepEqual(atEnd("  - UI:", "      name: UI"), insert("\n      "));
});

test("odd indentation and quoted structural characters keep default behavior", () => {
  assert.deepEqual(atEnd("   data:"), { kind: "default" });
  assert.deepEqual(atEnd('title: "data:"'), insert("\n"));
  assert.deepEqual(atEnd('  - "key: value:"'), insert("\n  - "));
});

test("array headers with a length behave like block headers", () => {
  assert.deepEqual(atEnd("skills[2]:"), insert("\n  - "));
  assert.deepEqual(atEnd("  - items[3]:"), insert("\n      - "));
  assert.deepEqual(atEnd('"my key"[1]:'), insert("\n  - "));
});

test("table headers and text blocks continue without a marker", () => {
  assert.deepEqual(atEnd("users[2]{id,name}:"), insert("\n  "));
  assert.deepEqual(atEnd("  users{id,name}:"), insert("\n    "));
  assert.deepEqual(atEnd("  - members{name}:"), insert("\n      "));
  assert.deepEqual(atEnd("body: |"), insert("\n  "));
  assert.deepEqual(atEnd("  - |"), insert("\n    "));
  assert.deepEqual(atEnd("  - note: |"), insert("\n      "));
  // Table rows and text lines keep their indentation.
  assert.deepEqual(atEnd("  1,Oleh"), insert("\n  "));
  assert.deepEqual(atEnd("  some text: here"), insert("\n  "));
});

test("quoted keys and dotted keys are recognized", () => {
  assert.deepEqual(atEnd('"@id":'), insert("\n  - "));
  assert.deepEqual(atEnd("user.name:"), insert("\n  - "));
  assert.deepEqual(atEnd('  - "a b": 1'), insert("\n    "));
  assert.deepEqual(atEnd("  - http://host"), insert("\n  - "));
  assert.deepEqual(atEnd("  - 10:30"), insert("\n  - "));
  assert.deepEqual(atEnd("  - tags[2]: [a,b]"), insert("\n    "));
});
