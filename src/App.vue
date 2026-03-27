<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  ref,
  watch,
  TransitionGroup,
} from "vue";
import { invoke } from "@tauri-apps/api/core";
import { ask, confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import AppBrand from "./components/AppBrand.vue";
import ListFilterMenu from "./components/ListFilterMenu.vue";
import NoteRichEditor from "./components/NoteRichEditor.vue";
import TrashList from "./components/TrashList.vue";
import { TRASH_RETENTION_DAYS } from "./constants/appMeta";
import type {
  FolderRow,
  ListSortMode,
  NoteRow,
  SidebarNav,
  TrashItemRow,
} from "./types/notebook";
import { sortNotesByMode } from "./utils/listSort";
import { trashItemKey } from "./utils/trash";

const folders = ref<FolderRow[]>([]);
const notes = ref<NoteRow[]>([]);
const allNotes = ref<NoteRow[]>([]);
const trashItems = ref<TrashItemRow[]>([]);
/** 废纸篓列表选中项，格式 `folder:id` / `note:id` */
const selectedTrashKey = ref<string | null>(null);

/** 左侧主导航 */
const sidebarNav = ref<SidebarNav>("all");

const selectedFolderId = ref<string | null>(null);
const selectedNoteId = ref<string | null>(null);

const searchQuery = ref("");
const searchInputRef = ref<HTMLInputElement | null>(null);

const listSortMode = ref<ListSortMode>("updated_desc");
const listFilterOnlyWithText = ref(false);
const listFilterOpen = ref(false);
const listFilterRootRef = ref<HTMLElement | null>(null);

const editorHtml = ref("<p></p>");
const noteTitleDraft = ref("");
let contentSaveTimer: ReturnType<typeof setTimeout> | null = null;
let titleSaveTimer: ReturnType<typeof setTimeout> | null = null;
/** 用户是否改过正文/标题（用于切换笔记时避免无改动也写库刷 updatedAt） */
const noteContentDirty = ref(false);
const noteTitleDirty = ref(false);
/** 加载笔记内容后忽略富文本同步事件，避免误标脏 */
const editorIgnoreUpdates = ref(false);
let editorIgnoreTimer: ReturnType<typeof setTimeout> | null = null;

const errorMsg = ref("");

/** 点击文件夹时播放一次突进动画 */
const folderSelectAnimId = ref<string | null>(null);
let folderSelectAnimTimer: ReturnType<typeof setTimeout> | null = null;

type ContextMenuState =
  | { kind: "folder"; x: number; y: number; folderId: string }
  | { kind: "note"; x: number; y: number; noteId: string };

const contextMenu = ref<ContextMenuState | null>(null);
const ctxMenuRef = ref<HTMLElement | null>(null);
const ctxMoveOpen = ref(false);
let ctxListenersCleanup: (() => void) | null = null;

const textPromptOpen = ref(false);
const textPromptTitle = ref("");
const textPromptPlaceholder = ref("");
const textPromptValue = ref("");
const textPromptInputRef = ref<HTMLInputElement | null>(null);
let textPromptResolve: ((value: string | null) => void) | null = null;

function openTextPrompt(options: {
  title: string;
  placeholder: string;
  defaultValue: string;
}): Promise<string | null> {
  return new Promise((resolve) => {
    textPromptTitle.value = options.title;
    textPromptPlaceholder.value = options.placeholder;
    textPromptValue.value = options.defaultValue;
    textPromptResolve = resolve;
    textPromptOpen.value = true;
    void nextTick(() => {
      textPromptInputRef.value?.focus();
      textPromptInputRef.value?.select();
    });
  });
}

function submitTextPrompt() {
  const v = textPromptValue.value.trim();
  textPromptOpen.value = false;
  textPromptResolve?.(v.length ? v : null);
  textPromptResolve = null;
}

function cancelTextPrompt() {
  textPromptOpen.value = false;
  textPromptResolve?.(null);
  textPromptResolve = null;
}

function onTextPromptKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    submitTextPrompt();
  }
  if (e.key === "Escape") {
    e.preventDefault();
    cancelTextPrompt();
  }
}

function showError(e: unknown) {
  errorMsg.value = e instanceof Error ? e.message : String(e);
  setTimeout(() => {
    errorMsg.value = "";
  }, 4000);
}

/** 列表时间：与「今天」同一天只显示 时:分；跨天显示 年-月-日 时:分:秒 */
function formatListTime(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (sameDay) {
    const h = String(d.getHours()).padStart(2, "0");
    const m = String(d.getMinutes()).padStart(2, "0");
    return `${h}:${m}`;
  }
  const y = d.getFullYear();
  const mo = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const mi = String(d.getMinutes()).padStart(2, "0");
  const s = String(d.getSeconds()).padStart(2, "0");
  return `${y}-${mo}-${day} ${hh}:${mi}:${s}`;
}

function formatWordStat(units: number): string {
  if (units <= 0) return "";
  if (units >= 10000) return `${(units / 10000).toFixed(1).replace(/\.0$/, "")} 万字`;
  return `${units} 字`;
}

const folderById = computed(() => {
  const m = new Map<string, FolderRow>();
  for (const f of folders.value) m.set(f.id, f);
  return m;
});

const displayedNotes = computed(() => {
  if (sidebarNav.value === "trash") {
    return [];
  }
  const q = searchQuery.value.trim().toLowerCase();
  const filterBySearch = (arr: NoteRow[]) => {
    if (!q) return arr;
    return arr.filter((n) => {
      const t = (n.title + " " + (n.preview ?? "")).toLowerCase();
      return t.includes(q);
    });
  };

  let base: NoteRow[];
  if (sidebarNav.value === "favorites") {
    base = filterBySearch(allNotes.value.filter((n) => n.isFavorite));
  } else if (q) {
    base = allNotes.value.filter((n) => {
      const t = (n.title + " " + (n.preview ?? "")).toLowerCase();
      return t.includes(q);
    });
  } else if (sidebarNav.value === "all") {
    base = allNotes.value;
  } else {
    base = notes.value;
  }

  let out = base;
  if (listFilterOnlyWithText.value) {
    out = out.filter((n) => (n.bodyTextUnits ?? 0) > 0);
  }

  const preserveFolderOrder = sidebarNav.value === "folder" && !q;
  const preserveAllNotesOrder = sidebarNav.value === "all" && !q;
  if (preserveFolderOrder || preserveAllNotesOrder) {
    return out;
  }
  return sortNotesByMode(out, listSortMode.value);
});

const listPanelTitle = computed(() => {
  if (sidebarNav.value === "favorites") return "收藏";
  if (sidebarNav.value === "trash") return "废纸篓";
  const q = searchQuery.value.trim();
  if (q) return "搜索";
  if (sidebarNav.value === "all") return "全部笔记";
  if (!selectedFolderId.value) return "笔记";
  return folderById.value.get(selectedFolderId.value)?.name ?? "笔记";
});

const listPanelSubtitle = computed(() => {
  if (sidebarNav.value === "favorites") {
    return `${displayedNotes.value.length} 条收藏`;
  }
  if (sidebarNav.value === "trash") {
    const n = trashItems.value.length;
    return `${n} 项 · ${TRASH_RETENTION_DAYS} 天后永久删除`;
  }
  if (searchQuery.value.trim()) {
    return `在所有笔记中搜索 · ${displayedNotes.value.length} 条`;
  }
  if (sidebarNav.value === "all") {
    return `${displayedNotes.value.length} 条笔记`;
  }
  if (selectedFolderId.value) {
    return `${displayedNotes.value.length} 条笔记`;
  }
  return "";
});

const noteDraggable = computed(
  () =>
    (sidebarNav.value === "folder" || sidebarNav.value === "all") &&
    !searchQuery.value.trim() &&
    !listFilterOnlyWithText.value,
);

const showNoteFolderTag = computed(
  () =>
    sidebarNav.value === "all" ||
    sidebarNav.value === "favorites" ||
    !!searchQuery.value.trim(),
);

