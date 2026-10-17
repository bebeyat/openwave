// Safely merge new translation keys into assets/locales/{tr,en}.json.
// Usage: node tools/add-keys.mjs keys.json   where keys.json = { "tr": {"a.b": "..."}, "en": {"a.b": "..."} }
// Read-modify-write is atomic per file; keys are sorted. Refuses to overwrite an existing key with a different value
// unless --force is passed, and refuses keys present in only one language.
import { readFileSync, writeFileSync, renameSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "assets", "locales");
const force = process.argv.includes("--force");
const input = JSON.parse(readFileSync(process.argv[2], "utf8"));
const trKeys = Object.keys(input.tr ?? {});
const enKeys = Object.keys(input.en ?? {});
const missing = [...trKeys.filter((k) => !enKeys.includes(k)), ...enKeys.filter((k) => !trKeys.includes(k))];
if (missing.length) {
  console.error("keys must exist in both languages:", missing.join(", "));
  process.exit(1);
}
for (const lang of ["tr", "en"]) {
  const file = join(root, `${lang}.json`);
  const cur = JSON.parse(readFileSync(file, "utf8"));
  for (const [k, v] of Object.entries(input[lang] ?? {})) {
    if (k in cur && cur[k] !== v && !force) {
      console.error(`[${lang}] key exists with a different value: ${k} (use --force)`);
      process.exit(1);
    }
    cur[k] = v;
  }
  const sorted = Object.fromEntries(Object.entries(cur).sort(([a], [b]) => a.localeCompare(b)));
  const tmp = `${file}.tmp`;
  writeFileSync(tmp, JSON.stringify(sorted, null, 2) + "\n");
  renameSync(tmp, file);
}
console.log(`merged ${trKeys.length} keys into tr.json and en.json`);
