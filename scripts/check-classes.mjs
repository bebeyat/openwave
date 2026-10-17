// CI check: node scripts/check-classes.mjs
// 1. Tailwind here sets `--color-*: initial`, so default-palette utilities (text-white, bg-black/40,
//    border-gray-200 ...) compile to nothing. Use token utilities or arbitrary values (text-[#fff]).
// 2. `dangerous_inner_html` may only receive the embedded SVG icon/logo constants (the CSP allows
//    inline scripts, so no other HTML may ever be injected).
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "src");
const PALETTE = "white|black|slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose";
const PREFIX = "text|bg|border|border-[xytblrse]|ring|ring-offset|fill|stroke|from|via|to|shadow|outline|divide|decoration|accent|caret|placeholder|inset-ring";
export const FORBIDDEN = new RegExp(`^(?:[^\\s]*:)?!?-?(?:${PREFIX})-(?:${PALETTE})(?:-\\d{2,3})?(?:/\\d+)?$`);
export const INNER_HTML_OK = /^(body),?$/;

export function scanSource(text, file = "") {
  const problems = [];
  text.split("\n").forEach((line, i) => {
    if (/^\s*\/\//.test(line)) return;
    for (const m of line.matchAll(/"((?:[^"\\]|\\.)*)"/g)) {
      for (const tok of m[1].split(/\s+/)) {
        if (FORBIDDEN.test(tok)) problems.push(`${file}:${i + 1}: default-palette utility \`${tok}\` renders nothing`);
      }
    }
    const h = line.match(/dangerous_inner_html:\s*(.*)$/);
    if (h && !INNER_HTML_OK.test(h[1].trim())) {
      problems.push(`${file}:${i + 1}: dangerous_inner_html must be an embedded SVG constant, got \`${h[1].trim()}\``);
    }
  });
  return problems;
}

function walk(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : p.endsWith(".rs") ? [p] : [];
  });
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  const problems = walk(root).flatMap((f) => scanSource(readFileSync(f, "utf8"), f.slice(root.length + 1)));
  problems.forEach((p) => console.error(p));
  if (problems.length) process.exit(1);
  console.log("check-classes: ok");
}
