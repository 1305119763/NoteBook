<script setup lang="ts">
import { getMarkRange } from "@tiptap/core";
import { computed, onBeforeUnmount, onMounted, ref, watch, nextTick } from "vue";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import Placeholder from "@tiptap/extension-placeholder";
import Link from "@tiptap/extension-link";
import Underline from "@tiptap/extension-underline";
import StarterKit from "@tiptap/starter-kit";
import Image from "@tiptap/extension-image";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PhysicalPosition } from "@tauri-apps/api/dpi";
import { open } from "@tauri-apps/plugin-dialog";
import { Video } from "../extensions/Video";

const props = defineProps<{
  modelValue: string;
  editable: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [string];
}>();

const isImporting = ref(false);

const ALLOWED_IMAGE_EXTS = ["jpg", "jpeg", "png", "gif", "webp", "svg"];
const ALLOWED_VIDEO_EXTS = ["mp4", "webm", "mov"];
const ALLOWED_EXTS = [...ALLOWED_IMAGE_EXTS, ...ALLOWED_VIDEO_EXTS];

const dragOverlayVisible = ref(false);
const dragPreviewUrl = ref<string | null>(null);
const dragPreviewLabel = ref("");
const dragPreviewKind = ref<"image" | "video" | "media">("image");
const dragDepth = ref(0);
const editorDropZoneRef = ref<HTMLElement | null>(null);
const linkBtnRef = ref<HTMLElement | null>(null);
const linkPopoverRef = ref<HTMLElement | null>(null);
const linkFormOpen = ref(false);
/** create=新建链接，edit=编辑，view=只读查看 */
const linkFormMode = ref<"create" | "edit" | "view">("create");
const linkFormTitle = ref("");
const linkFormHref = ref("");
const linkFormRange = ref<{ from: number; to: number } | null>(null);
const linkPopoverStyle = ref({ top: "0px", left: "0px" });

const linkFormTitleLabel = computed(() =>
  linkFormMode.value === "view" ? "查看链接" : linkFormRange.value ? "编辑链接" : "插入链接",
);

const linkHover = ref({
  visible: false,
  href: "",
  title: "",
  from: 0,
  to: 0,
  top: 0,
  left: 0,
});

const HEADING_LEVELS = [1, 2, 3, 4, 5, 6] as const;

let dragPreviewObjectUrl: string | null = null;
let linkHoverHideTimer: ReturnType<typeof setTimeout> | null = null;
let linkFormDismissHandler: ((e: MouseEvent) => void) | null = null;
let dropHandledAt = 0;
let unlistenTauriDragDrop: UnlistenFn | null = null;
let windowScaleFactor = 1;

function revokeDragPreviewUrl() {
  if (dragPreviewObjectUrl) {
    URL.revokeObjectURL(dragPreviewObjectUrl);
    dragPreviewObjectUrl = null;
  }
  dragPreviewUrl.value = null;
}

function basename(path: string): string {
  return path.split(/[/\\]/).pop() || path;
}

function isMediaPath(path: string): boolean {
  const ext = basename(path).split(".").pop()?.toLowerCase() || "";
  return ALLOWED_EXTS.includes(ext);
}

function mediaPathsFromStrings(paths: string[]): string[] {
  return paths.filter(isMediaPath);
}

function classifyFile(file: File): { ext: string; isImage: boolean; isVideo: boolean } | null {
  const extFromName = file.name.split(".").pop()?.toLowerCase() || "";
  const extFromType = file.type.split("/").pop()?.toLowerCase() || "";
  const ext = ALLOWED_EXTS.includes(extFromName) ? extFromName : extFromType;
  const isImage = ALLOWED_IMAGE_EXTS.includes(ext);
  const isVideo = ALLOWED_VIDEO_EXTS.includes(ext);
  if (!isImage && !isVideo) return null;
  return { ext, isImage, isVideo };
}

function filesFromDataTransfer(dt: DataTransfer): File[] {
  const fromList = Array.from(dt.files);
  if (fromList.length > 0) return fromList;
  const out: File[] = [];
  for (let i = 0; i < dt.items.length; i++) {
    const item = dt.items[i];
    if (item.kind !== "file") continue;
    const file = item.getAsFile();
    if (file) out.push(file);
  }
  return out;
}

function mediaFilesFromDataTransfer(dt: DataTransfer): File[] {
  return filesFromDataTransfer(dt).filter((f) => classifyFile(f) !== null);
}

function hasMediaPayload(dt: DataTransfer): boolean {
  if (mediaFilesFromDataTransfer(dt).length > 0) return true;
  for (let i = 0; i < dt.types.length; i++) {
    const t = dt.types[i];
    if (t === "Files" || t.startsWith("image/") || t.startsWith("video/")) return true;
  }
  return false;
}

