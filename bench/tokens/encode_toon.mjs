// Encodes every data/*.json file as TOON into out/ and checks that it decodes
// back to the same value.
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { isDeepStrictEqual } from "node:util";
import { encode, decode } from "@toon-format/toon";

const here = new URL(".", import.meta.url);
mkdirSync(new URL("out/", here), { recursive: true });

for (const file of readdirSync(new URL("data/", here)).filter((f) => f.endsWith(".json"))) {
  const value = JSON.parse(readFileSync(new URL(`data/${file}`, here), "utf8"));
  const text = encode(value);
  if (!isDeepStrictEqual(decode(text), value)) {
    throw new Error(`${file}: TOON output does not decode to the input`);
  }
  writeFileSync(new URL(`out/${file.replace(/\.json$/, ".toon")}`, here), text);
}
