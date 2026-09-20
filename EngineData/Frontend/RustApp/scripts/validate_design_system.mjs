import { readdir, readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const stylesRoot = resolve(appRoot, "src/styles");
const errors = [];

const semanticColorTokens = [
  "--sn-danger:",
  "--sn-danger-surface:",
  "--sn-danger-border:",
  "--sn-focus:",
  "--sn-focus-ring:",
  "--sn-warning:",
  "--sn-success:",
];
const tokens = await readFile(resolve(stylesRoot, "tokens.css"), "utf8");
for (const token of semanticColorTokens) {
  if (!tokens.includes(token)) errors.push(`Missing semantic design token: ${token}`);
}

const styleFiles = (await readdir(stylesRoot))
  .filter((name) => extname(name) === ".css" && name !== "tokens.css");

const forbiddenSemanticColors = new Map([
  ["#f0a0aa", "--sn-danger"],
  ["#60353a", "--sn-danger-border"],
  ["#271416", "--sn-danger-surface"],
]);

for (const file of styleFiles) {
  const text = await readFile(resolve(stylesRoot, file), "utf8");
  for (const [literal, token] of forbiddenSemanticColors) {
    if (text.toLowerCase().includes(literal)) {
      errors.push(`${file} hardcodes semantic color ${literal}; use ${token}`);
    }
  }
}

const workspace = await readFile(resolve(stylesRoot, "workspace.css"), "utf8");
for (const forbidden of [
  "--sn-content-width: 1360px",
  "--sn-sidebar-width: 188px",
  "--sn-page-padding-x: clamp(28px, 3vw, 48px)",
  "--sn-page-padding-y: 28px",
]) {
  if (workspace.includes(forbidden)) errors.push(`workspace.css duplicates global token: ${forbidden}`);
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow design-system contract: PASS");