function updateDragPreviewFromPaths(paths: string[]) {
  if (paths.length === 0) {
    dragPreviewLabel.value = "拖放图片或视频到此处";
    dragPreviewKind.value = "media";
    revokeDragPreviewUrl();
    return;
  }
  const first = paths[0];
  const ext = basename(first).split(".").pop()?.toLowerCase() || "";
  dragPreviewLabel.value =
    paths.length > 1 ? `${basename(first)} 等 ${paths.length} 个文件` : basename(first);
  dragPreviewKind.value = ALLOWED_VIDEO_EXTS.includes(ext) ? "video" : "image";
  if (ALLOWED_IMAGE_EXTS.includes(ext)) {
    revokeDragPreviewUrl();
    dragPreviewUrl.value = convertFileSrc(first);
  } else {
    revokeDragPreviewUrl();
  }
}

function updateDragPreview(dt: DataTransfer) {
  const media = mediaFilesFromDataTransfer(dt);
  if (media.length === 0) {
    dragPreviewLabel.value = "拖放图片或视频到此处";
    dragPreviewKind.value = "media";
    revokeDragPreviewUrl();
    return;
  }
  const first = media[0];
  const kind = classifyFile(first);
  dragPreviewLabel.value =
    media.length > 1
      ? `${first.name} 等 ${media.length} 个文件`
      : first.name || (kind?.isVideo ? "视频" : "图片");
  dragPreviewKind.value = kind?.isVideo ? "video" : "image";
  if (kind?.isImage && first.type.startsWith("image/")) {
    if (dragPreviewObjectUrl) URL.revokeObjectURL(dragPreviewObjectUrl);
    dragPreviewObjectUrl = URL.createObjectURL(first);
    dragPreviewUrl.value = dragPreviewObjectUrl;
  } else {
    revokeDragPreviewUrl();
  }
}

function clearDragOverlay() {
  dragDepth.value = 0;
  dragOverlayVisible.value = false;
  dragPreviewLabel.value = "";
  revokeDragPreviewUrl();
}

function onEditorDragEnter(e: DragEvent) {
  if (!props.editable || !e.dataTransfer) return;
  if (!hasMediaPayload(e.dataTransfer)) return;
  dragDepth.value += 1;
  dragOverlayVisible.value = true;
  updateDragPreview(e.dataTransfer);
}

function onEditorDragOver(e: DragEvent) {
  if (!props.editable || !e.dataTransfer) return;
  if (!hasMediaPayload(e.dataTransfer)) return;
  e.preventDefault();
  e.stopPropagation();
  e.dataTransfer.dropEffect = "copy";
  if (!dragOverlayVisible.value) {
    dragOverlayVisible.value = true;
    updateDragPreview(e.dataTransfer);
  } else {
    updateDragPreview(e.dataTransfer);
  }
}

function onEditorDragLeave(e: DragEvent) {
  if (!props.editable) return;
  const wrap = e.currentTarget as HTMLElement;
  const related = e.relatedTarget as Node | null;
  if (related && wrap.contains(related)) return;
  dragDepth.value = Math.max(0, dragDepth.value - 1);
  if (dragDepth.value === 0) clearDragOverlay();
}

function clientCoordsFromDrop(position: PhysicalPosition): { x: number; y: number } {
  const logical = position.toLogical(windowScaleFactor);
  if (isPointerOverEditor(logical.x, logical.y)) return logical;
  if (isPointerOverEditor(position.x, position.y)) {
    return { x: position.x, y: position.y };
  }
  return logical;
}

function isPointerOverEditor(clientX: number, clientY: number): boolean {
  const zone = editorDropZoneRef.value;
  if (!zone) return false;
  const hit = document.elementFromPoint(clientX, clientY);
  if (hit && zone.contains(hit)) return true;
  const r = zone.getBoundingClientRect();
  return clientX >= r.left && clientX <= r.right && clientY >= r.top && clientY <= r.bottom;
}

function setSelectionAtClient(clientX: number, clientY: number) {
  const ed = editor.value;
  const view = ed?.view;
  if (!ed || !view) return;
  const coords = view.posAtCoords({ left: clientX, top: clientY });
  if (coords) {
    ed.chain().setTextSelection(coords.pos).run();
  } else {
    ed.chain().focus("end").run();
  }
}

function setSelectionAtDrop(event: DragEvent) {
  setSelectionAtClient(event.clientX, event.clientY);
}

function setupTauriDragDropListener() {
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const payload = event.payload;

      if (payload.type === "leave") {
        clearDragOverlay();
        return;
      }

      if (!props.editable) return;

      if (payload.type === "enter") {
        const paths = mediaPathsFromStrings(payload.paths);
        if (paths.length === 0) return;
        dragDepth.value = 1;
        dragOverlayVisible.value = true;
        updateDragPreviewFromPaths(paths);
        return;
      }

      if (payload.type === "drop") {
        const paths = mediaPathsFromStrings(payload.paths);
        const hadOverlay = dragOverlayVisible.value;
        const { x, y } = clientCoordsFromDrop(payload.position);
        const overEditor = isPointerOverEditor(x, y) || hadOverlay;
        clearDragOverlay();
        if (paths.length === 0) return;
        if (!overEditor) return;
        dropHandledAt = Date.now();
        setSelectionAtClient(x, y);
        void importFromPaths(paths);
      }
    })
    .then((unlisten) => {
      unlistenTauriDragDrop = unlisten;
    })
    .catch(() => {
      /* 非 Tauri 环境（纯浏览器 dev）时忽略 */
    });
}

