// Conversions between AXS core positions and editor positions, kept free of the
// `vscode` module so they can be unit tested.
//
// The AXS core reports 1-based lines and 1-based columns counted in Unicode
// code points; VS Code uses 0-based lines and UTF-16 code-unit offsets.

export interface LineSource {
  readonly lineCount: number;
  lineText(line: number): string;
}

export interface EditorRange {
  startLine: number;
  startCharacter: number;
  endLine: number;
  endCharacter: number;
}

/** UTF-16 offset of the code point at `codePoints` (clamped to the line). */
function utf16Offset(text: string, codePoints: number): number {
  let offset = 0;
  let count = 0;
  while (offset < text.length && count < codePoints) {
    const code = text.charCodeAt(offset);
    offset += code >= 0xd800 && code <= 0xdbff && offset + 1 < text.length ? 2 : 1;
    count += 1;
  }
  return offset;
}

/**
 * Converts an AXS error location into an editor range. Locations past the end
 * of the document (for example "unexpected end of file") are clamped to the
 * last line. A zero-length span still covers one character when possible, so
 * the diagnostic is visible.
 */
export function axsRange(
  source: LineSource,
  line: number | undefined,
  column: number | undefined,
  length: number | undefined
): EditorRange {
  if (line === undefined || source.lineCount === 0) {
    return { startLine: 0, startCharacter: 0, endLine: 0, endCharacter: 0 };
  }

  const lastLine = source.lineCount - 1;
  if (line - 1 > lastLine) {
    const end = source.lineText(lastLine).length;
    return {
      startLine: lastLine,
      startCharacter: Math.max(0, end - 1),
      endLine: lastLine,
      endCharacter: end
    };
  }

  const lineIndex = Math.max(0, line - 1);
  const text = source.lineText(lineIndex);
  const startPoint = Math.max(0, (column ?? 1) - 1);
  let start = utf16Offset(text, startPoint);
  const end = start + utf16Offset(text.slice(start), Math.max(1, length ?? 1));

  if (start === end && start > 0) {
    // At the end of the line: cover the last character (one code point).
    const low = text.charCodeAt(start - 1);
    const high = text.charCodeAt(start - 2);
    const pair = low >= 0xdc00 && low <= 0xdfff && high >= 0xd800 && high <= 0xdbff;
    start -= pair ? 2 : 1;
  }

  return { startLine: lineIndex, startCharacter: start, endLine: lineIndex, endCharacter: end };
}

/**
 * Minimal single replacement turning `original` into `formatted`, or
 * `undefined` when they are equal. `formatted` uses LF newlines (as `axs fmt`
 * does); it is converted to `eol` first so the document keeps its line endings.
 * Offsets are UTF-16 code units in `original`.
 */
export function formattingEdit(
  original: string,
  formatted: string,
  eol: "\n" | "\r\n"
): { start: number; end: number; text: string } | undefined {
  const target = eol === "\n" ? formatted : formatted.replace(/\n/g, "\r\n");
  if (target === original) {
    return undefined;
  }

  const limit = Math.min(original.length, target.length);
  let prefix = 0;
  while (prefix < limit && original.charCodeAt(prefix) === target.charCodeAt(prefix)) {
    prefix += 1;
  }
  let suffix = 0;
  while (
    suffix < limit - prefix &&
    original.charCodeAt(original.length - 1 - suffix) ===
      target.charCodeAt(target.length - 1 - suffix)
  ) {
    suffix += 1;
  }

  // Never split a surrogate pair or a CRLF sequence at either boundary.
  const splits = (text: string, index: number): boolean => {
    if (index <= 0 || index >= text.length) return false;
    const previous = text.charCodeAt(index - 1);
    const next = text.charCodeAt(index);
    return (
      (previous >= 0xd800 && previous <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) ||
      (previous === 0x0d && next === 0x0a)
    );
  };
  while (prefix > 0 && (splits(original, prefix) || splits(target, prefix))) prefix -= 1;
  while (
    suffix > 0 &&
    (splits(original, original.length - suffix) || splits(target, target.length - suffix))
  ) {
    suffix -= 1;
  }

  return {
    start: prefix,
    end: original.length - suffix,
    text: target.slice(prefix, target.length - suffix)
  };
}
