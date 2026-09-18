import { readdir, stat } from "node:fs/promises";
import { extname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const roots = [
  resolve(appRoot, "src"),
  resolve(appRoot, "src-tauri", "src"),
  resolve(appRoot, "../../Backend/RustCore/src"),
];
const tracked = new Set([".rs", ".svelte", ".ts"]);
const advisoryThresholds = { ".rs": 20_000, ".svelte": 18_000, ".ts": 16_000 };

async function collect(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) files.push(...await collect(path));
    else if (entry.isFile() && tracked.has(extname(entry.name))) files.push(path);
  }
  return files;
}

const largeFiles = [];
for (const root of roots) {
  for (const path of await collect(root)) {
    const extension = extname(path);
    const { size } = await stat(path);
    const threshold = advisoryThresholds[extension];
    if (size > threshold) {
      largeFiles.push({
        path: relative(appRoot, path).replaceAll("\\", "/"),
        size,
        threshold,
      });
    }
  }
}

if (largeFiles.length) {
  console.warn("Source size advisory:");
  for (const item of largeFiles) {
    console.warn(`- ${item.path}: ${item.size} bytes (advisory threshold ${item.threshold})`);
  }
  console.warn("Review responsibility/cohesion before adding more scope. Size alone is not a reason to split a file.");
} else {
  console.log("SearchNow source size advisory: no files above advisory thresholds.");
}