async function onEditorDrop(e: DragEvent) {
  if (!props.editable || !e.dataTransfer) return;
  if (Date.now() - dropHandledAt < 80) return;
  const files = mediaFilesFromDataTransfer(e.dataTransfer);
  if (files.length === 0) return;
  e.preventDefault();
  e.stopPropagation();
  clearDragOverlay();
  setSelectionAtDrop(e);
  await handleFiles(files);
}

const editor = useEditor({
  extensions: [
    StarterKit.configure({
      heading: { levels: [1, 2, 3, 4, 5, 6] },
    }),
    Underline,
    Link.configure({
      openOnClick: false,
      HTMLAttributes: { class: "notebook-inline-link" },
    }),
    Placeholder.configure({
      placeholder: "在此处输入正文…",
    }),
    Image.configure({
      allowBase64: false,
      HTMLAttributes: {
        class: "notebook-inline-media",
      },
    }),
    Video,
  ],
  content: props.modelValue?.trim() ? props.modelValue : "<p></p>",
  editable: props.editable,
  editorProps: {
    attributes: {
      class: "note-editor-prosemirror",
    },
    handleDrop: (_view, event) => {
      if (!event.dataTransfer || !props.editable) return false;
      const files = mediaFilesFromDataTransfer(event.dataTransfer);
      if (files.length === 0) return false;
      event.preventDefault();
      dropHandledAt = Date.now();
      clearDragOverlay();
      setSelectionAtDrop(event);
      void handleFiles(files);
      return true;
    },
    handlePaste: (_view, event) => {
      if (!event.clipboardData || !props.editable) return false;
      const items = event.clipboardData.items;
      const files: File[] = [];
      for (let i = 0; i < items.length; i++) {
        const item = items[i];
        if (item.kind === "file") {
          const file = item.getAsFile();
          if (file) files.push(file);
        }
      }
      if (files.length > 0) {
        handleFiles(files);
        return true;
      }
      return false;
    },
    handleDOMEvents: {
      mouseover: (view, event) => {
        if (!props.editable || linkFormOpen.value) return false;
        const target = (event.target as HTMLElement).closest(
          "a.notebook-inline-link",
        ) as HTMLAnchorElement | null;
        if (!target || !view.dom.contains(target)) return false;
        showLinkHoverFromAnchor(target);
        return false;
      },
      mouseout: (view, event) => {
        if (!props.editable) return false;
        const related = event.relatedTarget as Node | null;
        if (related && view.dom.contains(related)) {
          const hoverEl = document.querySelector(".rte-link-hover");
          if (hoverEl?.contains(related)) return false;
        }
        scheduleHideLinkHover();
        return false;
      },
    },
  },
  onUpdate: ({ editor: ed }) => {
    emit("update:modelValue", ed.getHTML());
  },
});

watch(
  () => props.modelValue,
  (html) => {
    const ed = editor.value;
    if (!ed) return;
    const next = html?.trim() ? html : "<p></p>";
    if (ed.getHTML() === next) return;
    ed.commands.setContent(next, { emitUpdate: false });
  },
);

watch(
  () => props.editable,
  (v) => {
    editor.value?.setEditable(v);
    if (!v) {
      clearDragOverlay();
      closeLinkForm();
      linkHover.value.visible = false;
    }
  },
);

onMounted(() => {
  void getCurrentWindow()
    .scaleFactor()
    .then((factor) => {
      windowScaleFactor = factor;
    })
    .catch(() => {
      windowScaleFactor = window.devicePixelRatio || 1;
    });
  setupTauriDragDropListener();
  linkFormDismissHandler = (e: MouseEvent) => {
    if (!linkFormOpen.value) return;
    const t = e.target as Node;
    if (linkBtnRef.value?.contains(t)) return;
    if (linkPopoverRef.value?.contains(t)) return;
    closeLinkForm();
  };
  document.addEventListener("mousedown", linkFormDismissHandler, true);
});

onBeforeUnmount(() => {
  unlistenTauriDragDrop?.();
  unlistenTauriDragDrop = null;
  clearDragOverlay();
  if (linkFormDismissHandler) {
    document.removeEventListener("mousedown", linkFormDismissHandler, true);
    linkFormDismissHandler = null;
  }
  clearLinkHoverHideTimer();
  editor.value?.destroy();
});

async function handleFiles(files: File[]) {
  const ed = editor.value;
  if (!ed) return;
  isImporting.value = true;

  for (const file of files) {
    const kind = classifyFile(file);
    if (!kind) continue;

    try {
      const buffer = await file.arrayBuffer();
      const result = await invoke("import_media_bytes", {
        data: Array.from(new Uint8Array(buffer)),
        extension: kind.ext,
      }) as { mediaSrc: string; storageKey: string; absolutePath: string };

      if (kind.isImage) {
        ed.chain().focus().setImage({ src: convertFileSrc(result.absolutePath) }).run();
      } else {
        ed.chain().focus().setVideo({ src: convertFileSrc(result.absolutePath) }).run();
      }
    } catch (e) {
      console.error("媒体导入失败:", e);
    }
  }
  isImporting.value = false;
}

