import type { ListSortMode, NoteRow } from "../types/notebook";

export function sortNotesByMode(notes: NoteRow[], mode: ListSortMode): NoteRow[] {
  const copy = [...notes];
  switch (mode) {
    case "updated_desc":
      copy.sort(
        (a, b) =>
          new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
      );
      break;
    case "updated_asc":
      copy.sort(
        (a, b) =>
          new Date(a.updatedAt).getTime() - new Date(b.updatedAt).getTime(),
      );
      break;
    case "title_asc":
      copy.sort((a, b) => a.title.localeCompare(b.title, "zh-CN"));
      break;
    case "title_desc":
      copy.sort((a, b) => b.title.localeCompare(a.title, "zh-CN"));
      break;
    default:
      break;
  }
  return copy;
}
