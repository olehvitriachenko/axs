// Smart Enter for AXS, kept free of the `vscode` module so it can be unit tested.
//
// Strategy: Enter mirrors existing children; a new block uses the configured
// collection style (arrays by default). This is an editing preference, not grammar.
//
// | Current line (cursor at end) | Result                                         |
// |------------------------------|------------------------------------------------|
// | `key:`                       | new line indented +2, with `- ` by default   |
// | `- key:`                     | new line indented +4, with `- ` by default   |
// | `-` (bare nested opener)     | new line indented +2, with `- ` by default   |
// | `- value`                    | new line `- ` at the same indent               |
// | `key[N]:` (array header)     | like `key:`, with `- ` by default              |
// | `key{a,b}:` (table header)   | new line indented +2, no marker (rows have none)|
// | `key: |` (text block)        | new line indented +2, no marker                |
// | `- key: value`               | new line indented +2 (next inline-object field)|
// | `- ` (empty item)            | dash removed, line outdented one level         |
// | `key: value`                 | new line at the same indent                    |
//
// Existing children override the preference, so adding a field to an object
// never inserts a list marker. Comments, invalid indentation, and Enter with
// text after the cursor use the editor's default behavior.

export type CollectionStyle = "array" | "object";

export type EnterPlan =
  | { kind: "default" }
  /** Insert `text` at the cursor and place the cursor at its end. */
  | { kind: "insert"; text: string }
  /** Replace the current line's text with `text`, cursor at its end. */
  | { kind: "replaceLine"; text: string };

// A bare or quoted key, optionally followed by `[N]` and a table's `{a,b}`.
const KEY = '(?:[A-Za-z_][A-Za-z0-9_.-]*|"(?:[^"\\\\]|\\\\.)*")';
const HEADER = `${KEY}(?:\\[\\d*\\])?(\\{[^}]*\\})?\\s*:\\s*$`;
const BLOCK_HEADER = new RegExp(`^${HEADER}`);
const INLINE_BLOCK_HEADER = new RegExp(`^-\\s+${HEADER}`);
const TEXT_HEADER = new RegExp(`^${KEY}\\s*:\\s+\\|\\s*$`);
const INLINE_TEXT_HEADER = new RegExp(`^-\\s+${KEY}\\s*:\\s+\\|\\s*$`);
const ITEM_TEXT = /^-\s+\|\s*$/;
const INLINE_FIELD = new RegExp(`^-\\s+${KEY}(?:\\[\\d*\\])?\\s*:\\s+\\S`);
const EMPTY_ITEM = /^-\s+$/;
const SCALAR_ITEM = /^-\s+\S/;
const LIST_ITEM = /^-(?:\s|$)/;

const INDENT = 2;

/**
 * Decides what Enter does.
 *
 * @param line Text of the cursor's line.
 * @param character Cursor offset in UTF-16 code units.
 * @param nextContentLine The next line below that is neither blank nor a
 *   comment, if any. Used to mirror an existing block's first child.
 * @param defaultCollection Style for a new block without existing children.
 */
export function planEnter(
  line: string,
  character: number,
  nextContentLine?: string,
  defaultCollection: CollectionStyle = "array"
): EnterPlan {
  const before = line.slice(0, character);
  const after = line.slice(character);

  if (after.trim() !== "") {
    return { kind: "default" };
  }

  const indent = /^ */.exec(before)?.[0].length ?? 0;
  const content = before.slice(indent);

  if (content.trim() === "" || content.startsWith("#") || content.includes("\t") || indent % INDENT !== 0) {
    return { kind: "default" };
  }

  // Table rows and text lines carry no marker, so they never get a `- `.
  const header = (childIndent: number, markerless = false): EnterPlan => ({
    kind: "insert",
    text: `\n${" ".repeat(childIndent)}${
      !markerless && childIsList(nextContentLine, childIndent, defaultCollection) ? "- " : ""
    }`
  });

  const blockHeader = BLOCK_HEADER.exec(content);
  if (blockHeader) {
    return header(indent + INDENT, blockHeader[1] !== undefined);
  }

  // `- key:` opens an inline object whose `key` value sits four spaces below
  // the dash (Grammar v3).
  const inlineHeader = INLINE_BLOCK_HEADER.exec(content);
  if (inlineHeader) {
    return header(indent + 2 * INDENT, inlineHeader[1] !== undefined);
  }

  if (TEXT_HEADER.test(content) || ITEM_TEXT.test(content)) {
    return header(indent + INDENT, true);
  }

  if (INLINE_TEXT_HEADER.test(content)) {
    return header(indent + 2 * INDENT, true);
  }

  if (content.trimEnd() === "-" && !EMPTY_ITEM.test(content)) {
    return header(indent + INDENT);
  }

  if (EMPTY_ITEM.test(content)) {
    return {
      kind: "replaceLine",
      text: " ".repeat(Math.max(0, indent - INDENT))
    };
  }

  if (INLINE_FIELD.test(content)) {
    return { kind: "insert", text: `\n${" ".repeat(indent + INDENT)}` };
  }

  if (SCALAR_ITEM.test(content)) {
    return { kind: "insert", text: `\n${" ".repeat(indent)}- ` };
  }

  return { kind: "insert", text: `\n${" ".repeat(indent)}` };
}

function childIsList(
  line: string | undefined,
  childIndent: number,
  defaultCollection: CollectionStyle
): boolean {
  if (line !== undefined) {
    const indent = /^ */.exec(line)?.[0].length ?? 0;
    if (indent === childIndent) return LIST_ITEM.test(line.slice(indent));
  }
  return defaultCollection === "array";
}

/** Returns true for lines that `nextContentLine` should skip. */
export function isSkippableLine(line: string): boolean {
  const trimmed = line.trim();
  return trimmed === "" || trimmed.startsWith("#");
}