async function pickAndImportImage() {
  const selected = await open({
    multiple: true,
    filters: [{
      name: "图片",
      extensions: ["jpg", "jpeg", "png", "gif", "webp", "svg"],
    }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await importFromPaths(paths);
}

async function pickAndImportVideo() {
  const selected = await open({
    multiple: true,
    filters: [{
      name: "视频",
      extensions: ["mp4", "webm", "mov"],
    }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await importFromPaths(paths);
}

async function importFromPaths(paths: string[]) {
  const ed = editor.value;
  if (!ed) return;
  const mediaPaths = mediaPathsFromStrings(paths);
  if (mediaPaths.length === 0) return;
  isImporting.value = true;

  for (const filePath of mediaPaths) {
    try {
      const result = await invoke("import_media_file", {
        filePath,
      }) as { mediaSrc: string; storageKey: string; absolutePath: string };

      const ext = filePath.split(".").pop()?.toLowerCase() || "";
      const isImage = ALLOWED_IMAGE_EXTS.includes(ext);

      if (isImage) {
        ed.chain().focus().setImage({ src: convertFileSrc(result.absolutePath) }).run();
      } else {
        ed.chain().focus().setVideo({ src: convertFileSrc(result.absolutePath) }).run();
      }
    } catch (e) {
      console.error("媒体导入失败:", e);
    }
  }
  isImporting.value = false;
}

function tbDisabled(): boolean {
  return !props.editable || !editor.value || isImporting.value;
}

function tbUndoDisabled(): boolean {
  return tbDisabled() || !editor.value?.can().chain().focus().undo().run();
}

function tbRedoDisabled(): boolean {
  return tbDisabled() || !editor.value?.can().chain().focus().redo().run();
}

function tbSinkListDisabled(): boolean {
  return (
    tbDisabled() ||
    !editor.value?.can().chain().focus().sinkListItem("listItem").run()
  );
}

function tbLiftListDisabled(): boolean {
  return (
    tbDisabled() ||
    !editor.value?.can().chain().focus().liftListItem("listItem").run()
  );
}

function clearFormatting() {
  editor.value?.chain().focus().unsetAllMarks().clearNodes().run();
}

function normalizeHref(raw: string): string {
  const t = raw.trim();
  if (!t) return "";
  if (/^(https?:|mailto:|tel:|#)/i.test(t)) return t;
  return `https://${t}`;
}

function clampPopoverLeft(left: number, width: number): number {
  const margin = 8;
  const maxLeft = window.innerWidth - width - margin;
  return Math.max(margin, Math.min(left, maxLeft));
}

function positionLinkPopoverAtRect(rect: DOMRect) {
  const width = 300;
  const left = clampPopoverLeft(rect.left + rect.width / 2 - width / 2, width);
  linkPopoverStyle.value = {
    top: `${rect.bottom + 8}px`,
    left: `${left}px`,
  };
}

async function positionLinkPopoverFromButton() {
  await nextTick();
  const el = linkBtnRef.value;
  if (!el) return;
  positionLinkPopoverAtRect(el.getBoundingClientRect());
}

function clearLinkHoverHideTimer() {
  if (linkHoverHideTimer) {
    clearTimeout(linkHoverHideTimer);
    linkHoverHideTimer = null;
  }
}

function scheduleHideLinkHover() {
  clearLinkHoverHideTimer();
  linkHoverHideTimer = setTimeout(() => {
    linkHover.value.visible = false;
    linkHoverHideTimer = null;
  }, 180);
}

function showLinkHoverFromAnchor(anchor: HTMLAnchorElement) {
  const ed = editor.value;
  if (!ed || !props.editable) return;
  clearLinkHoverHideTimer();
  const view = ed.view;
  const pos = view.posAtDOM(anchor, 0);
  const $pos = view.state.doc.resolve(pos);
  const range = getMarkRange($pos, view.state.schema.marks.link);
  if (!range) return;
  const href = anchor.getAttribute("href") || "";
  const title = anchor.textContent?.trim() || href;
  const rect = anchor.getBoundingClientRect();
  linkHover.value = {
    visible: true,
    href,
    title,
    from: range.from,
    to: range.to,
    top: rect.bottom + 6,
    left: rect.left + rect.width / 2,
  };
}

function closeLinkForm() {
  linkFormOpen.value = false;
  linkFormRange.value = null;
  linkFormMode.value = "create";
}

function openLinkFormFromToolbar() {
  if (tbDisabled()) return;
  const ed = editor.value;
  if (!ed) return;
  if (linkFormOpen.value) {
    closeLinkForm();
    return;
  }
  linkHover.value.visible = false;
  linkFormMode.value = "create";
  const { from, to, empty } = ed.state.selection;
  if (ed.isActive("link")) {
    const $pos = ed.state.doc.resolve(from);
    const range = getMarkRange($pos, ed.state.schema.marks.link);
    if (range) {
      linkFormRange.value = range;
      linkFormTitle.value = ed.state.doc.textBetween(range.from, range.to);
    } else {
      linkFormRange.value = { from, to };
      linkFormTitle.value = ed.state.doc.textBetween(from, to);
    }
    linkFormHref.value = (ed.getAttributes("link").href as string) || "";
    linkFormMode.value = "edit";
  } else if (!empty) {
    linkFormTitle.value = ed.state.doc.textBetween(from, to);
    linkFormHref.value = "https://";
    linkFormRange.value = { from, to };
  } else {
    linkFormTitle.value = "";
    linkFormHref.value = "https://";
    linkFormRange.value = null;
  }
  linkFormOpen.value = true;
  void positionLinkPopoverFromButton();
}

async function openLinkPopoverFromHover(
  mode: "view" | "edit",
  e?: MouseEvent,
) {
  e?.preventDefault();
  e?.stopPropagation();
  clearLinkHoverHideTimer();
  linkFormTitle.value = linkHover.value.title;
  linkFormHref.value = linkHover.value.href;
  linkFormRange.value = { from: linkHover.value.from, to: linkHover.value.to };
  linkFormMode.value = mode;
  linkHover.value.visible = false;
  linkFormOpen.value = true;
  await nextTick();
  const width = 300;
  const left = clampPopoverLeft(linkHover.value.left - width / 2, width);
  linkPopoverStyle.value = {
    top: `${linkHover.value.top}px`,
    left: `${left}px`,
  };
}

function openLinkFormFromHover(e?: MouseEvent) {
  void openLinkPopoverFromHover("edit", e);
}

function submitLinkForm() {
  const ed = editor.value;
  if (!ed) return;
  const href = normalizeHref(linkFormHref.value);
  if (!href) return;
  const title = linkFormTitle.value.trim() || href;
  const range = linkFormRange.value;
  const chain = ed.chain().focus();
  if (range) {
    chain
      .setTextSelection(range)
      .deleteSelection()
      .insertContent({
        type: "text",
        text: title,
        marks: [{ type: "link", attrs: { href } }],
      })
      .run();
  } else {
    const { empty, from, to } = ed.state.selection;
    if (!empty) {
      chain.setTextSelection({ from, to }).deleteSelection();
    }
    chain
      .insertContent({
        type: "text",
        text: title,
        marks: [{ type: "link", attrs: { href } }],
      })
      .run();
  }
  closeLinkForm();
}

function removeLinkFromForm() {
  const ed = editor.value;
  const range = linkFormRange.value;
  if (!ed || !range) return;
  ed.chain().focus().setTextSelection(range).unsetLink().run();
  closeLinkForm();
}

function viewHoveredLink(e: MouseEvent) {
  void openLinkPopoverFromHover("view", e);
}

function onLinkHoverEnter() {
  clearLinkHoverHideTimer();
}

function onLinkHoverLeave() {
  scheduleHideLinkHover();
}
</script>

<template>
  <div ref="editorDropZoneRef" class="note-rich-root">
    <div v-show="editable" class="rte-toolbar">
      <button
        type="button"
        class="rte-tb-btn rte-tb-btn--wide"
        :disabled="tbDisabled()"
        title="正文段落"
        @click="editor?.chain().focus().setParagraph().run()"
      >
        正文
      </button>
      <button
        v-for="level in HEADING_LEVELS"
        :key="level"
        type="button"
        class="rte-tb-btn rte-tb-btn--heading"
        :class="{ on: editor?.isActive('heading', { level }) }"
        :disabled="tbDisabled()"
        :title="`标题 ${level}`"
        @click="editor?.chain().focus().toggleHeading({ level }).run()"
      >
        H{{ level }}
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn rte-tb-btn--icon"
        :class="{ on: editor?.isActive('bold') }"
        :disabled="tbDisabled()"
        title="加粗 (⌘B)"
        @click="editor?.chain().focus().toggleBold().run()"
      >
        <strong>B</strong>
      </button>
      <button
        type="button"
        class="rte-tb-btn rte-tb-btn--icon"
        :class="{ on: editor?.isActive('italic') }"
        :disabled="tbDisabled()"
        title="斜体 (⌘I)"
        @click="editor?.chain().focus().toggleItalic().run()"
      >
        <em>I</em>
      </button>
      <button
        type="button"
        class="rte-tb-btn rte-tb-btn--icon"
        :class="{ on: editor?.isActive('underline') }"
        :disabled="tbDisabled()"
        title="下划线 (⌘U)"
        @click="editor?.chain().focus().toggleUnderline().run()"
      >
        <span class="rte-tb-u">U</span>
      </button>
      <button
        type="button"
        class="rte-tb-btn rte-tb-btn--icon"
        :class="{ on: editor?.isActive('strike') }"
        :disabled="tbDisabled()"
        title="删除线"
        @click="editor?.chain().focus().toggleStrike().run()"
      >
        <s>S</s>
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('code') }"
        :disabled="tbDisabled()"
        title="行内代码"
        @click="editor?.chain().focus().toggleCode().run()"
      >
        &lt;/&gt;
      </button>
      <div ref="linkBtnRef" class="rte-link-btn-wrap">
        <button
          type="button"
          class="rte-tb-btn"
          :class="{ on: editor?.isActive('link') || linkFormOpen }"
          :disabled="tbDisabled()"
          title="插入或编辑链接"
          @click="openLinkFormFromToolbar"
        >
          链接
        </button>
      </div>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('bulletList') }"
        :disabled="tbDisabled()"
        title="无序列表"
        @click="editor?.chain().focus().toggleBulletList().run()"
      >
        • 列表
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('orderedList') }"
        :disabled="tbDisabled()"
        title="有序列表"
        @click="editor?.chain().focus().toggleOrderedList().run()"
      >
        1. 列表
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbSinkListDisabled()"
        title="增加列表缩进"
        @click="editor?.chain().focus().sinkListItem('listItem').run()"
      >
        缩进+
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbLiftListDisabled()"
        title="减少列表缩进"
        @click="editor?.chain().focus().liftListItem('listItem').run()"
      >
        缩进−
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('blockquote') }"
        :disabled="tbDisabled()"
        title="引用块"
        @click="editor?.chain().focus().toggleBlockquote().run()"
      >
        引用
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('codeBlock') }"
        :disabled="tbDisabled()"
        title="代码块"
        @click="editor?.chain().focus().toggleCodeBlock().run()"
      >
        代码块
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="分隔线"
        @click="editor?.chain().focus().setHorizontalRule().run()"
      >
        分隔线
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="软换行 (Shift+Enter)"
        @click="editor?.chain().focus().setHardBreak().run()"
      >
        换行
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="导入图片"
        @click="pickAndImportImage"
      >
        图片
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="导入视频"
        @click="pickAndImportVideo"
      >
        视频
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbUndoDisabled()"
        title="撤销 (⌘Z)"
        @click="editor?.chain().focus().undo().run()"
      >
        撤销
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbRedoDisabled()"
        title="重做 (⌘⇧Z)"
        @click="editor?.chain().focus().redo().run()"
      >
        重做
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="清除选区格式"
        @click="clearFormatting"
      >
        清除格式
      </button>
    </div>
    <div
      class="rte-editor-wrap"
      :class="{ 'rte-editor-wrap--drag': dragOverlayVisible }"
      @dragenter="onEditorDragEnter"
      @dragover="onEditorDragOver"
      @dragleave="onEditorDragLeave"
      @drop="onEditorDrop"
    >
      <editor-content v-if="editor" :editor="editor" />
      <Transition name="rte-drop-fade">
        <div
          v-if="editable && dragOverlayVisible"
          class="rte-drop-overlay"
          aria-hidden="true"
        >
          <div class="rte-drop-preview">
            <img
              v-if="dragPreviewUrl"
              :src="dragPreviewUrl"
              class="rte-drop-preview-img"
              alt=""
            />
            <div v-else class="rte-drop-preview-icon">
              {{ dragPreviewKind === "video" ? "🎥" : "📷" }}
            </div>
            <p class="rte-drop-preview-label">{{ dragPreviewLabel }}</p>
            <p class="rte-drop-preview-hint">松开鼠标即可插入到笔记</p>
          </div>
        </div>
      </Transition>
      <div v-if="isImporting" class="rte-importing-badge">正在导入媒体…</div>
      <div v-if="!editable" class="rte-mask">
        <p class="rte-mask-text">在中间列表选择一条笔记，或点击左侧「新建笔记」。</p>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="linkFormOpen"
        ref="linkPopoverRef"
        class="rte-link-popover"
        role="dialog"
        :aria-label="linkFormTitleLabel"
        :style="linkPopoverStyle"
        @mousedown.stop
      >
        <p class="rte-link-popover-title">{{ linkFormTitleLabel }}</p>
        <label class="rte-link-field">
          <span class="rte-link-field-label">显示标题</span>
          <input
            v-model="linkFormTitle"
            type="text"
            class="rte-link-field-input"
            :class="{ 'rte-link-field-input--readonly': linkFormMode === 'view' }"
            :readonly="linkFormMode === 'view'"
            :tabindex="linkFormMode === 'view' ? -1 : 0"
            placeholder="链接显示文字"
            @keydown.enter.prevent="linkFormMode !== 'view' && submitLinkForm()"
          />
        </label>
        <label class="rte-link-field">
          <span class="rte-link-field-label">链接地址</span>
          <input
            v-model="linkFormHref"
            type="url"
            class="rte-link-field-input"
            :class="{ 'rte-link-field-input--readonly': linkFormMode === 'view' }"
            :readonly="linkFormMode === 'view'"
            :tabindex="linkFormMode === 'view' ? -1 : 0"
            placeholder="https://"
            @keydown.enter.prevent="linkFormMode !== 'view' && submitLinkForm()"
          />
        </label>
        <div class="rte-link-popover-actions">
          <template v-if="linkFormMode === 'view'">
            <button type="button" class="rte-link-action rte-link-action--primary" @click="closeLinkForm">
              关闭
            </button>
          </template>
          <template v-else>
            <button
              v-if="linkFormRange"
              type="button"
              class="rte-link-action rte-link-action--danger"
              @click="removeLinkFromForm"
            >
              移除
            </button>
            <button type="button" class="rte-link-action" @click="closeLinkForm">
              取消
            </button>
            <button
              type="button"
              class="rte-link-action rte-link-action--primary"
              @click="submitLinkForm"
            >
              确定
            </button>
          </template>
        </div>
      </div>

      <div
        v-if="linkHover.visible && !linkFormOpen"
        class="rte-link-hover"
        :style="{
          top: linkHover.top + 'px',
          left: linkHover.left + 'px',
        }"
        @mouseenter="onLinkHoverEnter"
        @mouseleave="onLinkHoverLeave"
        @mousedown.stop
      >
        <button
          type="button"
          class="rte-link-hover-btn"
          @mousedown.prevent.stop="viewHoveredLink"
          @click.prevent.stop="viewHoveredLink"
        >
          查看
        </button>
        <button
          type="button"
          class="rte-link-hover-btn"
          @mousedown.prevent.stop="openLinkFormFromHover"
          @click.prevent.stop="openLinkFormFromHover"
        >
          修改
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.note-rich-root {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: #fafbfc;
}

.rte-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--line, #e8eaed);
  background: #fff;
  flex-shrink: 0;
  row-gap: 6px;
}

.rte-tb-btn {
  border: 1px solid transparent;
  background: #f0f2f5;
  border-radius: 6px;
  padding: 5px 9px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  color: #333;
  font-family: inherit;
  line-height: 1.2;
  white-space: nowrap;
}

.rte-tb-btn--wide {
  min-width: 40px;
}

.rte-tb-btn--icon {
  min-width: 30px;
  padding: 5px 7px;
  text-align: center;
}

.rte-tb-btn--icon strong,
.rte-tb-btn--icon em,
.rte-tb-btn--icon s {
  font-size: 13px;
  font-style: normal;
  font-weight: 700;
}

.rte-tb-btn--icon em {
  font-style: italic;
  font-weight: 600;
}

.rte-tb-u {
  text-decoration: underline;
}

.rte-tb-btn--heading {
  min-width: 28px;
  padding: 5px 6px;
}

.rte-link-btn-wrap {
  display: inline-flex;
}

.rte-link-popover {
  position: fixed;
  z-index: 200;
  width: 300px;
  padding: 14px 14px 12px;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  box-shadow:
    0 8px 30px rgba(15, 23, 42, 0.12),
    0 2px 8px rgba(15, 23, 42, 0.06);
}

.rte-link-popover-title {
  margin: 0 0 10px;
  font-size: 13px;
  font-weight: 700;
  color: #111827;
}

.rte-link-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 10px;
}

