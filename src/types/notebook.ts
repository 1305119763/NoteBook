export type FolderRow = {
  id: string;
  parentId: string | null;
  name: string;
  sortOrder: number;
  createdAt: string;
  color?: string | null;
};

export type NoteRow = {
  id: string;
  folderId: string;
  title: string;
  preview: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
  isFavorite: boolean;
  /** 可统计正文字数（后端 body_text_units，不含空白） */
  bodyTextUnits: number;
  isPinned: boolean;
  pinnedAt: string | null;
};

export type TrashItemRow = {
  kind: "folder" | "note";
  id: string;
  title: string;
  deletedAt: string;
  noteCount?: number;
};

export type SidebarNav = "all" | "folder" | "favorites" | "trash";

export type ListSortMode =
  | "updated_desc"
  | "updated_asc"
  | "title_asc"
  | "title_desc";
