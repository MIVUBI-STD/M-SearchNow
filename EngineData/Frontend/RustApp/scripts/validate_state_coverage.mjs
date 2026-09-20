import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const read = (path) => readFile(resolve(appRoot, path), "utf8");
const errors = [];

const surfaces = [
  {
    name: "Library",
    path: "src/pages/Library.svelte",
    markers: [
      "Library unavailable",
      "Scanning content",
      "Minecraft wasn't detected",
      "No matching content",
      "No Minecraft content found",
      "selectionMode",
      "Drop to inspect",
      "Library scan failed",
      "Import complete",
      "Update failed",
    ],
  },
  {
    name: "Discover",
    path: "src/pages/Discover.svelte",
    markers: [
      "Discover unavailable",
      "No content source available",
      "Searching",
      "No matching content",
      "Content unavailable",
      "Download could not start",
      "Preview only",
      "Available",
    ],
  },
  {
    name: "Downloads",
    path: "src/pages/Downloads.svelte",
    markers: [
      "Downloads unavailable",
      "Loading downloads",
      "No downloads yet",
      "No matching downloads",
      "Downloads paused",
      "Download can be retried",
      "downloadRecoveryHint",
    ],
  },
  {
    name: "Settings",
    path: "src/pages/Settings.svelte",
    markers: [
      "SettingsFeedback",
      "Settings could not be saved",
      "minecraftDirty",
      "DiagnosticsPanel",
    ],
  },
  {
    name: "Package modal",
    path: "src/components/ui/PackageInspectionModal.svelte",
    markers: [
      "Ready to install",
      "Update available",
      "Same version already installed",
      "Older package detected",
      "Installation conflict",
      "Package rejected",
      "Minecraft storage required",
    ],
  },
];

for (const surface of surfaces) {
  const text = await read(surface.path);
  for (const marker of surface.markers) {
    if (!text.includes(marker)) errors.push(`${surface.name} state coverage is missing: ${marker}`);
  }
}

const activity = await read("src/components/ui/ActivityDialog.svelte");
for (const marker of ["Recent activity", "Activity unavailable", "session-only", "aria-modal", "Escape"]) {
  if (!activity.includes(marker)) errors.push(`Activity state/accessibility contract is missing: ${marker}`);
}

if (errors.length) {
  for (const error of errors) console.error(`ERROR: ${error}`);
  process.exit(1);
}
console.log("SearchNow state coverage: PASS");
