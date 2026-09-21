import type { AppSettings } from "../shared/types";
import {
  publishApplicationChanges,
  settingsChangeSet,
  type ApplicationChangeSet,
} from "./applicationChanges";

export type ApplicationEvent =
  | {
      kind: "settingsChanged";
      previous: AppSettings | null;
      next: AppSettings;
    }
  | { kind: "packageImported" }
  | { kind: "packageReplaced" }
  | { kind: "contentRemoved" }
  | { kind: "downloadChanged" }
  | { kind: "providerStateChanged" };

export function changesForApplicationEvent(event: ApplicationEvent): ApplicationChangeSet {
  switch (event.kind) {
    case "settingsChanged":
      return settingsChangeSet(event.previous, event.next);
    case "packageImported":
    case "packageReplaced":
    case "contentRemoved":
      return { library: true, diagnostics: true };
    case "downloadChanged":
      return { downloads: true, diagnostics: true };
    case "providerStateChanged":
      return { providers: true, catalog: true, diagnostics: true };
  }
}

export function publishApplicationEvent(event: ApplicationEvent): void {
  publishApplicationChanges(changesForApplicationEvent(event));
}
