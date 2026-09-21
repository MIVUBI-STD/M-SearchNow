import type { AppSettings } from "../shared/types";

export type ApplicationChangeSet = {
  settings?: boolean;
  minecraft?: boolean;
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

  return {
    settings: true,
    minecraft: minecraftChanged,
  };
}
