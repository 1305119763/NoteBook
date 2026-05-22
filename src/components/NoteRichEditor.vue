<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import Placeholder from "@tiptap/extension-placeholder";
import StarterKit from "@tiptap/starter-kit";
import Image from "@tiptap/extension-image";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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
    handleDrop: (view, event, slice, moved) => {
      if (!event.dataTransfer || !props.editable) return false;
      const files = event.dataTransfer.files;
      if (files.length > 0) {
        handleFiles(Array.from(files));
        return true;
      }
      return false;
    },
    handlePaste: (view, event) => {
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
  },
);

onBeforeUnmount(() => {
  editor.value?.destroy();
});

async function handleFiles(files: File[]) {
  const ed = editor.value;
  if (!ed) return;
  isImporting.value = true;

  const ALLOWED_EXTS = ["jpg", "jpeg", "png", "gif", "webp", "svg", "mp4", "webm", "mov"];

  for (const file of files) {
    const extFromName = file.name.split(".").pop()?.toLowerCase() || "";
    const extFromType = file.type.split("/").pop()?.toLowerCase() || "";
    // 优先使用文件名扩展名，若无效则从 MIME 类型推断（覆盖粘贴截图等文件名缺失场景）
    const ext = ALLOWED_EXTS.includes(extFromName) ? extFromName : extFromType;
    const isImage = ["jpg", "jpeg", "png", "gif", "webp", "svg"].includes(ext);
    const isVideo = ["mp4", "webm", "mov"].includes(ext);
    if (!isImage && !isVideo) continue;

    try {
      const buffer = await file.arrayBuffer();
      const result = await invoke("import_media_bytes", {
        data: Array.from(new Uint8Array(buffer)),
        extension: ext,
      }) as { mediaSrc: string; storageKey: string; absolutePath: string };

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
  isImporting.value = true;

  for (const filePath of paths) {
    try {
      const result = await invoke("import_media_file", {
        filePath,
      }) as { mediaSrc: string; storageKey: string; absolutePath: string };

      const ext = filePath.split(".").pop()?.toLowerCase() || "";
      const isImage = ["jpg", "jpeg", "png", "gif", "webp", "svg"].includes(ext);

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
  <div class="note-rich-root">
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
    <div class="rte-editor-wrap">
      <editor-content v-if="editor" :editor="editor" />
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
