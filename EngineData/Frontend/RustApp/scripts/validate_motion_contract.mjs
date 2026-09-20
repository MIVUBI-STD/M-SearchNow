import { readdir, readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const stylesRoot = resolve(appRoot, "src/styles");
const errors = [];
const files = (await readdir(stylesRoot)).filter((name) => extname(name) === ".css");

for (const file of files) {
  const text = await readFile(resolve(stylesRoot, file), "utf8");
  if (/transition\s*:\s*all\b/i.test(text)) {
    errors.push(`${file} uses transition: all; animate explicit properties only`);
  }
  if (/scale\(0(?:\.0+)?\)/i.test(text)) {
    errors.push(`${file} animates from scale(0); use a subtle non-zero origin`);
  }
  for (const match of text.matchAll(/(?:transition-duration|animation-duration)\s*:\s*(\d+)ms/gi)) {
    if (Number(match[1]) > 300) errors.push(`${file} contains UI motion over 300ms: ${match[0]}`);
  }
}

const presentation = await readFile(resolve(stylesRoot, "presentation.css"), "utf8");
if (!presentation.includes("@media (prefers-reduced-motion: reduce)")) {
  errors.push("presentation.css must explicitly remove press movement for reduced-motion users");
}
if (!presentation.includes("@media (hover: hover) and (pointer: fine)")) {
  errors.push("pointer hover polish must be gated to fine hover-capable pointers");
}
const appCss = await readFile(resolve(stylesRoot, "app.css"), "utf8");
if (!appCss.includes("@media (prefers-reduced-motion: reduce)")) {
  errors.push("app.css must retain the global reduced-motion contract");
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow motion contract: PASS");