async function toggleNoteFavorite(noteId: string) {
  const n = allNotes.value.find((x) => x.id === noteId);
  if (!n) return;
  const favorite = !n.isFavorite;
  try {
    await invoke<NoteRow>("set_note_favorite", { noteId, favorite });
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

const FOLDER_DOT_COLORS = ["#3b82f6", "#64748b", "#ec4899", "#f59e0b", "#10b981"];

function folderDotColor(index: number): string {
  return FOLDER_DOT_COLORS[index % FOLDER_DOT_COLORS.length] ?? "#3b82f6";
}

function goAllNotes() {
  sidebarNav.value = "all";
}

function goFavorites() {
  sidebarNav.value = "favorites";
}

function goTrash() {
  sidebarNav.value = "trash";
}

async function refreshTrash() {
  try {
    trashItems.value = await invoke<TrashItemRow[]>("list_trash_items");
  } catch (e) {
    showError(e);
  }
}

async function restoreTrashItem(item: TrashItemRow, e?: Event) {
  e?.stopPropagation();
  try {
    if (item.kind === "folder") {
      await invoke("restore_trash_folder", { id: item.id });
    } else {
      await invoke("restore_trash_note", { id: item.id });
    }
    await reloadAll();
    selectedTrashKey.value = null;
  } catch (err) {
    showError(err);
  }
}

function selectTrashRow(item: TrashItemRow) {
  selectedTrashKey.value = trashItemKey(item);
}

const selectedNote = computed(() => {
  if (!selectedNoteId.value) return null;
  return (
    allNotes.value.find((n) => n.id === selectedNoteId.value) ??
    notes.value.find((n) => n.id === selectedNoteId.value) ??
    null
  );
});

const ctxMenuPosition = computed(() => {
  const m = contextMenu.value;
  if (!m) return { left: 0, top: 0 };
  const pad = 8;
  const mw = 216;
  const mh = 320;
  let left = m.x;
  let top = m.y;
  if (left + mw > window.innerWidth - pad) left = window.innerWidth - mw - pad;
  if (top + mh > window.innerHeight - pad) top = window.innerHeight - mh - pad;
  if (left < pad) left = pad;
  if (top < pad) top = pad;
  return { left, top };
});

const ctxNoteRow = computed(() => {
  const m = contextMenu.value;
  if (!m || m.kind !== "note") return null;
  return allNotes.value.find((n) => n.id === m.noteId) ?? null;
});

const ctxMoveTargetFolders = computed(() => {
  const n = ctxNoteRow.value;
  if (!n) return [];
  return folders.value.filter((f) => f.id !== n.folderId);
});

function detachContextMenuListeners() {
  ctxListenersCleanup?.();
  ctxListenersCleanup = null;
}

function closeContextMenu() {
  ctxMoveOpen.value = false;
  contextMenu.value = null;
  detachContextMenuListeners();
}

function attachContextMenuListeners() {
  detachContextMenuListeners();
  const onDocDown = (e: MouseEvent) => {
    const root = ctxMenuRef.value;
    if (root && !root.contains(e.target as Node)) {
      closeContextMenu();
    }
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") closeContextMenu();
  };
  const onScroll = () => closeContextMenu();
  document.addEventListener("mousedown", onDocDown, true);
  document.addEventListener("keydown", onKey, true);
  window.addEventListener("scroll", onScroll, true);
  ctxListenersCleanup = () => {
    document.removeEventListener("mousedown", onDocDown, true);
    document.removeEventListener("keydown", onKey, true);
    window.removeEventListener("scroll", onScroll, true);
  };
}

function openFolderContextMenu(e: MouseEvent, folderId: string) {
  e.preventDefault();
  e.stopPropagation();
  closeContextMenu();
  contextMenu.value = { kind: "folder", x: e.clientX, y: e.clientY, folderId };
  void nextTick(() => attachContextMenuListeners());
}

function openNoteContextMenu(e: MouseEvent, noteId: string) {
  e.preventDefault();
  e.stopPropagation();
  closeContextMenu();
  contextMenu.value = { kind: "note", x: e.clientX, y: e.clientY, noteId };
  void nextTick(() => attachContextMenuListeners());
}

async function openNoteFromCtx(noteId: string) {
  const n = allNotes.value.find((x) => x.id === noteId);
  if (!n) return;
  closeContextMenu();
  sidebarNav.value = "folder";
  if (selectedFolderId.value !== n.folderId) {
    selectedFolderId.value = n.folderId;
    await nextTick();
  }
  selectedNoteId.value = noteId;
}

async function addNoteInFolder(folderId: string) {
  closeContextMenu();
  selectFolder(folderId);
  await nextTick();
  const title = await openTextPrompt({
    title: "新建笔记",
    placeholder: "笔记标题",
    defaultValue: "未命名笔记",
  });
  if (!title?.trim()) return;
  try {
    const row = await invoke<NoteRow>("create_note", {
      folderId,
      title: title.trim(),
    });
    await reloadAll();
    selectedNoteId.value = row.id;
  } catch (e) {
    showError(e);
  }
}

async function renameFolderById(id: string) {
  closeContextMenu();
  const f = folders.value.find((x) => x.id === id);
  if (!f) return;
  const name = await openTextPrompt({
    title: "重命名文件夹",
    placeholder: "文件夹名称",
    defaultValue: f.name,
  });
  if (name == null || !name.trim()) return;
  const trimmed = name.trim();
  if (trimmed === f.name) return;
  if (folders.value.some((x) => x.id !== id && x.name.trim() === trimmed)) {
    await message("已存在同名文件夹，请换一个名称。", {
      title: "无法重命名",
      kind: "warning",
    });
    return;
  }
  try {
    await invoke("rename_folder", { id, name: trimmed });
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

async function renameNoteById(noteId: string) {
  closeContextMenu();
  const n = allNotes.value.find((x) => x.id === noteId);
  if (!n) return;
  const title = await openTextPrompt({
    title: "重命名笔记",
    placeholder: "笔记标题",
    defaultValue: n.title,
  });
  if (title == null || !title.trim()) return;
  const t = title.trim();
  if (t === n.title) return;
  try {
    await invoke("rename_note", { id: noteId, title: t });
    if (selectedNoteId.value === noteId) {
      noteTitleDraft.value = t;
    }
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

async function duplicateNoteById(noteId: string) {
  closeContextMenu();
  const n = allNotes.value.find((x) => x.id === noteId);
  if (!n) return;
  try {
    const html = await invoke<string | null>("get_note_content", { noteId });
    const base = n.title.trim() || "未命名笔记";
    const row = await invoke<NoteRow>("create_note", {
      folderId: n.folderId,
      title: `${base} 副本`,
    });
    await invoke("save_note_content", {
      noteId: row.id,
      contentHtml: html?.trim() ? html : "<p></p>",
    });
    await reloadAll();
    sidebarNav.value = "folder";
    if (selectedFolderId.value !== n.folderId) {
      selectedFolderId.value = n.folderId;
      await nextTick();
    }
    selectedNoteId.value = row.id;
  } catch (e) {
    showError(e);
  }
}

async function moveNoteToFolderFromCtx(noteId: string, targetFolderId: string) {
  closeContextMenu();
  const n = allNotes.value.find((x) => x.id === noteId);
  if (!n || n.folderId === targetFolderId) return;
  try {
    await invoke("place_note", {
      noteId,
      targetFolderId,
      beforeNoteId: null,
    });
    await reloadAll();
    sidebarNav.value = "folder";
    if (selectedNoteId.value === noteId) {
      selectedFolderId.value = targetFolderId;
      await nextTick();
      selectedNoteId.value = noteId;
    }
  } catch (e) {
    showError(e);
  }
}

async function removeFolderFromCtx(id: string) {
  closeContextMenu();
  await removeFolder(id);
}

async function removeNoteFromCtx(id: string) {
  closeContextMenu();
  await removeNote(id);
}

async function refreshFolders() {
  folders.value = await invoke<FolderRow[]>("list_folders");
  if (!selectedFolderId.value && folders.value.length) {
    selectedFolderId.value = folders.value[0]!.id;
  }
  if (
    selectedFolderId.value &&
    !folders.value.some((f) => f.id === selectedFolderId.value)
  ) {
    selectedFolderId.value = folders.value[0]?.id ?? null;
  }
}

async function refreshNotesInFolder() {
  if (!selectedFolderId.value) {
    notes.value = [];
    return;
  }
  notes.value = await invoke<NoteRow[]>("list_notes", {
    folderId: selectedFolderId.value,
  });
}

async function refreshAllNotes() {
  const rows = await invoke<NoteRow[]>("list_all_notes");
  allNotes.value = rows.map((n) => ({
    ...n,
    bodyTextUnits: n.bodyTextUnits ?? 0,
  }));
}

async function loadNoteEditorContent(noteId: string) {
  if (editorIgnoreTimer) {
    clearTimeout(editorIgnoreTimer);
    editorIgnoreTimer = null;
  }
  editorIgnoreUpdates.value = true;
  const html = await invoke<string | null>("get_note_content", {
    noteId,
  });
  editorHtml.value = html?.trim() ? html : "<p></p>";
  await nextTick();
  await nextTick();
  editorIgnoreTimer = setTimeout(() => {
    editorIgnoreUpdates.value = false;
    editorIgnoreTimer = null;
  }, 220);
}

function flushContentSave() {
  contentSaveTimer = null;
  const id = selectedNoteId.value;
  if (!id || !noteContentDirty.value) return;
  void (async () => {
    try {
      await invoke("save_note_content", {
        noteId: id,
        contentHtml: editorHtml.value,
      });
      noteContentDirty.value = false;
      await refreshAllNotes();
      await refreshNotesInFolder();
    } catch (e) {
      showError(e);
    }
  })();
}

function scheduleContentSave() {
  if (!selectedNoteId.value) return;
  if (contentSaveTimer) clearTimeout(contentSaveTimer);
  contentSaveTimer = setTimeout(flushContentSave, 450);
}

function onEditorHtmlUpdate(v: string) {
  editorHtml.value = v;
  if (editorIgnoreUpdates.value) return;
  noteContentDirty.value = true;
  scheduleContentSave();
}

function flushTitleSave() {
  titleSaveTimer = null;
  const id = selectedNoteId.value;
  if (!id || !noteTitleDirty.value) return;
  const t = noteTitleDraft.value.trim();
  if (!t) return;
  void (async () => {
    try {
      await invoke("rename_note", { id, title: t });
      noteTitleDirty.value = false;
      await refreshAllNotes();
      await refreshNotesInFolder();
    } catch (e) {
      showError(e);
    }
  })();
}

function scheduleTitleSave() {
  if (!selectedNoteId.value) return;
  if (titleSaveTimer) clearTimeout(titleSaveTimer);
  titleSaveTimer = setTimeout(flushTitleSave, 400);
}

function onNoteTitleInput() {
  noteTitleDirty.value = true;
  scheduleTitleSave();
}

async function reloadAll() {
  try {
    await refreshFolders();
    await refreshAllNotes();
    await refreshNotesInFolder();
    await refreshTrash();
    if (selectedNoteId.value) {
      noteTitleDraft.value = selectedNote.value?.title ?? "";
      await loadNoteEditorContent(selectedNoteId.value);
      noteContentDirty.value = false;
      noteTitleDirty.value = false;
    }
  } catch (e) {
    showError(e);
  }
}

function onGlobalMousedownFilter(e: MouseEvent) {
  if (!listFilterOpen.value) return;
  const root = listFilterRootRef.value;
  if (root && e.target instanceof Node && root.contains(e.target)) return;
  listFilterOpen.value = false;
}

onMounted(() => {
  reloadAll();
  window.addEventListener("keydown", onGlobalKeydown);
  document.addEventListener("mousedown", onGlobalMousedownFilter, true);
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onGlobalMousedownFilter, true);
  detachContextMenuListeners();
  if (editorIgnoreTimer) {
    clearTimeout(editorIgnoreTimer);
    editorIgnoreTimer = null;
  }
});

function onGlobalKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
    e.preventDefault();
    searchInputRef.value?.focus();
  }
}

watch(selectedFolderId, async () => {
  selectedNoteId.value = null;
  editorHtml.value = "<p></p>";
  noteTitleDraft.value = "";
  try {
    await refreshNotesInFolder();
  } catch (e) {
    showError(e);
  }
});

watch(selectedNoteId, async (id, prevId) => {
  if (contentSaveTimer) {
    clearTimeout(contentSaveTimer);
    contentSaveTimer = null;
  }
  if (titleSaveTimer) {
    clearTimeout(titleSaveTimer);
    titleSaveTimer = null;
  }

  const leaving = prevId != null && prevId !== id;
  if (leaving) {
    try {
      if (noteContentDirty.value) {
        await invoke("save_note_content", {
          noteId: prevId,
          contentHtml: editorHtml.value,
        });
        noteContentDirty.value = false;
      }
      const t = noteTitleDraft.value.trim();
      if (noteTitleDirty.value && t) {
        await invoke("rename_note", { id: prevId, title: t });
        noteTitleDirty.value = false;
      }
      await refreshAllNotes();
      await refreshNotesInFolder();
    } catch (e) {
      showError(e);
    }
  }

  try {
    if (!id) {
      editorHtml.value = "<p></p>";
      noteTitleDraft.value = "";
      noteContentDirty.value = false;
      noteTitleDirty.value = false;
      return;
    }
    noteTitleDraft.value = selectedNote.value?.title ?? "";
    noteContentDirty.value = false;
    noteTitleDirty.value = false;
    await loadNoteEditorContent(id);
  } catch (e) {
    showError(e);
  }
});

watch(searchQuery, async (q) => {
  if (q.trim()) {
    try {
      await refreshAllNotes();
    } catch (e) {
      showError(e);
    }
  }
});

watch(sidebarNav, (nav) => {
  listFilterOpen.value = false;
  if (nav === "trash") {
    selectedNoteId.value = null;
    selectedTrashKey.value = null;
    void refreshTrash();
  } else {
    selectedTrashKey.value = null;
  }
});

function selectFolder(id: string) {
  sidebarNav.value = "folder";
  selectedFolderId.value = id;
  if (folderSelectAnimTimer) {
    clearTimeout(folderSelectAnimTimer);
    folderSelectAnimTimer = null;
  }
  folderSelectAnimId.value = null;
  void nextTick(() => {
    folderSelectAnimId.value = id;
    folderSelectAnimTimer = setTimeout(() => {
      folderSelectAnimId.value = null;
      folderSelectAnimTimer = null;
    }, 480);
  });
}

async function addFolder() {
  const name = await openTextPrompt({
    title: "新建文件夹",
    placeholder: "文件夹名称",
    defaultValue: "新建文件夹",
  });
  if (name == null || !name.trim()) return;
  const trimmed = name.trim();
  if (folders.value.some((f) => f.name.trim() === trimmed)) {
    await message("已存在同名文件夹，请换一个名称。", {
      title: "无法创建",
      kind: "warning",
    });
    return;
  }
  try {
    await invoke("create_folder", {
      name: trimmed,
    });
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

async function removeFolder(id: string) {
  if (folders.value.length <= 1) {
    await message("至少保留一个文件夹。", { title: "提示", kind: "info" });
    return;
  }
  const ok = await confirm(
    `删除文件夹将移入废纸篓（${TRASH_RETENTION_DAYS} 天内可恢复），其中笔记会一并移入，确定？`,
    {
      title: "确认删除",
      kind: "warning",
    },
  );
  if (!ok) return;
  try {
    await invoke("delete_folder", { id });
    if (selectedFolderId.value === id) selectedFolderId.value = null;
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

async function addNote() {
  if (sidebarNav.value === "favorites" || sidebarNav.value === "trash") {
    await message("请先在左侧选择「全部笔记」或某一文件夹后再新建笔记。", {
      title: "提示",
      kind: "info",
    });
    return;
  }
  const targetFolder =
    selectedFolderId.value ?? folders.value[0]?.id ?? null;
  if (!targetFolder) {
    await message("请先创建一个文件夹。", { title: "提示", kind: "info" });
    return;
  }
  const title = await openTextPrompt({
    title: "新建笔记",
    placeholder: "笔记标题",
    defaultValue: "未命名笔记",
  });
  if (!title?.trim()) return;
  try {
    const n = await invoke<NoteRow>("create_note", {
      folderId: targetFolder,
      title: title.trim(),
    });
    await reloadAll();
    sidebarNav.value = "folder";
    selectedFolderId.value = targetFolder;
    await nextTick();
    selectedNoteId.value = n.id;
  } catch (e) {
    showError(e);
  }
}

function selectNote(id: string) {
  selectedNoteId.value = id;
}

async function removeNote(id: string) {
  const ok = await ask(
    `删除后将移入废纸篓，${TRASH_RETENTION_DAYS} 天内可恢复。`,
    { title: "确认删除", kind: "warning" },
  );
  if (!ok) return;
  try {
    await invoke("delete_note", { id });
    if (selectedNoteId.value === id) selectedNoteId.value = null;
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

async function exportBook() {
  try {
    const path = await save({
      defaultPath: "我的笔记.tbook",
      filters: [{ name: "Tbook", extensions: ["tbook"] }],
    });
    if (path == null) return;
    const p = typeof path === "string" ? path : String(path);
    await invoke("export_tbook", { path: p });
  } catch (e) {
    showError(e);
  }
}

async function importBook() {
  const ok = await confirm("导入将覆盖当前所有笔记与文件夹，是否继续？", {
    title: "确认导入",
    kind: "warning",
  });
  if (!ok) return;
  try {
    const path = await open({
      multiple: false,
      filters: [{ name: "Tbook", extensions: ["tbook"] }],
    });
    if (path == null) return;
    const p = Array.isArray(path) ? path[0] : path;
    if (p == null || p === "") return;
    await invoke("import_tbook", { path: p });
    selectedNoteId.value = null;
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

function folderLabelForNote(folderId: string) {
  return folderById.value.get(folderId)?.name ?? "";
}

/** 在「全部笔记」列表中根据落点计算 place_note（同文件夹内下一笔记为插入锚点） */
function placeArgsFromAllNotesList(
  list: NoteRow[],
  targetIdx: number,
  insertBefore: boolean,
): { targetFolderId: string; beforeNoteId: string | null } {
  const target = list[targetIdx]!;
  const tf = target.folderId;
  if (insertBefore) {
    return { targetFolderId: tf, beforeNoteId: target.id };
  }
  let nextSame: string | null = null;
  for (let j = targetIdx + 1; j < list.length; j++) {
    if (list[j]!.folderId === tf) {
      nextSame = list[j]!.id;
      break;
    }
  }
  return { targetFolderId: tf, beforeNoteId: nextSame };
}

const dragOverFolderId = ref<string | null>(null);

const noteListRef = ref<InstanceType<typeof TransitionGroup> | null>(null);

type NoteListSlot =
  | { kind: "note"; note: NoteRow }
  | { kind: "gap" };

function onNoteListRowClick(slot: NoteListSlot) {
  if (slot.kind === "note") selectNote(slot.note.id);
}

function onNoteListRowContextMenu(e: MouseEvent, slot: NoteListSlot) {
  if (slot.kind === "note") openNoteContextMenu(e, slot.note.id);
}

/** 指针拖拽排序（替代 HTML5 DnD，避免 WebView 中 drop 不可靠） */
const ptrDragNoteId = ref<string | null>(null);
const ptrInsertSlot = ref(0);
const ptrGhostX = ref(0);
const ptrGhostY = ref(0);
const ptrGhostW = ref(280);
const ptrGhostH = ref(72);
const ptrGrabDx = ref(0);
const ptrGrabDy = ref(0);
const ptrActiveHandle = ref<HTMLElement | null>(null);
let ptrCapturedId = 0;

const ptrDragNote = computed(() => {
  const id = ptrDragNoteId.value;
  if (!id) return null;
  return displayedNotes.value.find((n) => n.id === id) ?? null;
});

const noteListSlots = computed((): NoteListSlot[] => {
  const full = displayedNotes.value;
  const dragId = ptrDragNoteId.value;
  if (!dragId || !noteDraggable.value) {
    return full.map((note) => ({ kind: "note" as const, note }));
  }
  const base = full.filter((n) => n.id !== dragId);
  const slot = Math.min(Math.max(ptrInsertSlot.value, 0), base.length);
  const out: NoteListSlot[] = [];
  for (let i = 0; i < base.length; i++) {
    if (i === slot) out.push({ kind: "gap" });
    out.push({ kind: "note", note: base[i]! });
  }
  if (slot === base.length) out.push({ kind: "gap" });
  return out;
});

function getNoteListUl(): HTMLElement | null {
  const c = noteListRef.value;
  const el = c && "$el" in c ? (c as { $el: HTMLElement }).$el : null;
  return el ?? null;
}

function ptrRecalcInsertSlot(clientY: number) {
  const dragId = ptrDragNoteId.value;
  if (!dragId) return;
  const ul = getNoteListUl();
  if (!ul) return;
  const base = displayedNotes.value.filter((n) => n.id !== dragId);
  let slot = base.length;
  for (let i = 0; i < base.length; i++) {
    const row = ul.querySelector(
      `[data-note-id="${CSS.escape(base[i]!.id)}"]`,
    ) as HTMLElement | null;
    if (!row) continue;
    const r = row.getBoundingClientRect();
    if (clientY < r.top + r.height / 2) {
      slot = i;
      break;
    }
  }
  ptrInsertSlot.value = slot;
}

function updateFolderHoverFromPoint(clientX: number, clientY: number) {
  if (!ptrDragNoteId.value) return;
  const el = document.elementFromPoint(clientX, clientY);
  const hit = el?.closest("[data-folder-drop-id]") as HTMLElement | null;
  const id = hit?.getAttribute("data-folder-drop-id");
  dragOverFolderId.value = id?.length ? id : null;
}

function onNoteHandlePointerDown(e: PointerEvent, noteId: string) {
  if (!noteDraggable.value || e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();
  const full = displayedNotes.value;
  const from = full.findIndex((n) => n.id === noteId);
  if (from < 0) return;

  const handle = e.currentTarget as HTMLElement;
  ptrActiveHandle.value = handle;
  handle.setPointerCapture(e.pointerId);
  ptrCapturedId = e.pointerId;

  ptrDragNoteId.value = noteId;
  ptrInsertSlot.value = from;

  const row = handle.closest(".note-row") as HTMLElement | null;
  const rect = row?.getBoundingClientRect() ?? handle.getBoundingClientRect();
  ptrGrabDx.value = e.clientX - rect.left;
  ptrGrabDy.value = e.clientY - rect.top;
  ptrGhostX.value = e.clientX - ptrGrabDx.value;
  ptrGhostY.value = e.clientY - ptrGrabDy.value;
  ptrGhostW.value = Math.max(220, Math.round(rect.width));
  ptrGhostH.value = Math.max(56, Math.round(rect.height));

  document.addEventListener("pointermove", onNotePtrMove);
  document.addEventListener("pointerup", onNotePtrUp);
  document.addEventListener("pointercancel", onNotePtrUp);
}

function onNotePtrMove(e: PointerEvent) {
  if (!ptrDragNoteId.value) return;
  e.preventDefault();
  ptrGhostX.value = e.clientX - ptrGrabDx.value;
  ptrGhostY.value = e.clientY - ptrGrabDy.value;
  updateFolderHoverFromPoint(e.clientX, e.clientY);
  if (!dragOverFolderId.value) {
    ptrRecalcInsertSlot(e.clientY);
  }
}

async function onNotePtrUp(e: PointerEvent) {
  if (!ptrDragNoteId.value) return;
  document.removeEventListener("pointermove", onNotePtrMove);
  document.removeEventListener("pointerup", onNotePtrUp);
  document.removeEventListener("pointercancel", onNotePtrUp);

  const h = ptrActiveHandle.value;
  if (h && ptrCapturedId === e.pointerId) {
    try {
      h.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
  }
  ptrActiveHandle.value = null;
  ptrCapturedId = 0;

  const noteId = ptrDragNoteId.value;
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const hit = el?.closest("[data-folder-drop-id]") as HTMLElement | null;
  const dropFolder = hit?.getAttribute("data-folder-drop-id") ?? null;

  ptrDragNoteId.value = null;
  dragOverFolderId.value = null;

  if (!noteId) return;
  if (searchQuery.value.trim()) return;

  if (dropFolder?.length) {
    try {
      await invoke("place_note", {
        noteId,
        targetFolderId: dropFolder,
        beforeNoteId: null,
      });
      await reloadAll();
    } catch (err) {
      showError(err);
    }
    return;
  }

  await commitPointerListReorder(noteId);
}

async function commitPointerListReorder(noteId: string) {
  const nav = sidebarNav.value;
  if (nav !== "folder" && nav !== "all") return;

  const full = displayedNotes.value;
  const from = full.findIndex((n) => n.id === noteId);
  if (from < 0) return;
  const base = full.filter((n) => n.id !== noteId);
  const slot = Math.min(Math.max(ptrInsertSlot.value, 0), base.length);
  if (slot === from) return;

  if (slot < base.length) {
    await reorderNoteDrop(noteId, base[slot]!.id, true);
  } else {
    await reorderNoteAppend(noteId);
  }
}

async function reorderNoteDrop(
  noteId: string,
  targetNoteId: string,
  insertBefore: boolean,
) {
  if (searchQuery.value.trim()) return;
  const nav = sidebarNav.value;
  if (nav !== "folder" && nav !== "all") return;

  if (nav === "folder") {
    if (!selectedFolderId.value) return;
    const list = notes.value;
    const idx = list.findIndex((n) => n.id === targetNoteId);
    if (idx < 0) return;

    if (noteId === targetNoteId) {
      if (insertBefore) return;
      const nextId = idx + 1 < list.length ? list[idx + 1]!.id : null;
      try {
        await invoke("place_note", {
          noteId,
          targetFolderId: selectedFolderId.value,
          beforeNoteId: nextId,
        });
        await reloadAll();
      } catch (err) {
        showError(err);
      }
      return;
    }

    let beforeNoteId: string | null;
    if (insertBefore) {
      beforeNoteId = targetNoteId;
    } else {
      beforeNoteId = idx + 1 < list.length ? list[idx + 1]!.id : null;
    }
    try {
      await invoke("place_note", {
        noteId,
        targetFolderId: selectedFolderId.value,
        beforeNoteId,
      });
      await reloadAll();
    } catch (err) {
      showError(err);
    }
    return;
  }

  const list = displayedNotes.value;
  const idx = list.findIndex((n) => n.id === targetNoteId);
  if (idx < 0) return;

  if (noteId === targetNoteId) {
    if (insertBefore) return;
    const { targetFolderId, beforeNoteId } = placeArgsFromAllNotesList(
      list,
      idx,
      false,
    );
    try {
      await invoke("place_note", { noteId, targetFolderId, beforeNoteId });
      await reloadAll();
    } catch (err) {
      showError(err);
    }
    return;
  }

  const { targetFolderId, beforeNoteId } = placeArgsFromAllNotesList(
    list,
    idx,
    insertBefore,
  );
  try {
    await invoke("place_note", { noteId, targetFolderId, beforeNoteId });
    await reloadAll();
  } catch (err) {
    showError(err);
  }
}

async function reorderNoteAppend(noteId: string) {
  if (searchQuery.value.trim()) return;
  const nav = sidebarNav.value;
  if (nav !== "folder" && nav !== "all") return;

  if (nav === "folder") {
    if (!selectedFolderId.value) return;
    try {
      await invoke("place_note", {
        noteId,
        targetFolderId: selectedFolderId.value,
        beforeNoteId: null,
      });
      await reloadAll();
    } catch (err) {
      showError(err);
    }
    return;
  }

  const list = displayedNotes.value;
  if (list.length === 0) return;
  const last = list[list.length - 1]!;
  try {
    await invoke("place_note", {
      noteId,
      targetFolderId: last.folderId,
      beforeNoteId: null,
    });
    await reloadAll();
  } catch (err) {
    showError(err);
  }
}

</script>

<template>
  <div class="app-root" :class="{ 'note-pointer-dragging': ptrDragNoteId }">
    <div class="app-columns">
    <aside class="col-nav">
      <div class="col-overlay-strip col-overlay-strip--nav" data-tauri-drag-region />
      <AppBrand />
      <nav class="nav-primary" aria-label="主导航">
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'all' }"
          @click="goAllNotes"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
          </span>
          全部笔记
        </button>
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'favorites' }"
          @click="goFavorites"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3-1.9 5.8H4l4.95 3.6-1.9 5.8L12 14.6l5 5.8-1.9-5.8 4.95-3.6h-6.1L12 3z"/></svg>
          </span>
          收藏
        </button>
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'trash' }"
          @click="goTrash"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
          </span>
          废纸篓
        </button>
      </nav>
      <div class="nav-section-label">自定义文件夹</div>
      <ul class="folder-dir" role="list">
        <li
          v-for="(f, fi) in folders"
          :key="f.id"
          class="folder-dir-item"
          :class="{
            selected: sidebarNav === 'folder' && f.id === selectedFolderId,
            'drag-target': dragOverFolderId === f.id,
            'folder-select-anim': folderSelectAnimId === f.id,
          }"
          @contextmenu="openFolderContextMenu($event, f.id)"
        >
          <button
            type="button"
            class="folder-dir-hit"
            :title="f.name"
            :data-folder-drop-id="f.id"
            @click="selectFolder(f.id)"
          >
            <span
              class="folder-dot"
              :style="{ background: folderDotColor(fi) }"
              aria-hidden="true"
            />
            <span class="folder-dir-name">{{ f.name }}</span>
          </button>
          <button
            type="button"
            class="folder-dir-del"
            title="删除文件夹"
            aria-label="删除文件夹"
            @click.stop="removeFolder(f.id)"
          >
            ×
          </button>
        </li>
      </ul>
      <button type="button" class="btn-new-note" @click="addNote">+ 新建笔记</button>
      <button type="button" class="btn-text-link" @click="addFolder">新建文件夹</button>
      <div class="nav-footer">
        <button type="button" class="linkish" @click="importBook">导入 .tbook</button>
        <button type="button" class="linkish" @click="exportBook">导出 .tbook</button>
      </div>
    </aside>

    <section class="col-list">
      <div class="col-overlay-strip col-overlay-strip--list" data-tauri-drag-region />
      <header class="list-header">
        <div class="list-header-top">
          <div class="list-header-text">
            <span class="list-title">{{ listPanelTitle }}</span>
            <span v-if="listPanelSubtitle" class="list-subtitle">{{ listPanelSubtitle }}</span>
          </div>
          <div ref="listFilterRootRef" class="list-filter-anchor">
            <ListFilterMenu
              v-model:open="listFilterOpen"
              v-model:sort-mode="listSortMode"
              v-model:only-with-text="listFilterOnlyWithText"
              :disabled="sidebarNav === 'trash'"
            />
          </div>
        </div>
        <div v-if="sidebarNav !== 'trash'" class="list-search-wrap">
          <span class="list-search-ic" aria-hidden="true">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
          </span>
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            class="list-search-input"
            type="search"
            placeholder="搜索笔记…"
            autocomplete="off"
          />
        </div>
      </header>
      <TrashList
        v-if="sidebarNav === 'trash'"
        :items="trashItems"
        :selected-key="selectedTrashKey"
        @select="selectTrashRow"
        @restore="restoreTrashItem"
      />
      <div
        v-else-if="sidebarNav === 'folder' && !selectedFolderId"
        class="list-empty-hint"
      >
        请先在左侧选择一个文件夹
      </div>
      <div
        v-else-if="sidebarNav === 'favorites' && displayedNotes.length === 0"
        class="list-empty-hint"
      >
        {{
          searchQuery.trim()
            ? "没有匹配的收藏笔记"
            : "暂无收藏笔记，在笔记列表中点击星标即可收藏"
        }}
      </div>
      <TransitionGroup
        v-else
        ref="noteListRef"
        name="note-reorder"
        tag="ul"
        class="note-list note-list-cards"
      >
        <li
          v-for="slot in noteListSlots"
          :key="slot.kind === 'gap' ? '__note_gap__' : slot.note.id"
          :class="
            slot.kind === 'gap'
              ? 'note-row-gap'
              : ['note-row', { active: slot.note.id === selectedNoteId }]
          "
          :data-note-id="slot.kind === 'note' ? slot.note.id : undefined"
          @click="onNoteListRowClick(slot)"
          @contextmenu="onNoteListRowContextMenu($event, slot)"
        >
          <template v-if="slot.kind === 'gap'">
            <span class="note-row-gap-label">放在此处</span>
          </template>
          <template v-else>
            <button
              v-if="noteDraggable"
              type="button"
              class="note-drag-handle"
              title="拖动排序"
              aria-label="拖动排序"
              @pointerdown="onNoteHandlePointerDown($event, slot.note.id)"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="9" cy="7" r="1.6" fill="currentColor" />
                <circle cx="15" cy="7" r="1.6" fill="currentColor" />
                <circle cx="9" cy="12" r="1.6" fill="currentColor" />
                <circle cx="15" cy="12" r="1.6" fill="currentColor" />
                <circle cx="9" cy="17" r="1.6" fill="currentColor" />
                <circle cx="15" cy="17" r="1.6" fill="currentColor" />
              </svg>
            </button>
            <div class="note-main">
              <div class="note-title-line">
                <span class="note-title">{{ slot.note.title }}</span>
              </div>
              <p class="note-preview">
                <span v-if="showNoteFolderTag" class="folder-tag">{{
                  folderLabelForNote(slot.note.folderId)
                }}</span>
                {{ slot.note.preview || "暂无预览" }}
              </p>
            </div>
            <div class="note-aside">
              <span class="note-time">{{ formatListTime(slot.note.updatedAt) }}</span>
              <button
                type="button"
                class="note-fav"
                :class="{ 'note-fav--on': slot.note.isFavorite }"
                :title="slot.note.isFavorite ? '取消收藏' : '收藏'"
                :aria-pressed="slot.note.isFavorite"
                aria-label="收藏"
                @click.stop="toggleNoteFavorite(slot.note.id)"
              >
                <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    v-if="slot.note.isFavorite"
                    fill="currentColor"
                    d="M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
                  />
                  <path
                    v-else
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linejoin="round"
                    d="M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
                  />
                </svg>
              </button>
              <span v-if="slot.note.bodyTextUnits > 0" class="note-word-stat">{{
                formatWordStat(slot.note.bodyTextUnits)
              }}</span>
            </div>
          </template>
        </li>
      </TransitionGroup>
      <Teleport to="body">
        <div
          v-if="ptrDragNoteId && ptrDragNote"
          class="note-drag-ghost"
          :style="{
            width: ptrGhostW + 'px',
            minHeight: ptrGhostH + 'px',
            transform: `translate(${ptrGhostX}px, ${ptrGhostY}px)`,
          }"
        >
          <div class="note-drag-ghost-inner">
            <span class="note-title">{{ ptrDragNote.title }}</span>
            <p class="note-preview note-drag-ghost-preview">
              {{ ptrDragNote.preview || "暂无预览" }}
            </p>
          </div>
        </div>
      </Teleport>
    </section>

    <section class="col-main">
      <div class="col-overlay-strip col-overlay-strip--main" data-tauri-drag-region />
      <header class="main-title-bar">
        <div
          v-if="sidebarNav === 'trash'"
          class="main-title-placeholder main-title-placeholder--trash"
        >
          废纸篓中的内容无法编辑，请恢复后再查看
        </div>
        <div v-else-if="selectedNoteId" class="main-title-row">
          <button
            type="button"
            class="main-title-fav"
            :class="{ 'main-title-fav--on': selectedNote?.isFavorite }"
            :title="selectedNote?.isFavorite ? '取消收藏' : '收藏'"
            :aria-pressed="selectedNote?.isFavorite"
            aria-label="收藏"
            @click="toggleNoteFavorite(selectedNoteId!)"
          >
            <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true">
              <path
                v-if="selectedNote?.isFavorite"
                fill="currentColor"
                d="M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
              />
              <path
                v-else
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linejoin="round"
                d="M12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
              />
            </svg>
          </button>
          <input
            v-model="noteTitleDraft"
            class="main-title-input"
            type="text"
            placeholder="笔记标题"
            @input="onNoteTitleInput"
          />
        </div>
        <div v-else class="main-title-placeholder">选择一条笔记开始编辑</div>
      </header>
      <div class="editor-shell" :class="{ 'editor-shell--trash': sidebarNav === 'trash' }">
        <NoteRichEditor
          :model-value="editorHtml"
          :editable="!!selectedNoteId && sidebarNav !== 'trash'"
          @update:model-value="onEditorHtmlUpdate"
        />
      </div>
    </section>
    </div>

    <div v-if="errorMsg" class="toast">{{ errorMsg }}</div>

    <Teleport to="body">
      <div
        v-if="contextMenu"
        ref="ctxMenuRef"
        class="ctx-menu"
        role="menu"
        :style="{
          left: ctxMenuPosition.left + 'px',
          top: ctxMenuPosition.top + 'px',
        }"
        @mousedown.stop
        @contextmenu.prevent
      >
        <template v-if="contextMenu.kind === 'folder'">
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="selectFolder(contextMenu.folderId); closeContextMenu()"
          >
            打开此文件夹
          </button>
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="addNoteInFolder(contextMenu.folderId)"
          >
            在此新建笔记
          </button>
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="renameFolderById(contextMenu.folderId)"
          >
            重命名
          </button>
          <div class="ctx-sep" role="separator" />
          <button
            type="button"
            class="ctx-item danger"
            role="menuitem"
            @click="removeFolderFromCtx(contextMenu.folderId)"
          >
            删除文件夹
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="openNoteFromCtx(contextMenu.noteId)"
          >
            打开笔记
          </button>
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="renameNoteById(contextMenu.noteId)"
          >
            重命名
          </button>
          <button
            type="button"
            class="ctx-item"
            role="menuitem"
            @click="duplicateNoteById(contextMenu.noteId)"
          >
            复制副本
          </button>
          <div
            class="ctx-item ctx-item-with-sub"
            role="menuitem"
            @mouseenter="ctxMoveOpen = true"
            @mouseleave="ctxMoveOpen = false"
          >
            <span>移动到</span>
            <span class="ctx-chevron" aria-hidden="true">›</span>
            <ul
              v-show="ctxMoveOpen"
              class="ctx-submenu"
              role="menu"
              @mouseenter="ctxMoveOpen = true"
              @mouseleave="ctxMoveOpen = false"
            >
              <template v-if="ctxMoveTargetFolders.length === 0">
                <li class="ctx-sub-empty">无其他文件夹</li>
              </template>
              <template v-else>
                <li v-for="f in ctxMoveTargetFolders" :key="f.id">
                  <button
                    type="button"
                    class="ctx-sub-item"
                    role="menuitem"
                    @click="moveNoteToFolderFromCtx(contextMenu.noteId, f.id)"
                  >
                    {{ f.name }}
                  </button>
                </li>
              </template>
            </ul>
          </div>
          <div class="ctx-sep" role="separator" />
          <button
            type="button"
            class="ctx-item danger"
            role="menuitem"
            @click="removeNoteFromCtx(contextMenu.noteId)"
          >
            删除笔记
          </button>
        </template>
      </div>
    </Teleport>

    <div
      v-if="textPromptOpen"
      class="modal-backdrop"
      role="dialog"
      aria-modal="true"
      :aria-label="textPromptTitle"
      @click.self="cancelTextPrompt"
    >
      <div class="modal-panel">
        <div class="modal-title">{{ textPromptTitle }}</div>
        <input
          ref="textPromptInputRef"
          v-model="textPromptValue"
          class="modal-input"
          type="text"
          :placeholder="textPromptPlaceholder"
          @keydown="onTextPromptKeydown"
        />
        <div class="modal-actions">
          <button type="button" class="modal-btn secondary" @click="cancelTextPrompt">
            取消
          </button>
          <button type="button" class="modal-btn primary" @click="submitTextPrompt">
            确定
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.app-root {
  --overlay-titlebar-h: 38px;
  --overlay-traffic-safe-left: 76px;
  --bg-side: #f5f7fa;
  --bg-middle: #e1e8f2;
  --bg: #ffffff;
  --line: #e5e7eb;
  --text: #111827;
  --muted: #6b7280;
  --accent: #2563eb;
  --accent-soft: #dbeafe;
  --accent-strong: #1d4ed8;
  --accent-sidebar-active: #e8f1fe;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  flex: 1;
  align-self: stretch;
  width: 100%;
  min-width: 0;
  min-height: 0;
  height: 100%;
  margin: 0;
  overflow: hidden;
  color: var(--text);
  font-family:
    ui-sans-serif,
    system-ui,
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    Roboto,
    "Helvetica Neue",
    Arial,
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    sans-serif;
  font-size: 14px;
}

.app-columns {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

.col-nav {
  width: 268px;
  min-width: 240px;
  max-width: 300px;
  background: var(--bg-side);
  border-right: 1px solid var(--line);
  display: flex;
  flex-direction: column;
  padding: 10px 14px 16px;
  gap: 12px;
  min-height: 0;
  overflow-x: hidden;
}

.col-overlay-strip {
  flex-shrink: 0;
  height: calc(var(--overlay-titlebar-h) + 4px);
  user-select: none;
  cursor: grab;
  -webkit-user-select: none;
}

.col-overlay-strip:active {
  cursor: grabbing;
}

.col-overlay-strip--nav {
  background: var(--bg-side);
}

.col-overlay-strip--list {
  background: var(--bg-middle);
}

.col-overlay-strip--main {
  background: var(--bg);
}

.nav-primary {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-pill {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: none;
  border-radius: 12px;
  padding: 10px 12px;
  background: transparent;
  color: var(--text);
  font-size: 14px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  text-align: left;
  transition: background 0.15s ease;
}

.nav-pill:hover {
  background: rgba(255, 255, 255, 0.75);
}

.nav-pill.active {
  background: var(--accent-sidebar-active);
  color: var(--accent);
  font-weight: 600;
}

.nav-pill-ic {
  display: flex;
  color: var(--muted);
  flex-shrink: 0;
}

.nav-pill.active .nav-pill-ic {
  color: var(--accent);
}

.nav-section-label {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: #9ca3af;
  padding: 8px 8px 4px;
}

.folder-dir {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-x: hidden;
  overflow-y: auto;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.folder-dir-item {
  display: flex;
  align-items: stretch;
  border-radius: 10px;
  position: relative;
  overflow: hidden;
  transition:
    background 0.15s ease,
    box-shadow 0.15s ease;
}

.folder-dir-item:not(.selected):hover {
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}

.folder-dir-item.selected {
  background: #fff;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}

.folder-dir-item.selected::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  border-radius: 0 5px 5px 0;
  background: var(--accent);
  box-shadow: 2px 0 12px rgba(37, 99, 235, 0.35);
  z-index: 0;
  pointer-events: none;
}

.folder-dir-item.drag-target {
  box-shadow: inset 0 0 0 2px rgba(0, 122, 255, 0.55);
  background: rgba(0, 122, 255, 0.07);
}

.folder-dir-item.selected.drag-target {
  box-shadow:
    0 1px 3px rgba(0, 0, 0, 0.06),
    inset 0 0 0 2px rgba(0, 122, 255, 0.55);
}

@keyframes folder-select-nudge {
  0% {
    transform: translateX(-5px) scale(0.988);
  }
  45% {
    transform: translateX(4px) scale(1.008);
  }
  100% {
    transform: translateX(0) scale(1);
  }
}

.folder-dir-item.folder-select-anim {
  animation: folder-select-nudge 0.44s cubic-bezier(0.22, 1.12, 0.36, 1);
  contain: layout style;
}

.folder-dir-hit {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  border: none;
  background: transparent;
  cursor: pointer;
  text-align: left;
  padding: 9px 4px 9px 12px;
  border-radius: 0;
  font-size: 13px;
  color: var(--text);
  font-family: inherit;
  position: relative;
  z-index: 1;
}

.folder-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.9);
}

.folder-dir-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}

.folder-dir-del {
  flex-shrink: 0;
  width: 32px;
  border: none;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  padding: 0 6px 0 0;
  border-radius: 0;
  opacity: 0;
  transition: opacity 0.12s ease;
  position: relative;
  z-index: 1;
  align-self: stretch;
  display: flex;
  align-items: center;
  justify-content: center;
}

.folder-dir-item:hover .folder-dir-del,
.folder-dir-item.selected .folder-dir-del {
  opacity: 1;
}

.folder-dir-del:hover {
  color: #ff3b30;
}

.btn-new-note {
  margin-top: 4px;
  width: 100%;
  border: none;
  border-radius: 12px;
  padding: 12px 14px;
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  color: #fff;
  background: linear-gradient(180deg, #1e40af 0%, #1d4ed8 100%);
  box-shadow: 0 2px 8px rgba(29, 78, 216, 0.35);
  transition:
    transform 0.12s ease,
    box-shadow 0.12s ease;
}

.btn-new-note:hover {
  box-shadow: 0 4px 14px rgba(29, 78, 216, 0.45);
  transform: translateY(-1px);
}

.btn-text-link {
  border: none;
  background: none;
  color: var(--accent);
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  text-align: center;
  padding: 6px 4px;
}

.btn-text-link:hover {
  text-decoration: underline;
}

.nav-footer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px solid var(--line);
}

.linkish {
  border: none;
  background: none;
  color: var(--accent);
  cursor: pointer;
  font-size: 13px;
  text-align: left;
  padding: 4px 2px;
}

.col-list {
  width: 340px;
  min-width: 300px;
  max-width: 400px;
  background: var(--bg-middle);
  border-right: 1px solid #cdd6e4;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.list-header {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 10px 16px 14px;
  flex-shrink: 0;
  background: var(--bg-middle);
}

.list-header-top {
  display: flex;
  align-items: center;
  gap: 10px;
}

.list-filter-anchor {
  flex-shrink: 0;
  position: relative;
}

.list-header-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.list-title {
  font-weight: 700;
  font-size: 18px;
  letter-spacing: -0.02em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.list-subtitle {
  font-size: 12px;
  color: var(--muted);
}

.list-search-wrap {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  height: 40px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.88);
  border: 1px solid #c5d0e0;
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
}

.list-search-ic {
  display: flex;
  color: #9ca3af;
  flex-shrink: 0;
}

.list-search-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  font-size: 14px;
  font-family: inherit;
  outline: none;
  color: var(--text);
}

.list-search-input::placeholder {
  color: #9ca3af;
}

.list-empty-hint {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px 20px;
  font-size: 13px;
  color: var(--muted);
  text-align: center;
  line-height: 1.6;
}

.note-list {
  list-style: none;
  margin: 0;
  padding: 8px 0 20px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}

.note-list-cards .note-row {
  margin: 0 12px 10px;
}

.note-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 14px 14px 14px 18px;
  cursor: pointer;
  position: relative;
  border-radius: 14px;
  border: 1px solid rgba(197, 208, 224, 0.95);
  background: rgba(255, 255, 255, 0.58);
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.05);
  transition:
    background 0.15s ease,
    box-shadow 0.15s ease,
    border-color 0.15s ease;
}

.note-row:hover {
  background: rgba(255, 255, 255, 0.92);
  border-color: #b8c8dc;
  box-shadow: 0 2px 10px rgba(15, 23, 42, 0.07);
}

.note-row.active {
  background: #fff;
  border-color: #9eb4d0;
  box-shadow:
    0 1px 3px rgba(15, 23, 42, 0.06),
    0 6px 22px rgba(15, 23, 42, 0.1);
}

.note-row.active::before {
  content: "";
  position: absolute;
  left: 6px;
  top: 50%;
  width: 10px;
  height: 32px;
  transform: translateY(-50%);
  background-repeat: no-repeat;
  background-size: contain;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 36' fill='none'%3E%3Crect x='2.25' y='4.5' width='7.5' height='24' rx='3.6' fill='%232563eb'/%3E%3Cpath d='M6 1.5 9.6 6H2.4L6 1.5Z' fill='%231d4ed8'/%3E%3Crect x='3.4' y='9' width='1.25' height='14.5' rx='0.6' fill='%23ffffff' fill-opacity='0.85'/%3E%3Ccircle cx='6' cy='31.2' r='2.3' fill='%231d4ed8'/%3E%3C/svg%3E");
  filter: drop-shadow(0 1px 2px rgba(37, 99, 235, 0.22));
  transform-origin: 50% 70%;
  animation:
    pen-slide-in 800ms cubic-bezier(0.2, 0.85, 0.25, 1),
    pen-breathe 1.8s ease-in-out 260ms infinite;
}

@keyframes pen-slide-in {
  0% {
    opacity: 0;
    transform: translateY(-50%) translateX(-3px) scaleY(0.92);
  }
  100% {
    opacity: 1;
    transform: translateY(-50%) translateX(0) scaleY(1);
  }
}

@keyframes pen-breathe {
  0%,
  100% {
    filter: drop-shadow(0 1px 2px rgba(37, 99, 235, 0.2));
    transform: translateY(-50%) scale(1);
  }
  50% {
    filter: drop-shadow(0 2px 5px rgba(37, 99, 235, 0.3));
    transform: translateY(-50%) scale(1.04);
  }
}

@media (prefers-reduced-motion: reduce) {
  .note-row.active::before {
    animation: none;
  }
}

.note-main {
  flex: 1;
  min-width: 0;
}

.note-title-line {
  display: flex;
  align-items: center;
  min-width: 0;
}

.note-title {
  font-weight: 700;
  font-size: 14px;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: -0.01em;
}

.note-aside {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  flex-shrink: 0;
  padding-top: 1px;
}

.note-time {
  font-size: 11px;
  font-weight: 500;
  color: var(--muted);
  flex-shrink: 0;
  line-height: 1.2;
  white-space: nowrap;
}

.note-word-stat {
  font-size: 10px;
  font-weight: 600;
  color: #9ca3af;
  line-height: 1.2;
  white-space: nowrap;
}

.note-preview {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--muted);
  line-height: 1.45;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  overflow: hidden;
}

.folder-tag {
  display: inline-block;
  margin-right: 6px;
  padding: 2px 8px;
  border-radius: 6px;
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.02em;
  text-transform: uppercase;
}

.note-fav {
  flex-shrink: 0;
  border: none;
  background: transparent;
  cursor: pointer;
  padding: 4px;
  color: var(--muted);
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.45;
  transition:
    opacity 0.12s ease,
    color 0.12s ease,
    background 0.12s ease;
}

.note-row:hover .note-fav,
.note-fav--on {
  opacity: 1;
}

.note-fav--on {
  color: #f59e0b;
}

.note-fav:hover {
  background: rgba(245, 158, 11, 0.12);
}

.app-root.note-pointer-dragging {
  user-select: none;
  -webkit-user-select: none;
}

.note-drag-handle {
  flex-shrink: 0;
  border: none;
  background: transparent;
  cursor: grab;
  padding: 4px 4px 4px 2px;
  margin: 0 4px 0 0;
  color: #94a3b8;
  touch-action: none;
  border-radius: 8px;
  align-self: flex-start;
  margin-top: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.note-drag-handle:hover {
  color: var(--accent);
  background: rgba(37, 99, 235, 0.08);
}

.note-drag-handle:active {
  cursor: grabbing;
}

.note-row-gap {
  list-style: none;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 52px;
  margin: 0 12px 10px;
  padding: 10px 12px;
  border-radius: 14px;
  border: 2px dashed rgba(37, 99, 235, 0.42);
  background: rgba(37, 99, 235, 0.06);
  box-sizing: border-box;
}

.note-row-gap-label {
  font-size: 12px;
  font-weight: 600;
  color: rgba(37, 99, 235, 0.8);
  letter-spacing: 0.06em;
}

.note-reorder-move {
  transition: transform 0.26s cubic-bezier(0.25, 0.82, 0.3, 1);
}

.note-reorder-enter-active,
.note-reorder-leave-active {
  transition: none;
}

@media (prefers-reduced-motion: reduce) {
  .note-reorder-move {
    transition: none;
  }
}

.note-drag-ghost {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 100002;
  pointer-events: none;
  box-sizing: border-box;
  padding: 12px 14px;
  border-radius: 14px;
  background: #fff;
  border: 1px solid #b8c5d8;
  box-shadow:
    0 12px 40px rgba(15, 23, 42, 0.18),
    0 4px 12px rgba(37, 99, 235, 0.1);
  opacity: 0.95;
}

.note-drag-ghost-inner {
  min-width: 0;
}

.note-drag-ghost-preview {
  margin-top: 6px;
  margin-bottom: 0;
}

.col-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--bg);
}

.main-title-bar {
  padding: 10px 28px 8px;
  flex-shrink: 0;
  background: #fff;
}

.main-title-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
}

.main-title-fav {
  flex-shrink: 0;
  border: none;
  background: transparent;
  cursor: pointer;
  padding: 4px 2px;
  margin-top: 2px;
  color: var(--muted);
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    color 0.12s ease,
    background 0.12s ease;
}

.main-title-fav:hover {
  background: rgba(245, 158, 11, 0.1);
}

.main-title-fav--on {
  color: #f59e0b;
}

@supports (-webkit-touch-callout: none) {
  .col-overlay-strip--nav {
    padding-left: var(--overlay-traffic-safe-left);
  }
}

.main-title-input {
  flex: 1;
  min-width: 0;
  width: auto;
  box-sizing: border-box;
  border: none;
  background: transparent;
  font-weight: 800;
  font-size: clamp(22px, 2.4vw, 30px);
  letter-spacing: -0.03em;
  line-height: 1.2;
  color: var(--text);
  font-family: inherit;
  padding: 4px 0;
  border-radius: 6px;
}

.main-title-input:focus {
  outline: none;
}

.main-title-input::placeholder {
  color: #d1d5db;
  font-weight: 700;
}

.main-title-placeholder {
  font-weight: 700;
  font-size: clamp(18px, 2vw, 22px);
  color: #d1d5db;
  padding: 4px 0;
}

.main-title-placeholder--trash {
  color: var(--muted);
  font-weight: 600;
}

.editor-shell {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background: #fafbfc;
}

.editor-shell--trash {
  opacity: 0.55;
  pointer-events: none;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  background: #333;
  color: #fff;
  padding: 10px 16px;
  border-radius: 8px;
  font-size: 13px;
  z-index: 100;
  max-width: 80vw;
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
  padding: 20px;
}

.modal-panel {
  width: 100%;
  max-width: 400px;
  background: #fff;
  border-radius: 12px;
  padding: 18px 18px 14px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
}

.modal-title {
  font-weight: 600;
  font-size: 16px;
  margin-bottom: 12px;
}

.modal-input {
  width: 100%;
  box-sizing: border-box;
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 14px;
  font-family: inherit;
  margin-bottom: 16px;
}

.modal-input:focus {
  outline: none;
  border-color: var(--accent);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.modal-btn {
  border-radius: 8px;
  padding: 8px 16px;
  font-size: 14px;
  cursor: pointer;
  border: none;
  font-family: inherit;
}

.modal-btn.secondary {
  background: #f0f2f5;
  color: var(--text);
}

.modal-btn.primary {
  background: var(--accent);
  color: #fff;
  font-weight: 600;
}

.ctx-menu {
  --ctx-line: #e8eaed;
  --ctx-text: #1a1a1a;
  --ctx-muted: #8a8f98;
  position: fixed;
  z-index: 10000;
  min-width: 200px;
  max-width: 280px;
  padding: 6px 0;
  margin: 0;
  list-style: none;
  background: #fff;
  border-radius: 10px;
  box-shadow:
    0 4px 24px rgba(0, 0, 0, 0.12),
    0 0 1px rgba(0, 0, 0, 0.08);
  border: 1px solid var(--ctx-line);
  font-family:
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    Roboto,
    "Helvetica Neue",
    Arial,
    "PingFang SC",
    "Hiragino Sans GB",
    "Microsoft YaHei",
    sans-serif;
  font-size: 15px;
  line-height: 1.6;
  font-weight: 400;
}

.ctx-item {
  display: flex;
  align-items: center;
  width: 100%;
  box-sizing: border-box;
  border: none;
  background: transparent;
  padding: 9px 14px;
  font-size: inherit;
  color: var(--ctx-text);
  text-align: left;
  cursor: pointer;
  font-family: inherit;
  gap: 8px;
}

.ctx-item:hover,
.ctx-item:focus-visible {
  background: rgba(0, 122, 255, 0.08);
  outline: none;
}

.ctx-item.danger {
  color: #c41e1e;
}

.ctx-item.danger:hover,
.ctx-item.danger:focus-visible {
  background: rgba(196, 30, 30, 0.08);
}

.ctx-sep {
  height: 1px;
  margin: 6px 10px;
  background: var(--ctx-line);
}

.ctx-item-with-sub {
  position: relative;
  justify-content: space-between;
}

.ctx-chevron {
  color: var(--ctx-muted);
  font-size: 1.1em;
  line-height: 1;
}

.ctx-submenu {
  position: absolute;
  left: 100%;
  top: -6px;
  margin-left: -10px;
  padding: 6px 0 6px 12px;
  min-width: 140px;
  max-width: 220px;
  max-height: 240px;
  overflow-y: auto;
  list-style: none;
  margin: 0;
  background: #fff;
  border-radius: 10px;
  box-shadow:
    0 4px 24px rgba(0, 0, 0, 0.12),
    0 0 1px rgba(0, 0, 0, 0.08);
  border: 1px solid var(--ctx-line);
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
}

.ctx-sub-item {
  display: block;
  width: 100%;
  box-sizing: border-box;
  border: none;
  background: transparent;
  padding: 8px 12px;
  font-size: inherit;
  text-align: left;
  cursor: pointer;
  font-family: inherit;
  color: var(--ctx-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ctx-sub-item:hover,
.ctx-sub-item:focus-visible {
  background: rgba(0, 122, 255, 0.08);
  outline: none;
}

.ctx-sub-empty {
  padding: 10px 12px;
  font-size: inherit;
  color: var(--ctx-muted);
  cursor: default;
}
</style>

<style>
/* 与 tauri.conf.json 中 backgroundColor、侧栏 --bg-side 一致 */
html {
  margin: 0;
  min-height: 100%;
  height: 100%;
  background: #f5f7fa;
}

body {
  margin: 0;
  min-height: 100%;
  height: 100%;
  background: #f5f7fa;
}

#app {
  margin: 0;
  min-height: 100%;
  height: 100%;
  background: #f5f7fa;
  display: flex;
  flex-direction: column;
  align-items: stretch;
}
</style>
