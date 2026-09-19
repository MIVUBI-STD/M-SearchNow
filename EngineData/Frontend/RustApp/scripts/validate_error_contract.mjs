import { readdir, readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const backendRoot = resolve(appRoot, "../../Backend/RustCore/src");
const tauriCommandsRoot = resolve(appRoot, "src-tauri/src/commands");
const productErrorsPath = resolve(appRoot, "src/app/shared/productErrors.ts");

async function collectRust(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await collectRust(path));
    else if (entry.isFile() && extname(entry.name) === ".rs") files.push(path);
  }
  return files;
}

const rustFiles = [
  ...await collectRust(backendRoot),
  ...await collectRust(tauriCommandsRoot),
];

const sourceCodes = new Set();
for (const path of rustFiles) {
  const text = await readFile(path, "utf8");
  for (const match of text.matchAll(/"([a-z][a-z0-9_]{2,})"/g)) {
    const value = match[1];
    if (value.includes("_")) sourceCodes.add(value);
  }
}

const productErrors = await readFile(productErrorsPath, "utf8");
const mappedCodes = new Set(
  [...productErrors.matchAll(/^\s{2}([a-z][a-z0-9_]+):/gm)].map((match) => match[1]),
);

const errors = [];
for (const code of mappedCodes) {
  if (!sourceCodes.has(code)) {
    errors.push(`productErrors.ts maps unknown/stale backend code: ${code}`);
  }
}

for (const code of [...sourceCodes].filter((value) => value.startsWith("package_bundle_"))) {
  if (!mappedCodes.has(code)) {
    errors.push(`transactional package bundle backend code is missing friendly frontend copy: ${code}`);
  }
}

for (const code of [
  "dialog_path_invalid",
  "directory_unavailable",
  "directory_open_failed",
  "library_item_not_found",
  "download_directory_not_ready",
  "download_directory_unavailable",
  "settings_load_failed",
  "settings_save_failed",
]) {
  if (!sourceCodes.has(code)) errors.push(`expected user-facing backend code is missing from Rust/Tauri source: ${code}`);
  if (!mappedCodes.has(code)) errors.push(`expected user-facing backend code is missing friendly frontend copy: ${code}`);
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}

console.log(`SearchNow error contract: PASS (${mappedCodes.size} friendly mappings checked)`);
