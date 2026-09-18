import type {
  CatalogContentType,
  DownloadJobState,
  LocalContentType,
  MinecraftChannel,
  MinecraftStorageKind,
} from "./types";

export function formatBytes(value: number | null | undefined): string {
  if (value == null) return "Unknown size";
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let amount = value / 1024;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount >= 10 ? amount.toFixed(0) : amount.toFixed(1)} ${units[unit]}`;
}

export function formatTransferRate(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null || !Number.isFinite(bytesPerSecond) || bytesPerSecond <= 0) return "—";
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "—";
  const rounded = Math.max(0, Math.round(seconds));
  if (rounded < 60) return `${rounded}s`;
  const minutes = Math.floor(rounded / 60);
  const remainingSeconds = rounded % 60;
  if (minutes < 60) return remainingSeconds ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}

export function formatDate(timestampMs: number): string {
  if (!timestampMs) return "Unknown date";
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(timestampMs));
}

export function formatDateTime(timestampMs: number): string {
  if (!timestampMs) return "Unknown time";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(timestampMs));
}

export function localContentTypeLabel(type: LocalContentType): string {
  return {
    behaviorPack: "Behavior Pack",
    resourcePack: "Resource Pack",
    skinPack: "Skin Pack",
    world: "World",
  }[type];
}

export function catalogContentTypeLabel(type: CatalogContentType): string {
  return {
    world: "World",
    addon: "Add-On",
    resourcePack: "Resource Pack",
    skin: "Skin",
    persona: "Persona",
    other: "Other",
  }[type];
}

export function minecraftChannelLabel(channel: MinecraftChannel): string {
  return {
    stable: "Minecraft Bedrock",
    preview: "Minecraft Preview",
    custom: "Custom location",
  }[channel];
}

export function minecraftStorageKindLabel(kind: MinecraftStorageKind): string {
  return {
    gdkShared: "GDK shared storage",
    gdkUser: "GDK account storage",
    legacyUwp: "Legacy UWP storage",
    customOverride: "Manual override",
  }[kind];
}

export function downloadStateLabel(state: DownloadJobState): string {
  return {
    queued: "Queued",
    preparing: "Preparing",
    transferring: "Downloading",
    finalizing: "Finalizing",
    cancelRequested: "Cancelling",
    completed: "Finished",
    failed: "Failed",
    cancelled: "Cancelled",
    interrupted: "Interrupted",
  }[state];
}

export function progressPercent(downloaded: number, total: number | null): number | null {
  if (total == null || total <= 0) return null;
  return Math.max(0, Math.min(100, Math.round((downloaded / total) * 100)));
}
