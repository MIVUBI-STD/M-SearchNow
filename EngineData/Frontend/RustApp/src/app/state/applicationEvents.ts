import type { AppSettings } from "../shared/types";
import {
  publishApplicationChanges,
  settingsChangeSet,
  type ApplicationChangeSet,
} from "./applicationChanges";

export type ApplicationEvent = {
  kind: "settingsChanged";
  previous: AppSettings | null;
  next: AppSettings;
};

export function changesForApplicationEvent(event: ApplicationEvent): ApplicationChangeSet {
  return settingsChangeSet(event.previous, event.next);
}

export function publishApplicationEvent(event: ApplicationEvent): void {
  publishApplicationChanges(changesForApplicationEvent(event));
}
