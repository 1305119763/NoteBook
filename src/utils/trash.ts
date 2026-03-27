import { TRASH_RETENTION_DAYS } from "../constants/appMeta";
import type { TrashItemRow } from "../types/notebook";

export function trashRemainingDays(deletedAtIso: string): number {
  const del = new Date(deletedAtIso);
  if (Number.isNaN(del.getTime())) return 0;
  const expire = del.getTime() + TRASH_RETENTION_DAYS * 24 * 60 * 60 * 1000;
  const ms = expire - Date.now();
  return Math.max(0, Math.ceil(ms / (24 * 60 * 60 * 1000)));
}

export function trashItemKey(item: TrashItemRow): string {
  return `${item.kind}:${item.id}`;
}