.rte-link-field-label {
  font-size: 12px;
  font-weight: 600;
  color: #6b7280;
}

.rte-link-field-input {
  width: 100%;
  box-sizing: border-box;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-family: inherit;
  color: #111827;
  outline: none;
  transition: border-color 0.15s ease;
}

.rte-link-field-input:focus {
  border-color: var(--accent, #2563eb);
}

.rte-link-field-input--readonly {
  background: #f9fafb;
  color: #374151;
  cursor: default;
}

.rte-link-field-input--readonly:focus {
  border-color: #e5e7eb;
}

.rte-link-popover-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 4px;
}

.rte-link-action {
  border: 1px solid #e5e7eb;
  background: #fff;
  border-radius: 8px;
  padding: 6px 12px;
  font-size: 12px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  color: #374151;
}

.rte-link-action:hover {
  background: #f9fafb;
}

.rte-link-action--primary {
  border-color: var(--accent, #2563eb);
  background: var(--accent, #2563eb);
  color: #fff;
}

.rte-link-action--primary:hover {
  background: #1d4ed8;
}

.rte-link-action--danger {
  margin-right: auto;
  border-color: #fecaca;
  color: #dc2626;
}

.rte-link-action--danger:hover {
  background: #fef2f2;
}

.rte-link-hover {
  position: fixed;
  z-index: 199;
  transform: translateX(-50%);
  display: flex;
  gap: 4px;
  padding: 4px;
  background: #fff;
  border: 1px solid #e5e7eb;
  border-radius: 10px;
  box-shadow: 0 4px 16px rgba(15, 23, 42, 0.12);
}

.rte-link-hover-btn {
  border: none;
  background: #f3f4f6;
  border-radius: 7px;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  color: #374151;
  white-space: nowrap;
}

.rte-link-hover-btn:hover {
  background: var(--accent-sidebar-active, #e8f1fe);
  color: var(--accent, #2563eb);
}

.rte-tb-btn:hover:not(:disabled) {
  background: #e4e6ea;
}

.rte-tb-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.rte-tb-btn.on {
  border-color: var(--accent, #007aff);
  color: var(--accent, #007aff);
  background: rgba(0, 122, 255, 0.08);
}

.rte-tb-sep {
  width: 1px;
  height: 18px;
  background: var(--line, #e8eaed);
  margin: 0 4px;
}

.rte-editor-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
  position: relative;
}

.rte-editor-wrap--drag :deep(.note-editor-prosemirror) {
  pointer-events: none;
}

.rte-drop-overlay {
  position: absolute;
  inset: 12px;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 122, 255, 0.08);
  border: 2px dashed var(--accent, #007aff);
  border-radius: 12px;
  pointer-events: none;
  backdrop-filter: blur(2px);
}

.rte-drop-preview {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 20px 28px;
  max-width: min(360px, 90%);
  text-align: center;
}

.rte-drop-preview-img {
  max-width: 240px;
  max-height: 180px;
  object-fit: contain;
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  background: #fff;
}

.rte-drop-preview-icon {
  font-size: 48px;
  line-height: 1;
}

.rte-drop-preview-label {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: #1f2937;
  word-break: break-all;
}

.rte-drop-preview-hint {
  margin: 0;
  font-size: 13px;
  color: var(--accent, #007aff);
}

.rte-drop-fade-enter-active,
.rte-drop-fade-leave-active {
  transition: opacity 0.15s ease;
}

.rte-drop-fade-enter-from,
.rte-drop-fade-leave-to {
  opacity: 0;
}

.rte-importing-badge {
  position: absolute;
  bottom: 16px;
  right: 16px;
  z-index: 4;
  padding: 6px 12px;
  font-size: 12px;
  color: #fff;
  background: rgba(0, 0, 0, 0.65);
  border-radius: 8px;
  pointer-events: none;
}

.rte-mask {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 48px;
  background: #fafbfc;
  z-index: 1;
}

.rte-mask-text {
  margin: 0;
  color: var(--muted, #8a8f98);
  font-size: 14px;
  text-align: center;
  padding: 0 24px;
}

:deep(.note-editor-prosemirror) {
  min-height: 100%;
  padding: 16px 28px 40px;
  outline: none;
  font-size: 15px;
  line-height: 1.65;
  color: #111827;
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
}

:deep(.note-editor-prosemirror p) {
  margin: 0 0 0.6em;
}

:deep(.note-editor-prosemirror h1) {
  font-size: 1.75em;
  margin: 0.85em 0 0.45em;
  letter-spacing: -0.02em;
}

:deep(.note-editor-prosemirror h2) {
  font-size: 1.45em;
  margin: 0.8em 0 0.4em;
}

:deep(.note-editor-prosemirror h3) {
  font-size: 1.25em;
  margin: 0.7em 0 0.35em;
}

:deep(.note-editor-prosemirror h4) {
  font-size: 1.1em;
  margin: 0.65em 0 0.3em;
}

:deep(.note-editor-prosemirror h5) {
  font-size: 1em;
  margin: 0.6em 0 0.28em;
  font-weight: 700;
}

:deep(.note-editor-prosemirror h6) {
  font-size: 0.95em;
  margin: 0.55em 0 0.25em;
  font-weight: 700;
  color: #4b5563;
}

:deep(.note-editor-prosemirror ul),
:deep(.note-editor-prosemirror ol) {
  margin: 0 0 0.6em 1.25em;
  padding: 0;
}

:deep(.note-editor-prosemirror blockquote) {
  margin: 0 0 0.6em;
  padding-left: 12px;
  border-left: 3px solid #c5cad3;
  color: #444;
}

:deep(.note-editor-prosemirror code) {
  background: #f3f4f6;
  padding: 0.12em 0.35em;
  border-radius: 4px;
  font-size: 0.9em;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

:deep(.note-editor-prosemirror pre) {
  background: #1e293b;
  color: #e2e8f0;
  padding: 12px 14px;
  border-radius: 8px;
  overflow-x: auto;
  margin: 0 0 0.75em;
  font-size: 13px;
  line-height: 1.5;
}

:deep(.note-editor-prosemirror pre code) {
  background: none;
  padding: 0;
  color: inherit;
  font-size: inherit;
}

:deep(.note-editor-prosemirror hr) {
  border: none;
  border-top: 1px solid #e5e7eb;
  margin: 1em 0;
}

:deep(.note-editor-prosemirror a.notebook-inline-link) {
  color: var(--accent, #2563eb);
  text-decoration: underline;
  cursor: pointer;
}

:deep(.note-editor-prosemirror p.is-editor-empty:first-child::before) {
  color: #adb5bd;
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
}

:deep(.note-editor-prosemirror img.notebook-inline-media) {
  max-width: 100%;
  height: auto;
  border-radius: 8px;
  margin: 8px 0;
  display: block;
}



:deep(.note-editor-prosemirror .ProseMirror-selectednode) {
  outline: 2px solid var(--accent, #007aff);
  border-radius: 8px;
}
</style>
