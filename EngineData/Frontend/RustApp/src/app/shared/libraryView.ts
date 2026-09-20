import { localContentTypeLabel } from "./format";
import type { LocalContentItem, LocalContentType } from "./types";

export type LibraryFilter =
  | "all"
  | LocalContentType
  | "issues"
  | "duplicates"
  | "missingDependencies"
  | "outdatedDependencies";

export type LibrarySort = "nameAsc" | "nameDesc" | "type" | "status";

export type LibraryFilterContext = {
  rootFilter: string;
  filter: LibraryFilter;
  duplicateIds: Set<string>;
  missingDependencyIds: Set<string>;
  outdatedDependencyIds: Set<string>;
  query: string;
};

export function findDuplicatesForItem(
  selected: LocalContentItem | null,
  items: LocalContentItem[],
): LocalContentItem[] {
  if (!selected?.manifestUuid) return [];
  const uuid = selected.manifestUuid.toLowerCase();
  return items.filter(
    (item) =>
      item.id !== selected.id &&
      item.manifestUuid !== null &&
      item.manifestUuid.toLowerCase() === uuid,
  );
}

function compareVersionParts(left: number[], right: number[]): number {
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const leftPart = left[index] ?? 0;
    const rightPart = right[index] ?? 0;
    if (leftPart > rightPart) return 1;
    if (leftPart < rightPart) return -1;
  }
  return 0;
}

export function findOutdatedDependencyIds(items: LocalContentItem[]): Set<string> {
  const installedByRoot = new Map<string, Map<string, LocalContentItem>>();
  for (const item of items) {
    if (!item.manifestUuid) continue;
    const byUuid = installedByRoot.get(item.rootId) ?? new Map<string, LocalContentItem>();
    byUuid.set(item.manifestUuid.toLowerCase(), item);
    installedByRoot.set(item.rootId, byUuid);
  }

  const outdated = new Set<string>();
  for (const item of items) {
    const installed = installedByRoot.get(item.rootId);
    if (!installed) continue;
    if (
      item.dependencies.some((dependency) => {
        if (dependency.version.length === 0) return false;
        const target = installed.get(dependency.uuid.toLowerCase());
        if (!target || target.version.length === 0) return false;
        return compareVersionParts(target.version, dependency.version) < 0;
      })
    ) {
      outdated.add(item.id);
    }
  }
  return outdated;
}

export function findMissingDependencyIds(items: LocalContentItem[]): Set<string> {
  const installedByRoot = new Map<string, Set<string>>();
  for (const item of items) {
    if (!item.manifestUuid) continue;
    const uuids = installedByRoot.get(item.rootId) ?? new Set<string>();
    uuids.add(item.manifestUuid.toLowerCase());
    installedByRoot.set(item.rootId, uuids);
  }

  const missing = new Set<string>();
  for (const item of items) {
    const installed = installedByRoot.get(item.rootId) ?? new Set<string>();
    if (item.dependencies.some((dependency) => !installed.has(dependency.uuid.toLowerCase()))) {
      missing.add(item.id);
    }
  }
  return missing;
}

export function findDuplicateIds(items: LocalContentItem[]): Set<string> {
  const byUuid = new Map<string, LocalContentItem[]>();
  for (const item of items) {
    if (!item.manifestUuid) continue;
    const key = item.manifestUuid.toLowerCase();
    const group = byUuid.get(key) ?? [];
    group.push(item);
    byUuid.set(key, group);
  }

  const ids = new Set<string>();
  for (const group of byUuid.values()) {
    if (group.length < 2) continue;
    for (const item of group) ids.add(item.id);
  }
  return ids;
}

export function matchesLibraryFilter(
  item: LocalContentItem,
  context: LibraryFilterContext,
): boolean {
  const {
    rootFilter,
    filter,
    duplicateIds,
    missingDependencyIds,
    outdatedDependencyIds,
    query,
  } = context;

  if (rootFilter !== "all" && item.rootId !== rootFilter) return false;
  if (filter === "issues" && item.status !== "invalidMetadata") return false;
  if (filter === "duplicates" && !duplicateIds.has(item.id)) return false;
  if (filter === "missingDependencies" && !missingDependencyIds.has(item.id)) return false;
  if (filter === "outdatedDependencies" && !outdatedDependencyIds.has(item.id)) return false;
  if (
    filter !== "all" &&
    filter !== "issues" &&
    filter !== "duplicates" &&
    filter !== "missingDependencies" &&
    filter !== "outdatedDependencies" &&
    item.contentType !== filter
  ) return false;

  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return `${item.title} ${item.description ?? ""}`.toLowerCase().includes(needle);
}

export function compareLibraryItems(
  left: LocalContentItem,
  right: LocalContentItem,
  sort: LibrarySort,
): number {
  if (sort === "nameDesc") return right.title.localeCompare(left.title);
  if (sort === "type") {
    return localContentTypeLabel(left.contentType).localeCompare(localContentTypeLabel(right.contentType)) ||
      left.title.localeCompare(right.title);
  }
  if (sort === "status") {
    const leftNeedsReview = left.status === "invalidMetadata" ? 0 : 1;
    const rightNeedsReview = right.status === "invalidMetadata" ? 0 : 1;
    return leftNeedsReview - rightNeedsReview || left.title.localeCompare(right.title);
  }
  return left.title.localeCompare(right.title);
}
