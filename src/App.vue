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
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ask, confirm, message, open, save } from "@tauri-apps/plugin-dialog";
import AppBrand from "./components/AppBrand.vue";
import SettingsDialog, { type AppSettings } from "./components/SettingsDialog.vue";
import ListFilterMenu from "./components/ListFilterMenu.vue";
import NoteRichEditor from "./components/NoteRichEditor.vue";
import HistoryDrawer from "./components/HistoryDrawer.vue";
import PresentPanel from "./components/PresentPanel.vue";
import TrashList from "./components/TrashList.vue";
import { APP_DISPLAY_VERSION, TRASH_RETENTION_DAYS } from "./constants/appMeta";
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

const settings = ref<AppSettings>({
  theme: "system",
  skin: "blue",
  nickname: "",
  useDefaultNames: false,
});
const settingsPath = ref("");
const settingsOpen = ref(false);
const settingsFocus = ref<"general" | "about">("general");
const systemDark = ref(window.matchMedia("(prefers-color-scheme: dark)").matches);
const resolvedTheme = computed(() =>
  settings.value.theme === "system" ? (systemDark.value ? "dark" : "light") : settings.value.theme,
);
watch(
  [resolvedTheme, () => settings.value.skin],
  ([theme, skin]) => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.skin = skin;
  },
  { immediate: true },
);
let unlistenMenu: UnlistenFn | null = null;
let themeMedia: MediaQueryList | null = null;

function openSettings(focus: "general" | "about" = "general") {
  settingsFocus.value = focus;
  settingsOpen.value = true;
}

async function saveAppSettings(next: AppSettings) {
  try {
    const file = await invoke<{ settings: AppSettings; path: string }>("save_settings", { settings: next });
    settings.value = file.settings;
    settingsPath.value = file.path;
    settingsOpen.value = false;
    if (presentSession.value) {
      await invoke("set_present_nickname", { nickname: file.settings.nickname });
    }
  } catch (e) {
    showError(e);
  }
}

function nextDefaultName(base: string, taken: (name: string) => boolean) {
  if (!taken(base)) return base;
  let n = 2;
  while (taken(`${base} ${n}`)) n += 1;
  return `${base} ${n}`;
}

function onMenuAction(id: string) {
  if (id === "settings" || id === "tray-settings") openSettings("general");
  else if (id === "about" || id === "tray-about" || id === "about-help") openSettings("about");
  else if (id === "new-note") void addNote();
  else if (id === "new-folder") void addFolder();
  else if (id === "import") void importBook();
  else if (id === "export") void exportBook();
}

const selectedFolderId = ref<string | null>(null);
const selectedNoteId = ref<string | null>(null);

const searchQuery = ref("");
const searchInputRef = ref<HTMLInputElement | null>(null);

const listSortMode = ref<ListSortMode>("updated_desc");
const listFilterOnlyWithText = ref(false);
const listFilterOpen = ref(false);
const listFilterRootRef = ref<HTMLElement | null>(null);

const NAV_COLLAPSED_KEY = "notebook.navCollapsed";
const LIST_COLLAPSED_KEY = "notebook.listCollapsed";

function readStoredBool(key: string): boolean {
  try {
    return localStorage.getItem(key) === "1";
  } catch {
    return false;
  }
}

function writeStoredBool(key: string, value: boolean) {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    /* ignore */
  }
}

const navCollapsed = ref(readStoredBool(NAV_COLLAPSED_KEY));
const listCollapsed = ref(readStoredBool(LIST_COLLAPSED_KEY));

function toggleNavCollapsed() {
  navCollapsed.value = !navCollapsed.value;
  writeStoredBool(NAV_COLLAPSED_KEY, navCollapsed.value);
}

function toggleListCollapsed() {
  listCollapsed.value = !listCollapsed.value;
  writeStoredBool(LIST_COLLAPSED_KEY, listCollapsed.value);
}

/** 全屏编辑：隐藏左侧导航与笔记列表，仅保留编辑区 */
const editorFocusMode = ref(false);

function enterEditorFocusMode() {
  if (!selectedNoteId.value || sidebarNav.value === "trash") return;
  editorFocusMode.value = true;
}

function exitEditorFocusMode() {
  editorFocusMode.value = false;
}

function toggleEditorFocusMode() {
  if (editorFocusMode.value) exitEditorFocusMode();
  else enterEditorFocusMode();
}

type PresentPeer = { ip: string; mode: string; color: string };
type PresentSession = {
  noteId: string;
  title: string;
  url: string;
  ip: string;
  port: number;
  rev: number;
  origin: string;
  html: string;
  maxEditors: number;
  peers: PresentPeer[];
  requests: { ip: string }[];
  notices?: string[];
};

const PRESENT_MAX_KEY = "notebook.presentMaxEditors";
function readPresentMax() {
  const n = Number(localStorage.getItem(PRESENT_MAX_KEY) || "3");
  if (!Number.isFinite(n)) return 3;
  return Math.min(12, Math.max(1, Math.round(n)));
}

const historyOpen = ref(false);
const presentOpen = ref(false);
const presentMaxEditors = ref(readPresentMax());
const presentSession = ref<PresentSession | null>(null);
let presentPushTimer: ReturnType<typeof setTimeout> | null = null;
let presentPollTimer: ReturnType<typeof setInterval> | null = null;
let appliedPresentRev = -1;

function openHistory() {
  if (!selectedNoteId.value || sidebarNav.value === "trash") return;
  historyOpen.value = true;
}

async function recordRevision(noteId: string) {
  const title = noteTitleDraft.value.trim() || "未命名笔记";
  await invoke("record_note_revision", {
    noteId,
    title,
    contentHtml: editorHtml.value,
  });
}

function stripCollabHtml(html: string) {
  if (!html.includes("data-collab-ip")) return html;
  const doc = new DOMParser().parseFromString(html, "text/html");
  doc.querySelectorAll("[data-collab-ip]").forEach((el) => {
    const parent = el.parentNode;
    if (!parent) return;
    while (el.firstChild) parent.insertBefore(el.firstChild, el);
    parent.removeChild(el);
  });
  return doc.body.innerHTML;
}

