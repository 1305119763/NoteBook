<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import Placeholder from "@tiptap/extension-placeholder";
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
let dragPreviewObjectUrl: string | null = null;
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
      heading: { levels: [2, 3] },
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
    if (!v) clearDragOverlay();
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
});

onBeforeUnmount(() => {
  unlistenTauriDragDrop?.();
  unlistenTauriDragDrop = null;
  clearDragOverlay();
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
</script>

<template>
  <div ref="editorDropZoneRef" class="note-rich-root">
    <div v-show="editable" class="rte-toolbar">
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('bold') }"
        :disabled="tbDisabled()"
        title="加粗"
        @click="editor?.chain().focus().toggleBold().run()"
      >
        B
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('italic') }"
        :disabled="tbDisabled()"
        title="斜体"
        @click="editor?.chain().focus().toggleItalic().run()"
      >
        I
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('strike') }"
        :disabled="tbDisabled()"
        title="删除线"
        @click="editor?.chain().focus().toggleStrike().run()"
      >
        S
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('heading', { level: 2 }) }"
        :disabled="tbDisabled()"
        title="标题 2"
        @click="editor?.chain().focus().toggleHeading({ level: 2 }).run()"
      >
        H2
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('heading', { level: 3 }) }"
        :disabled="tbDisabled()"
        title="标题 3"
        @click="editor?.chain().focus().toggleHeading({ level: 3 }).run()"
      >
        H3
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('bulletList') }"
        :disabled="tbDisabled()"
        title="无序列表"
        @click="editor?.chain().focus().toggleBulletList().run()"
      >
        列表
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('orderedList') }"
        :disabled="tbDisabled()"
        title="有序列表"
        @click="editor?.chain().focus().toggleOrderedList().run()"
      >
        1.
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :class="{ on: editor?.isActive('blockquote') }"
        :disabled="tbDisabled()"
        title="引用"
        @click="editor?.chain().focus().toggleBlockquote().run()"
      >
        引用
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="导入图片"
        @click="pickAndImportImage"
      >
        📷
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="导入视频"
        @click="pickAndImportVideo"
      >
        🎥
      </button>
      <span class="rte-tb-sep" />
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="撤销"
        @click="editor?.chain().focus().undo().run()"
      >
        撤销
      </button>
      <button
        type="button"
        class="rte-tb-btn"
        :disabled="tbDisabled()"
        title="重做"
        @click="editor?.chain().focus().redo().run()"
      >
        重做
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
  gap: 4px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--line, #e8eaed);
  background: #fff;
  flex-shrink: 0;
}

.rte-tb-btn {
  border: 1px solid transparent;
  background: #f0f2f5;
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  color: #333;
  font-family: inherit;
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

:deep(.note-editor-prosemirror h2) {
  font-size: 1.35em;
  margin: 0.8em 0 0.4em;
}

:deep(.note-editor-prosemirror h3) {
  font-size: 1.15em;
  margin: 0.7em 0 0.35em;
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
