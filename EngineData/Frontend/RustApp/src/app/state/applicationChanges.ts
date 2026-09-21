import type { AppSettings } from "../shared/types";

export type ApplicationChangeSet = {
  settings?: boolean;
  minecraft?: boolean;
  library?: boolean;
  catalog?: boolean;
  downloads?: boolean;
  providers?: boolean;
  diagnostics?: boolean;
};

type ApplicationChangeListener = (changes: Readonly<ApplicationChangeSet>) => void;

const listeners = new Set<ApplicationChangeListener>();

export function publishApplicationChanges(changes: ApplicationChangeSet): void {
  if (!Object.values(changes).some(Boolean)) return;
  const snapshot = Object.freeze({ ...changes });
  queueMicrotask(() => {
    for (const listener of listeners) listener(snapshot);
  });
}

export function subscribeApplicationChanges(listener: ApplicationChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function settingsChangeSet(
  previous: AppSettings | null,
  next: AppSettings,
): ApplicationChangeSet {
  const minecraftChanged =
    previous === null ||
    previous.minecraft.rootOverride !== next.minecraft.rootOverride ||
    previous.minecraft.includePreview !== next.minecraft.includePreview ||
    previous.minecraft.includeLegacyUwp !== next.minecraft.includeLegacyUwp ||
    previous.minecraft.includeDevelopmentContent !== next.minecraft.includeDevelopmentContent;

  const downloadChanged =
    previous === null ||
    previous.download.bandwidthLimitBytesPerSecond !== next.download.bandwidthLimitBytesPerSecond ||
    previous.download.defaultDirectory !== next.download.defaultDirectory;

  return {
    settings: true,
    minecraft: minecraftChanged,
    library: minecraftChanged,
    downloads: downloadChanged,
    diagnostics: true,
  };
}

export function hasApplicationChanges(changes: ApplicationChangeSet): boolean {
  return Object.values(changes).some(Boolean);
}