function holdEditorIgnore() {
  editorIgnoreUpdates.value = true;
  if (editorIgnoreTimer) clearTimeout(editorIgnoreTimer);
  editorIgnoreTimer = setTimeout(() => {
    editorIgnoreUpdates.value = false;
    editorIgnoreTimer = null;
  }, 220);
}

function schedulePresentPush() {
  const session = presentSession.value;
  if (!session || session.noteId !== selectedNoteId.value) return;
  if (presentPushTimer) clearTimeout(presentPushTimer);
  presentPushTimer = setTimeout(() => {
    presentPushTimer = null;
    const current = presentSession.value;
    if (!current || current.noteId !== selectedNoteId.value) return;
    const title = noteTitleDraft.value.trim() || current.title || "未命名笔记";
    current.title = title;
    void invoke("update_present", {
      title,
      html: editorHtml.value,
    }).catch(() => {});
  }, 280);
}

function startPresentPoll() {
  if (presentPollTimer) return;
  presentPollTimer = setInterval(() => {
    void pullPresent();
  }, 1200);
}

function stopPresentPoll() {
  if (!presentPollTimer) return;
  clearInterval(presentPollTimer);
  presentPollTimer = null;
}

async function pullPresent() {
  if (!presentSession.value) return;
  try {
    const snap = await invoke<PresentSession | null>("present_status");
    if (!snap) {
      presentSession.value = null;
      presentOpen.value = false;
      return;
    }
    const prevRev = appliedPresentRev;
    presentSession.value = snap;
    for (const text of snap.notices ?? []) showError(text);
    if (snap.maxEditors) presentMaxEditors.value = snap.maxEditors;
    if (snap.origin === "owner" || snap.rev === prevRev) {
      appliedPresentRev = snap.rev;
      return;
    }
    appliedPresentRev = snap.rev;
    try {
      await invoke("save_note_content", { noteId: snap.noteId, contentHtml: snap.html });
      await invoke("record_note_revision", {
        noteId: snap.noteId,
        title: snap.title || "未命名笔记",
        contentHtml: snap.html,
      });
    } catch (e) {
      showError(e);
    }
    if (snap.noteId === selectedNoteId.value) {
      holdEditorIgnore();
      editorHtml.value = snap.html;
      noteContentDirty.value = false;
    }
  } catch {
    /* 演示进程已停 */
  }
}

async function openPresent() {
  const noteId = selectedNoteId.value;
  if (!noteId || sidebarNav.value === "trash") return;
  try {
    const info = await invoke<PresentSession>("start_present", {
      noteId,
      title: noteTitleDraft.value.trim() || "未命名笔记",
      html: editorHtml.value,
      maxEditors: presentMaxEditors.value,
    });
    presentSession.value = info;
    appliedPresentRev = info.rev ?? 0;
    presentOpen.value = true;
    startPresentPoll();
    void invoke("set_present_nickname", { nickname: settings.value.nickname }).catch(() => {});
  } catch (e) {
    showError(e);
  }
}

async function setPresentMax(n: number) {
  const v = Math.min(12, Math.max(1, Math.round(n) || 1));
  presentMaxEditors.value = v;
  localStorage.setItem(PRESENT_MAX_KEY, String(v));
  if (!presentSession.value) return;
  try {
    await invoke("set_present_max_editors", { maxEditors: v });
  } catch (e) {
    showError(e);
  }
}

const pendingEditRequest = computed(() => presentSession.value?.requests?.[0] ?? null);

async function revokeEdit(ip?: string) {
  if (!presentSession.value) return;
  try {
    await invoke("present_revoke", { ip: ip ?? null });
    await pullPresent();
  } catch (e) {
    showError(e);
  }
}

async function decideEdit(allow: boolean) {
  const ip = pendingEditRequest.value?.ip;
  if (!ip) return;
  try {
    await invoke("present_decide", { ip, allow });
    await pullPresent();
  } catch (e) {
    showError(e);
  }
}

async function stopPresent() {
  stopPresentPoll();
  if (presentPushTimer) {
    clearTimeout(presentPushTimer);
    presentPushTimer = null;
  }
  const session = presentSession.value;
  const noteId = session?.noteId && session.noteId === selectedNoteId.value ? selectedNoteId.value : session?.noteId;
  const raw = noteId && noteId === selectedNoteId.value ? editorHtml.value : session?.html || "";
  const stripped = stripCollabHtml(raw);
  if (noteId && stripped !== raw) {
    if (noteId === selectedNoteId.value) {
      holdEditorIgnore();
      editorHtml.value = stripped;
      noteContentDirty.value = false;
    }
    try {
      await invoke("save_note_content", { noteId, contentHtml: stripped });
      await invoke("record_note_revision", {
        noteId,
        title: (noteId === selectedNoteId.value ? noteTitleDraft.value.trim() : session?.title) || "未命名笔记",
        contentHtml: stripped,
      });
    } catch (e) {
      showError(e);
    }
  }
  try {
    await invoke("stop_present");
  } catch (e) {
    showError(e);
  }
  presentSession.value = null;
  presentOpen.value = false;
  appliedPresentRev = -1;
}

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
  const ordered = preserveFolderOrder || preserveAllNotesOrder ? out : sortNotesByMode(out, listSortMode.value);
  return liftPinned(ordered);
});

function liftPinned(arr: NoteRow[]): NoteRow[] {
  const pinned: NoteRow[] = [];
  const rest: NoteRow[] = [];
  for (const note of arr) {
    if (note.isPinned) pinned.push(note);
    else rest.push(note);
  }
  pinned.sort((a, b) => (b.pinnedAt || "").localeCompare(a.pinnedAt || ""));
  return pinned.concat(rest);
}

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

function noteById(noteId: string) {
  return allNotes.value.find((n) => n.id === noteId) ?? notes.value.find((n) => n.id === noteId) ?? null;
}

async function toggleNotePinned(noteId: string) {
  const n = noteById(noteId);
  if (!n) return;
  try {
    await invoke<NoteRow>("set_note_pinned", { noteId, pinned: !n.isPinned });
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

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

const FOLDER_DOT_COLORS = ["#3b82f6", "#64748b", "#ec4899", "#f59e0b", "#10b981", "#7c3aed", "#ea580c", "#0f172a"];

function folderDotColor(folder: FolderRow, index: number): string {
  return folder.color || FOLDER_DOT_COLORS[index % FOLDER_DOT_COLORS.length] || "#3b82f6";
}

const colorPick = ref<{ id: string; x: number; y: number } | null>(null);

function openFolderColor(e: MouseEvent, id: string) {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  colorPick.value = { id, x: rect.left, y: rect.bottom + 6 };
}

async function chooseFolderColor(color: string) {
  const id = colorPick.value?.id;
  colorPick.value = null;
  if (!id) return;
  try {
    await invoke("set_folder_color", { id, color });
    await reloadAll();
  } catch (e) {
    showError(e);
  }
}

type FolderSlot = { kind: "folder"; folder: FolderRow } | { kind: "gap" };
const ptrDragFolderId = ref<string | null>(null);
const ptrFolderSlot = ref(0);
const folderGhost = ref({ x: 0, y: 0, w: 180, name: "" });

const folderSlots = computed((): FolderSlot[] => {
  const full = folders.value;
  const dragId = ptrDragFolderId.value;
  if (!dragId) return full.map((folder) => ({ kind: "folder" as const, folder }));
  const base = full.filter((folder) => folder.id !== dragId);
  const slot = Math.min(Math.max(ptrFolderSlot.value, 0), base.length);
  const out: FolderSlot[] = [];
  for (let i = 0; i < base.length; i++) {
    if (i === slot) out.push({ kind: "gap" });
    out.push({ kind: "folder", folder: base[i]! });
  }
  if (slot === base.length) out.push({ kind: "gap" });
  return out;
});

function recalcFolderSlot(clientY: number) {
  const dragId = ptrDragFolderId.value;
  if (!dragId) return;
  const base = folders.value.filter((folder) => folder.id !== dragId);
  let slot = base.length;
  for (let i = 0; i < base.length; i++) {
    const row = document.querySelector(`[data-folder-row="${CSS.escape(base[i]!.id)}"]`);
    if (!(row instanceof HTMLElement)) continue;
    const rect = row.getBoundingClientRect();
    if (clientY < rect.top + rect.height / 2) {
      slot = i;
      break;
    }
  }
  ptrFolderSlot.value = slot;
}

function onFolderPointerDown(e: PointerEvent, id: string) {
  if (e.button !== 0) return;
  const target = e.target instanceof Element ? e.target : null;
  if (target?.closest(".folder-dot, .folder-dir-del")) return;
  const startX = e.clientX;
  const startY = e.clientY;
  const from = folders.value.findIndex((folder) => folder.id === id);
  let started = false;
  const move = (ev: PointerEvent) => {
    if (!started) {
      if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < 5) return;
      started = true;
      ptrDragFolderId.value = id;
      ptrFolderSlot.value = from < 0 ? 0 : from;
      const row = (e.currentTarget as HTMLElement).closest(".folder-dir-item");
      const rect = row instanceof HTMLElement ? row.getBoundingClientRect() : null;
      folderGhost.value = {
        x: ev.clientX - 20,
        y: ev.clientY - 16,
        w: rect ? Math.round(rect.width) : 180,
        name: folders.value.find((folder) => folder.id === id)?.name ?? "",
      };
    }
    folderGhost.value = { ...folderGhost.value, x: ev.clientX - 20, y: ev.clientY - 16 };
    recalcFolderSlot(ev.clientY);
  };
  const up = async () => {
    document.removeEventListener("pointermove", move);
    document.removeEventListener("pointerup", up);
    document.removeEventListener("pointercancel", up);
    if (!started) return;
    const dragId = ptrDragFolderId.value;
    const slot = ptrFolderSlot.value;
    const origin = folders.value.findIndex((folder) => folder.id === dragId);
    ptrDragFolderId.value = null;
    if (!dragId || slot === origin) return;
    const base = folders.value.filter((folder) => folder.id !== dragId);
    const before = slot < base.length ? base[slot]!.id : null;
    try {
      await invoke("place_folder", { folderId: dragId, beforeFolderId: before });
      await reloadAll();
    } catch (err) {
      showError(err);
    }
  };
  document.addEventListener("pointermove", move);
  document.addEventListener("pointerup", up);
  document.addEventListener("pointercancel", up);
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
  let title = "";
  if (settings.value.useDefaultNames) {
    title = nextDefaultName("未命名笔记", (name) =>
      allNotes.value.some((note) => note.folderId === folderId && note.title.trim() === name),
    );
  } else {
    const asked = await openTextPrompt({
      title: "新建笔记",
      placeholder: "笔记标题",
      defaultValue: "未命名笔记",
    });
    if (!asked?.trim()) return;
    title = asked.trim();
  }
  try {
    const row = await invoke<NoteRow>("create_note", {
      folderId,
      title,
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
      await recordRevision(id);
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
  schedulePresentPush();
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
      await recordRevision(id);
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
  schedulePresentPush();
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
  if (colorPick.value && e.target instanceof Element && !e.target.closest(".folder-color-pop, .folder-dot")) {
    colorPick.value = null;
  }
  if (!listFilterOpen.value) return;
  const root = listFilterRootRef.value;
  if (root && e.target instanceof Node && root.contains(e.target)) return;
  listFilterOpen.value = false;
}

onMounted(() => {
  reloadAll();
  themeMedia = window.matchMedia("(prefers-color-scheme: dark)");
  themeMedia.addEventListener("change", onSystemTheme);
  void invoke<{ settings: AppSettings; path: string }>("get_settings")
    .then((file) => {
      settings.value = file.settings;
      settingsPath.value = file.path;
    })
    .catch(() => {});
  void listen<string>("nb-menu", (event) => onMenuAction(event.payload)).then((stop) => {
    unlistenMenu = stop;
  });
  void invoke<PresentSession | null>("present_status")
    .then((status) => {
      if (!status) return;
      presentSession.value = status;
      appliedPresentRev = status.rev ?? 0;
      if (status.maxEditors) presentMaxEditors.value = status.maxEditors;
      startPresentPoll();
    })
    .catch(() => {});
  window.addEventListener("keydown", onGlobalKeydown);
  document.addEventListener("mousedown", onGlobalMousedownFilter, true);
});

function onSystemTheme() {
  systemDark.value = themeMedia?.matches ?? false;
}

onUnmounted(() => {
  themeMedia?.removeEventListener("change", onSystemTheme);
  unlistenMenu?.();
  unlistenMenu = null;
  void stopPresent();
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
  if (
    e.key === "Escape" &&
    editorFocusMode.value &&
    !textPromptOpen.value &&
    !contextMenu.value
  ) {
    e.preventDefault();
    exitEditorFocusMode();
  }
}

watch(selectedNoteId, (id) => {
  historyOpen.value = false;
  if (!id && editorFocusMode.value) exitEditorFocusMode();
});

watch(sidebarNav, (nav) => {
  if (nav === "trash" && editorFocusMode.value) exitEditorFocusMode();
});

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
  let trimmed = "";
  if (settings.value.useDefaultNames) {
    trimmed = nextDefaultName("新建文件夹", (name) => folders.value.some((f) => f.name.trim() === name));
  } else {
    const name = await openTextPrompt({
      title: "新建文件夹",
      placeholder: "文件夹名称",
      defaultValue: "新建文件夹",
    });
    if (name == null || !name.trim()) return;
    trimmed = name.trim();
    if (folders.value.some((f) => f.name.trim() === trimmed)) {
      await message("已存在同名文件夹，请换一个名称。", {
        title: "无法创建",
        kind: "warning",
      });
      return;
    }
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
  let title = "";
  if (settings.value.useDefaultNames) {
    title = nextDefaultName("未命名笔记", (name) =>
      allNotes.value.some((note) => note.folderId === targetFolder && note.title.trim() === name),
    );
  } else {
    const asked = await openTextPrompt({
      title: "新建笔记",
      placeholder: "笔记标题",
      defaultValue: "未命名笔记",
    });
    if (!asked?.trim()) return;
    title = asked.trim();
  }
  try {
    const n = await invoke<NoteRow>("create_note", {
      folderId: targetFolder,
      title,
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
  <div
    class="app-root"
    :data-theme="resolvedTheme"
    :data-skin="settings.skin"
    :class="{ 'note-pointer-dragging': ptrDragNoteId || ptrDragFolderId }"
  >
    <div
      class="app-columns"
      :class="{
        'nav-collapsed': navCollapsed,
        'list-collapsed': listCollapsed,
        'editor-focus-mode': editorFocusMode,
      }"
    >
    <div class="col-shell col-shell--nav">
    <aside class="col-nav">
      <div class="col-overlay-strip col-overlay-strip--nav" data-tauri-drag-region />
      <AppBrand class="col-nav-brand" />
      <nav class="nav-primary" aria-label="主导航">
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'all' }"
          title="全部笔记"
          @click="goAllNotes"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
          </span>
          <span class="nav-pill-label">全部笔记</span>
        </button>
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'favorites' }"
          title="收藏"
          @click="goFavorites"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m12 3-1.9 5.8H4l4.95 3.6-1.9 5.8L12 14.6l5 5.8-1.9-5.8 4.95-3.6h-6.1L12 3z"/></svg>
          </span>
          <span class="nav-pill-label">收藏</span>
        </button>
        <button
          type="button"
          class="nav-pill"
          :class="{ active: sidebarNav === 'trash' }"
          title="废纸篓"
          @click="goTrash"
        >
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
          </span>
          <span class="nav-pill-label">废纸篓</span>
        </button>
        <button type="button" class="nav-pill" title="软件设置" @click="openSettings('general')">
          <span class="nav-pill-ic" aria-hidden="true">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
          </span>
          <span class="nav-pill-label">软件设置</span>
        </button>
      </nav>
      <div class="col-nav-expandable">
      <div class="nav-section-label">自定义文件夹</div>
      <ul class="folder-dir" role="list">
        <li
          v-for="slot in folderSlots"
          :key="slot.kind === 'gap' ? '__folder_gap__' : slot.folder.id"
          class="folder-dir-item"
          :class="slot.kind === 'gap'
            ? 'folder-row-gap'
            : {
                selected: sidebarNav === 'folder' && slot.folder.id === selectedFolderId,
                'drag-target': dragOverFolderId === slot.folder.id,
                'folder-select-anim': folderSelectAnimId === slot.folder.id,
                'is-dragging': ptrDragFolderId === slot.folder.id,
              }"
          :data-folder-row="slot.kind === 'folder' ? slot.folder.id : undefined"
          @contextmenu="slot.kind === 'folder' && openFolderContextMenu($event, slot.folder.id)"
          @pointerdown="slot.kind === 'folder' && onFolderPointerDown($event, slot.folder.id)"
        >
          <template v-if="slot.kind === 'gap'">
            <span class="folder-row-gap-label">放在此处</span>
          </template>
          <template v-else>
            <button
              type="button"
              class="folder-dir-hit"
              :title="slot.folder.name"
              :data-folder-drop-id="slot.folder.id"
              @click="selectFolder(slot.folder.id)"
            >
              <span
                class="folder-dot"
                :style="{ background: folderDotColor(slot.folder, folders.findIndex((item) => item.id === slot.folder.id)) }"
                role="button"
                tabindex="0"
                title="选择颜色"
                @click.stop="openFolderColor($event, slot.folder.id)"
                @keydown.enter.stop.prevent="openFolderColor($event as unknown as MouseEvent, slot.folder.id)"
              />
              <span class="folder-dir-name">{{ slot.folder.name }}</span>
            </button>
            <button
              type="button"
              class="folder-dir-del"
              title="删除文件夹"
              aria-label="删除文件夹"
              @click.stop="removeFolder(slot.folder.id)"
            >
              ×
            </button>
          </template>
        </li>
      </ul>
      </div>
      <div v-if="!navCollapsed" class="col-nav-actions">
        <button type="button" class="btn-new-note" @click="addNote">+ 新建笔记</button>
        <button type="button" class="btn-text-link" @click="addFolder">新建文件夹</button>
        <div class="nav-footer">
          <button type="button" class="linkish" @click="importBook">导入 .tbook</button>
          <button type="button" class="linkish" @click="exportBook">导出 .tbook</button>
        </div>
        <p class="nav-version">NoteBook v{{ APP_DISPLAY_VERSION }}</p>
      </div>
      <button
        v-if="navCollapsed"
        type="button"
        class="nav-rail-new"
        title="新建笔记"
        aria-label="新建笔记"
        @click="addNote"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      </button>
    </aside>
    </div>

    <div class="col-splitter col-splitter--nav">
      <button
        type="button"
        class="col-splitter-btn"
        :title="navCollapsed ? '展开导航栏' : '收起导航栏'"
        :aria-label="navCollapsed ? '展开导航栏' : '收起导航栏'"
        :aria-expanded="!navCollapsed"
        @click="toggleNavCollapsed"
      >
        <svg class="col-splitter-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M14 6l-6 6 6 6" />
        </svg>
      </button>
    </div>

    <div class="col-shell col-shell--list">
    <section class="col-list">
      <div class="col-list-inner">
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
                <span v-if="slot.note.isPinned" class="note-pin-tag">置顶</span>
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
                :class="{ 'note-pin--on': slot.note.isPinned }"
                :title="slot.note.isPinned ? '取消置顶' : '置顶'"
                :aria-pressed="slot.note.isPinned"
                aria-label="置顶"
                @click.stop="toggleNotePinned(slot.note.id)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true">
                  <path d="M9 4h6l-1 7h4l-6 9-6-9h4L9 4z" />
                </svg>
              </button>
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
      </div>
    </section>
    </div>

    <div class="col-splitter col-splitter--list">
      <button
        type="button"
        class="col-splitter-btn"
        :title="listCollapsed ? '展开笔记列表' : '收起笔记列表'"
        :aria-label="listCollapsed ? '展开笔记列表' : '收起笔记列表'"
        :aria-expanded="!listCollapsed"
        @click="toggleListCollapsed"
      >
        <svg class="col-splitter-chevron" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M14 6l-6 6 6 6" />
        </svg>
      </button>
    </div>

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
          <input
            v-model="noteTitleDraft"
            class="main-title-input"
            type="text"
            placeholder="笔记标题"
            @input="onNoteTitleInput"
          />
          <div class="main-title-actions">
          <button
            type="button"
            class="main-title-focus"
            :class="{ 'main-title-focus--on': presentSession?.noteId === selectedNoteId }"
            :title="presentSession ? `实时演示，${presentSession.peers?.length || 0} 人在线` : '实时演示'"
            aria-label="实时演示"
            @click="openPresent"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 5h16v10H4z" />
              <path d="M8 19h8" />
              <path d="M12 15v4" />
            </svg>
            <span v-if="presentSession" class="present-count">{{ presentSession.peers?.length || 0 }}</span>
          </button>
          <button
            type="button"
            class="main-title-focus"
            :class="{ 'main-title-focus--on': historyOpen }"
            title="历史版本"
            aria-label="历史版本"
            @click="openHistory"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="12" cy="12" r="8" />
              <path d="M12 8v5l3 2" />
            </svg>
          </button>
          <button
            type="button"
            class="main-title-focus"
            :class="{ 'main-title-focus--on': editorFocusMode }"
            :title="editorFocusMode ? '退出全屏编辑 (Esc)' : '全屏编辑'"
            :aria-label="editorFocusMode ? '退出全屏编辑' : '全屏编辑'"
            :aria-pressed="editorFocusMode"
            @click="toggleEditorFocusMode"
          >
            <svg
              v-if="editorFocusMode"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M4 14h6v6M20 10h-6V4M14 20l7-7M10 4 3 11" />
            </svg>
            <svg
              v-else
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
            </svg>
          </button>
          </div>
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
    <HistoryDrawer :open="historyOpen" :note-id="selectedNoteId" @close="historyOpen = false" />
    <PresentPanel
      :open="presentOpen"
      :session="presentSession"
      :max-editors="presentMaxEditors"
      @close="presentOpen = false"
      @stop="stopPresent"
      @set-max="setPresentMax"
      @revoke="revokeEdit()"
      @revoke-ip="revokeEdit"
    />
    <SettingsDialog
      :open="settingsOpen"
      :settings="settings"
      :path="settingsPath"
      :focus="settingsFocus"
      @close="settingsOpen = false"
      @save="saveAppSettings"
    />
    <Teleport to="body">
      <div
        v-if="colorPick"
        class="folder-color-pop"
        :style="{ left: colorPick.x + 'px', top: colorPick.y + 'px' }"
      >
        <button
          v-for="color in FOLDER_DOT_COLORS"
          :key="color"
          type="button"
          class="folder-color-swatch"
          :style="{ background: color }"
          :aria-label="color"
          @click="chooseFolderColor(color)"
        />
      </div>
      <div
        v-if="ptrDragFolderId"
        class="folder-drag-ghost"
        :style="{ width: folderGhost.w + 'px', transform: `translate(${folderGhost.x}px, ${folderGhost.y}px)` }"
      >
        {{ folderGhost.name }}
      </div>
    </Teleport>
    <Teleport to="body">
      <div v-if="pendingEditRequest" class="edit-ask">
        <section class="edit-ask-card" role="dialog" aria-label="编辑权限申请">
          <h3>编辑权限申请</h3>
          <p>{{ pendingEditRequest.ip }} 申请该笔记的编辑权限</p>
          <p v-if="(presentSession?.requests.length || 0) > 1" class="edit-ask-more">
            还有 {{ (presentSession?.requests.length || 1) - 1 }} 条申请在排队
          </p>
          <div class="edit-ask-actions">
            <button type="button" class="edit-ask-yes" @click="decideEdit(true)">同意</button>
            <button type="button" class="edit-ask-no" @click="decideEdit(false)">拒绝</button>
          </div>
        </section>
      </div>
    </Teleport>

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
            @click="toggleNotePinned(contextMenu.noteId); closeContextMenu()"
          >
            {{ noteById(contextMenu.noteId)?.isPinned ? "取消置顶" : "置顶" }}
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
  font-family: var(--font);
  font-size: 14px;
}

.app-columns {
  --col-nav-w: 268px;
  --col-nav-w-collapsed: 72px;
  --col-list-w: 340px;
  --col-splitter-w: 14px;
  --col-ease: cubic-bezier(0.22, 1, 0.36, 1);
  --col-dur: 0.36s;
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

.col-shell {
  flex-shrink: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  transition: width var(--col-dur) var(--col-ease);
}

.col-shell--nav {
  width: var(--col-nav-w);
  border-right: 1px solid var(--line);
  box-sizing: border-box;
}

.app-columns.nav-collapsed .col-shell--nav {
  width: var(--col-nav-w-collapsed);
}

.col-shell--list {
  width: var(--col-list-w);
}

.app-columns.list-collapsed .col-shell--list {
  width: 0;
}

.col-splitter {
  flex-shrink: 0;
  width: var(--col-splitter-w);
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  z-index: 6;
  align-self: stretch;
}

.col-splitter--nav {
  margin-left: -7px;
  margin-right: -7px;
}

.col-splitter--list {
  margin-left: -7px;
  margin-right: -7px;
}

.col-splitter-btn {
  width: 22px;
  height: 48px;
  padding: 0;
  border: 1px solid rgba(197, 208, 224, 0.95);
  border-radius: 11px;
  background: linear-gradient(180deg, #fff 0%, #f8fafc 100%);
  color: #94a3b8;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow:
    0 1px 3px rgba(15, 23, 42, 0.06),
    0 4px 12px rgba(15, 23, 42, 0.05);
  transition:
    color 0.22s var(--col-ease),
    border-color 0.22s var(--col-ease),
    background 0.22s var(--col-ease),
    box-shadow 0.22s var(--col-ease),
    transform 0.22s var(--col-ease);
}

.col-splitter-btn:hover {
  color: var(--accent);
  border-color: var(--accent);
  background: var(--surface);
  box-shadow:
    0 2px 8px rgba(37, 99, 235, 0.12),
    0 6px 18px rgba(15, 23, 42, 0.08);
  transform: scale(1.04);
}

.col-splitter-btn:active {
  transform: scale(0.97);
}

.col-splitter-chevron {
  transition: transform var(--col-dur) var(--col-ease);
}

.app-columns.nav-collapsed .col-splitter--nav .col-splitter-chevron {
  transform: rotate(180deg);
}

.app-columns.list-collapsed .col-splitter--list .col-splitter-chevron {
  transform: rotate(180deg);
}

.app-columns.editor-focus-mode .col-shell--nav,
.app-columns.editor-focus-mode .col-shell--list {
  width: 0 !important;
  min-width: 0;
  border-width: 0;
  overflow: hidden;
}

.app-columns.editor-focus-mode .col-splitter--nav,
.app-columns.editor-focus-mode .col-splitter--list {
  width: 0;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  opacity: 0;
  pointer-events: none;
}

.col-nav {
  width: 100%;
  min-width: 0;
  height: 100%;
  background: var(--bg-side);
  border-right: none;
  display: flex;
  flex-direction: column;
  padding: 10px 12px 16px;
  gap: 12px;
  min-height: 0;
  flex: 1;
  overflow: hidden;
  box-sizing: border-box;
  transition: padding var(--col-dur) var(--col-ease);
}

.app-columns.nav-collapsed .col-nav {
  padding: 10px 8px 16px;
  align-items: center;
}

.col-nav-expandable {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  transition:
    opacity calc(var(--col-dur) * 0.75) var(--col-ease),
    max-height var(--col-dur) var(--col-ease),
    transform calc(var(--col-dur) * 0.85) var(--col-ease);
  max-height: 2000px;
  opacity: 1;
  transform: translateX(0);
}

.col-nav-actions {
  flex-shrink: 0;
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  width: 100%;
  padding-top: 4px;
}

.app-columns.nav-collapsed .col-nav-expandable {
  max-height: 0;
  opacity: 0;
  transform: translateX(-12px);
  pointer-events: none;
  margin: 0;
  gap: 0;
}

.nav-pill-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: opacity calc(var(--col-dur) * 0.6) var(--col-ease);
}

.app-columns.nav-collapsed .nav-pill-label {
  opacity: 0;
  width: 0;
  flex: 0;
}

.app-columns.nav-collapsed .nav-pill {
  justify-content: center;
  padding: 10px;
  gap: 0;
  width: 48px;
}

.app-columns.nav-collapsed .nav-primary {
  width: 100%;
  align-items: center;
}

.app-columns.nav-collapsed .nav-rail-new {
  margin-top: auto;
}

.nav-rail-new {
  flex-shrink: 0;
  width: 48px;
  height: 48px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(180deg, #1e40af 0%, #1d4ed8 100%);
  color: #fff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 10px rgba(29, 78, 216, 0.35);
  transition:
    transform 0.2s var(--col-ease),
    box-shadow 0.2s var(--col-ease),
    opacity 0.25s var(--col-ease);
  animation: rail-new-in 0.32s var(--col-ease);
}

@keyframes rail-new-in {
  from {
    opacity: 0;
    transform: scale(0.85);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.nav-rail-new:hover {
  transform: translateY(-1px);
  box-shadow: 0 4px 14px rgba(29, 78, 216, 0.45);
}

.app-columns.nav-collapsed :deep(.col-nav-brand .nav-brand) {
  padding: 0 0 4px;
}

.app-columns.nav-collapsed :deep(.col-nav-brand .nav-brand-visual) {
  flex-direction: column;
  align-items: center;
  gap: 0;
}

.app-columns.nav-collapsed :deep(.col-nav-brand .nav-brand-text) {
  opacity: 0;
  max-height: 0;
  overflow: hidden;
  margin: 0;
  pointer-events: none;
  transition: opacity 0.22s var(--col-ease), max-height var(--col-dur) var(--col-ease);
}

.app-columns.nav-collapsed :deep(.col-nav-brand .nav-brand-logo) {
  width: 44px;
  height: 44px;
  transition: width var(--col-dur) var(--col-ease), height var(--col-dur) var(--col-ease);
}

.col-list {
  width: 100%;
  min-width: 0;
  height: 100%;
  background: var(--bg-middle);
  border-right: none;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.col-list-inner {
  width: var(--col-list-w);
  min-width: var(--col-list-w);
  height: 100%;
  display: flex;
  flex-direction: column;
  min-height: 0;
  box-sizing: border-box;
  border-right: 1px solid var(--line);
  transition:
    opacity calc(var(--col-dur) * 0.7) var(--col-ease),
    transform calc(var(--col-dur) * 0.85) var(--col-ease);
}

.app-columns.list-collapsed .col-list-inner {
  opacity: 0;
  transform: translateX(-16px);
  pointer-events: none;
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
  background: var(--wash);
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
  color: var(--muted);
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
  background: var(--wash);
  box-shadow: none;
}

.folder-dir-item.selected {
  background: var(--accent-sidebar-active);
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
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.9);
  cursor: pointer;
}
.folder-row-gap {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 34px;
  margin: 2px 8px;
  border: 2px dashed rgba(37, 99, 235, 0.42);
  border-radius: 10px;
  background: rgba(37, 99, 235, 0.06);
  color: rgba(37, 99, 235, 0.85);
  font-size: 12px;
}
.folder-dir-item.is-dragging { opacity: 0.35; }
.nav-version {
  margin: 2px 2px 0;
  font-size: 12px;
  color: var(--muted);
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
  background: var(--field);
  border: 1px solid var(--field-line);
  box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
}

.list-search-ic {
  display: flex;
  color: var(--muted);
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
  color: var(--muted);
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
  border: 1px solid var(--card-line);
  background: var(--card);
  color: var(--text);
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.05);
  transition:
    background 0.15s ease,
    box-shadow 0.15s ease,
    border-color 0.15s ease;
}

.note-row:hover {
  background: var(--card-hover);
  border-color: var(--card-line);
  box-shadow: 0 2px 10px rgba(15, 23, 42, 0.07);
}

.note-row.active {
  background: var(--card-active);
  border-color: var(--accent);
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
  color: var(--text);
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
  color: var(--muted);
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
.note-fav--on,
.note-pin--on {
  opacity: 1;
}

.note-pin--on {
  color: #1d4ed8;
}

.note-pin-tag {
  flex-shrink: 0;
  margin-right: 6px;
  padding: 1px 6px;
  border-radius: 999px;
  background: #dbeafe;
  color: #1d4ed8;
  font-size: 10px;
  font-weight: 700;
  line-height: 16px;
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

.folder-color-pop {
  position: fixed;
  z-index: 100001;
  display: grid;
  grid-template-columns: repeat(4, 22px);
  gap: 8px;
  padding: 10px;
  border-radius: 12px;
  background: var(--bg, #fff);
  border: 1px solid var(--line, #e5e7eb);
  box-shadow: 0 12px 30px rgba(15, 23, 42, 0.16);
}
.folder-color-swatch {
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 99px;
  cursor: pointer;
  padding: 0;
}
.folder-drag-ghost {
  position: fixed;
  left: 0;
  top: 0;
  z-index: 100002;
  pointer-events: none;
  padding: 8px 12px;
  border-radius: 10px;
  background: var(--bg, #fff);
  color: var(--text, #111827);
  border: 1px solid var(--line, #e5e7eb);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.16);
  font-size: 13px;
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
  background: var(--card);
  color: var(--text);
  border: 1px solid var(--card-line);
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
  transition: flex var(--col-dur) var(--col-ease);
}

@media (prefers-reduced-motion: reduce) {
  .col-shell,
  .app-columns.editor-focus-mode .col-shell--nav,
  .app-columns.editor-focus-mode .col-shell--list,
  .app-columns.editor-focus-mode .col-splitter--nav,
  .app-columns.editor-focus-mode .col-splitter--list,
  .col-nav-expandable,
  .col-list-inner,
  .col-splitter-chevron,
  .col-splitter-btn,
  .nav-pill-label,
  .nav-rail-new {
    transition: none;
    animation: none;
  }

  .app-columns.nav-collapsed .col-nav-expandable,
  .app-columns.list-collapsed .col-list-inner {
    transform: none;
  }
}

.main-title-bar {
  padding: 10px 28px 8px;
  flex-shrink: 0;
  background: var(--paper);
}

.main-title-row {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  width: 100%;
}

.main-title-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.main-title-focus {
  position: relative;
  flex-shrink: 0;
  align-self: center;
  border: 1px solid var(--line);
  background: var(--chip);
  cursor: pointer;
  padding: 8px;
  margin-top: 2px;
  color: var(--muted);
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition:
    color 0.12s ease,
    background 0.12s ease,
    border-color 0.12s ease;
}

.main-title-focus:hover {
  color: var(--accent);
  border-color: var(--accent);
  background: var(--card-hover);
}

.main-title-focus--on {
  color: var(--accent);
  border-color: var(--accent-soft);
  background: var(--accent-sidebar-active);
}

.present-count {
  position: absolute;
  top: -6px;
  right: -6px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 99px;
  background: #1d4ed8;
  color: #fff;
  font-size: 10px;
  line-height: 16px;
  text-align: center;
}

.edit-ask {
  position: fixed;
  inset: 0;
  z-index: 90;
  display: grid;
  place-items: center;
  background: rgba(15, 23, 42, 0.28);
  font-family: var(--font);
}

.edit-ask-card {
  width: min(420px, calc(100vw - 48px));
  padding: 22px 22px 18px;
  border-radius: 16px;
  background: var(--surface);
  color: var(--surface-text);
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.22);
}

.edit-ask-card h3 {
  margin: 0 0 8px;
  font-size: 18px;
}

.edit-ask-card p {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--surface-muted);
}

.edit-ask-more {
  margin-top: 6px !important;
  color: #6b7280 !important;
  font-size: 12px !important;
}

.edit-ask-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.edit-ask-yes,
.edit-ask-no {
  height: 36px;
  padding: 0 16px;
  border: none;
  border-radius: 10px;
  cursor: pointer;
  font: inherit;
  font-size: 14px;
}

.edit-ask-yes {
  background: #1d4ed8;
  color: #fff;
}

.edit-ask-no {
  background: var(--chip);
  color: var(--chip-text);
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
  background: var(--paper);
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
  background: var(--surface);
  color: var(--surface-text);
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
  background: var(--field);
  color: var(--text);
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
  background: var(--chip);
  color: var(--chip-text);
}

.modal-btn.primary {
  background: var(--accent);
  color: #fff;
  font-weight: 600;
}

.ctx-menu {
  --ctx-line: var(--line);
  --ctx-text: var(--surface-text);
  --ctx-muted: var(--surface-muted);
  position: fixed;
  z-index: 10000;
  min-width: 200px;
  max-width: 280px;
  padding: 6px 0;
  margin: 0;
  list-style: none;
  background: var(--surface);
  border-radius: 10px;
  box-shadow:
    0 4px 24px rgba(0, 0, 0, 0.12),
    0 0 1px rgba(0, 0, 0, 0.08);
  border: 1px solid var(--ctx-line);
  font-family: var(--font);
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
  background: var(--surface);
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
html {
  margin: 0;
  min-height: 100%;
  height: 100%;
  color-scheme: light;
  --font: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  font-family: var(--font);
  --bg-side: #f5f7fa;
  --bg-middle: #e8eef6;
  --bg: #ffffff;
  --line: #e5e7eb;
  --text: #111827;
  --muted: #6b7280;
  --accent: #2563eb;
  --accent-soft: #dbeafe;
  --accent-strong: #1d4ed8;
  --accent-sidebar-active: #e8f1fe;
  --card: #ffffff;
  --card-hover: #f8fafc;
  --card-active: #ffffff;
  --card-line: #d5deea;
  --field: #ffffff;
  --field-line: #c5d0e0;
  --paper: #fafbfc;
  --toolbar: #ffffff;
  --chip: #f0f2f5;
  --chip-hover: #e4e6ea;
  --chip-text: #1f2937;
  --wash: rgba(15, 23, 42, 0.05);
  --surface: #ffffff;
  --surface-text: #111827;
  --surface-muted: #6b7280;
  --code-bg: #f3f4f6;
  --quote: #4b5563;
  --quote-bar: #c5cad3;
  background: var(--bg-side);
}
html[data-theme="dark"] {
  color-scheme: dark;
  --bg-side: #0e1420;
  --bg-middle: #121926;
  --bg: #171f2e;
  --line: #2a3548;
  --text: #e8eef7;
  --muted: #9aa8bd;
  --accent-soft: #1e3a5f;
  --accent-sidebar-active: #1a2c4d;
  --card: #1c2636;
  --card-hover: #243044;
  --card-active: #1d2d48;
  --card-line: #334158;
  --field: #1a2433;
  --field-line: #3a4a62;
  --paper: #171f2e;
  --toolbar: #141c2b;
  --chip: #243044;
  --chip-hover: #2d3c54;
  --chip-text: #e8eef7;
  --wash: rgba(255, 255, 255, 0.06);
  --surface: #1c2636;
  --surface-text: #e8eef7;
  --surface-muted: #9aa8bd;
  --code-bg: #243044;
  --quote: #c5d0e0;
  --quote-bar: #4b5d78;
}
html[data-skin="green"] { --accent: #059669; --accent-strong: #047857; --accent-soft: #d1fae5; --accent-sidebar-active: #ecfdf5; }
html[data-skin="violet"] { --accent: #7c3aed; --accent-strong: #6d28d9; --accent-soft: #ede9fe; --accent-sidebar-active: #f5f3ff; }
html[data-skin="amber"] { --accent: #d97706; --accent-strong: #b45309; --accent-soft: #fef3c7; --accent-sidebar-active: #fffbeb; }
html[data-skin="slate"] { --accent: #475569; --accent-strong: #334155; --accent-soft: #e2e8f0; --accent-sidebar-active: #f1f5f9; }
html[data-theme="dark"][data-skin="blue"] { --accent-soft: #16325c; --accent-sidebar-active: #1a3058; }
html[data-theme="dark"][data-skin="green"] { --accent: #34d399; --accent-strong: #6ee7b7; --accent-soft: #064e3b; --accent-sidebar-active: #0b3b2e; }
html[data-theme="dark"][data-skin="violet"] { --accent: #a78bfa; --accent-strong: #c4b5fd; --accent-soft: #3b2670; --accent-sidebar-active: #2e1b55; }
html[data-theme="dark"][data-skin="amber"] { --accent: #fbbf24; --accent-strong: #fcd34d; --accent-soft: #5c3b10; --accent-sidebar-active: #4a2e0c; }
html[data-theme="dark"][data-skin="slate"] { --accent: #cbd5e1; --accent-strong: #e2e8f0; --accent-soft: #334155; --accent-sidebar-active: #1e293b; }

body {
  margin: 0;
  min-height: 100%;
  height: 100%;
  background: var(--bg-side);
  color: var(--text);
}

#app {
  margin: 0;
  min-height: 100%;
  height: 100%;
  background: var(--bg-side);
  display: flex;
  flex-direction: column;
  align-items: stretch;
}
</style>
