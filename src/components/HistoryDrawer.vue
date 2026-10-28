<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { diffNoteHtml, historyTranscript, type NoteDiff } from "../utils/htmlDiff";

const props = defineProps<{
  open: boolean;
  noteId: string | null;
}>();

const emit = defineEmits<{
  close: [];
}>();

type Meta = { id: string; title: string; startedAt: string; updatedAt: string };
type Detail = {
  id: string;
  title: string;
  contentHtml: string;
  startedAt: string;
  updatedAt: string;
  previousHtml: string | null;
};

const list = ref<Meta[]>([]);
const selectedId = ref<string | null>(null);
const detail = ref<Detail | null>(null);
const previewHtml = ref("");
const loading = ref(false);
const errorText = ref("");

const diff = computed<NoteDiff | null>(() => {
  if (!detail.value?.previousHtml) return null;
  return diffNoteHtml(detail.value.previousHtml, detail.value.contentHtml);
});

function pad(n: number) {
  return String(n).padStart(2, "0");
}

function formatTime(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function rangeLabel(item: Meta) {
  const start = formatTime(item.startedAt);
  const end = formatTime(item.updatedAt);
  return start === end ? start : `${start} – ${end.slice(11)}`;
}

async function loadDetail(id: string) {
  selectedId.value = id;
  const row = await invoke<Detail | null>("get_note_revision", { revisionId: id });
  detail.value = row;
  previewHtml.value = row ? historyTranscript(row.contentHtml) : "";
}

watch(
  () => [props.open, props.noteId] as const,
  async ([open, noteId]) => {
    if (!open || !noteId) return;
    loading.value = true;
    errorText.value = "";
    detail.value = null;
    previewHtml.value = "";
    try {
      list.value = await invoke<Meta[]>("list_note_revisions", { noteId });
      const first = list.value[0]?.id ?? null;
      selectedId.value = first;
      if (first) await loadDetail(first);
    } catch (error) {
      errorText.value = error instanceof Error ? error.message : String(error);
      list.value = [];
    } finally {
      loading.value = false;
    }
  },
);
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="hd-root">
      <button type="button" class="hd-mask" aria-label="关闭历史版本" @click="emit('close')" />
      <aside class="hd-panel" role="dialog" aria-label="历史版本">
        <header class="hd-head">
          <div>
            <h2>历史版本</h2>
            <p>1 小时内的修改算同一次，只保留最近 20 条。只能查看。</p>
          </div>
          <button type="button" class="hd-close" @click="emit('close')">关闭</button>
        </header>

        <p v-if="loading" class="hd-hint">正在读取…</p>
        <p v-else-if="errorText" class="hd-hint hd-hint--bad">{{ errorText }}</p>
        <p v-else-if="list.length === 0" class="hd-hint">还没有历史版本。编辑并保存后，会从这一小时开始记录。</p>

        <div v-else class="hd-list">
          <button
            v-for="(item, index) in list"
            :key="item.id"
            type="button"
            class="hd-ver"
            :class="{ 'hd-ver--on': item.id === selectedId }"
            @click="loadDetail(item.id)"
          >
            <span class="hd-ver-index">{{ list.length - index }}</span>
            <span>
              <strong>{{ rangeLabel(item) }}</strong>
              <small>{{ item.title || "未命名笔记" }}</small>
            </span>
          </button>
        </div>

        <div v-if="detail" class="hd-body">
          <section class="hd-diff">
            <h3>修改说明</h3>
            <p v-if="!detail.previousHtml" class="hd-quiet">这是最早保留的版本，没有更早的记录可对照。</p>
            <template v-else-if="diff">
              <p class="hd-summary">
                相对上一版本：新增 {{ diff.added }} 处，删除 {{ diff.removed }} 处
              </p>
              <p v-if="diff.formatNote" class="hd-quiet">{{ diff.formatNote }}</p>
              <p v-if="diff.rows.length === 0 && !diff.mediaNote && !diff.formatNote" class="hd-quiet">
                这一小时里的文字和代码，与上一版本相同。
              </p>
              <ul v-if="diff.rows.length" class="hd-rows">
                <li
                  v-for="(row, index) in diff.rows"
                  :key="index"
                  class="hd-row"
                  :class="`hd-row--${row.kind}`"
                >
                  <span class="hd-time">{{ formatTime(detail.updatedAt) }}</span>
                  <span v-if="row.code" class="hd-code">代码</span>
                  <template v-if="row.kind === 'change'">
                    从「<span class="hd-from">{{ row.from }}</span>」改为「<span class="hd-to">{{ row.to }}</span>」
                  </template>
                  <template v-else-if="row.kind === 'add'">
                    从「无」改为「<span class="hd-to">{{ row.text }}</span>」
                  </template>
                  <template v-else>
                    从「<span class="hd-from">{{ row.text }}</span>」改为「无」
                  </template>
                </li>
              </ul>
              <p v-if="diff.truncated" class="hd-quiet">改动较多，只展示前 200 处。</p>
            </template>
          </section>

          <section class="hd-article">
            <h3>当时的正文</h3>
            <p class="hd-quiet">图像和视频只显示名称，不保留具体文件。</p>
            <h4>{{ detail.title || "未命名笔记" }}</h4>
            <div class="hd-preview" v-html="previewHtml" />
          </section>
        </div>
      </aside>
    </div>
  </Teleport>
</template>

<style scoped>
.hd-root {
  position: fixed;
  inset: 0;
  z-index: 70;
}

.hd-mask {
  position: absolute;
  inset: 0;
  border: none;
  background: rgba(15, 23, 42, 0.28);
  cursor: pointer;
}

.hd-panel {
  position: absolute;
  top: 0;
  right: 0;
  height: 100%;
  width: min(460px, 100vw);
  background: var(--paper, #f8fafc);
  box-shadow: -16px 0 40px rgba(15, 23, 42, 0.12);
  display: flex;
  flex-direction: column;
  animation: hd-in 0.2s ease;
  font-family: var(--font);
  font-size: 14px;
  color: var(--text, #111827);
}

@keyframes hd-in {
  from { transform: translateX(16px); opacity: 0.6; }
  to { transform: none; opacity: 1; }
}

.hd-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 18px 18px 12px;
  background: var(--toolbar, #fff);
  border-bottom: 1px solid var(--line, #e5e7eb);
}

.hd-head h2 {
  margin: 0;
  font-size: 18px;
}

.hd-head p {
  margin: 4px 0 0;
  color: var(--muted, #6b7280);
  font-size: 12px;
  line-height: 1.45;
}

.hd-close {
  height: 32px;
  padding: 0 12px;
  border-radius: 8px;
  border: 1px solid var(--line, #e5e7eb);
  background: var(--card, #fff);
  cursor: pointer;
}

.hd-hint {
  margin: 16px;
  color: var(--muted, #6b7280);
  font-size: 13px;
}

.hd-hint--bad {
  color: #b91c1c;
}

.hd-list {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding: 12px 16px;
  background: var(--card, #fff);
  border-bottom: 1px solid #e5e7eb;
}

.hd-ver {
  flex: 0 0 auto;
  display: flex;
  gap: 8px;
  align-items: center;
  max-width: 220px;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid var(--line, #e5e7eb);
  background: var(--card, #fff);
  text-align: left;
  cursor: pointer;
}

.hd-ver strong,
.hd-ver small {
  display: block;
}

.hd-ver small {
  color: var(--muted, #6b7280);
  font-size: 11px;
  margin-top: 2px;
}

.hd-ver-index {
  width: 22px;
  height: 22px;
  border-radius: 99px;
  background: #eef2ff;
  color: #3730a3;
  font-size: 12px;
  display: grid;
  place-items: center;
}

.hd-ver--on {
  border-color: #93c5fd;
  background: #eff6ff;
}

.hd-body {
  flex: 1;
  overflow: auto;
  padding: 14px 16px 28px;
}

.hd-diff,
.hd-article {
  background: var(--card, #fff);
  border: 1px solid var(--line, #e5e7eb);
  border-radius: 12px;
  padding: 12px 14px;
  margin-bottom: 12px;
}

.hd-diff h3,
.hd-article h3 {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--muted, #6b7280);
  font-weight: 600;
}

.hd-article h4 {
  margin: 0 0 10px;
  font-size: 20px;
}

.hd-summary {
  margin: 0 0 8px;
  font-size: 13px;
}

.hd-quiet,
.hd-media {
  margin: 0 0 8px;
  font-size: 13px;
  color: #4b5563;
}

.hd-media {
  color: #9a3412;
  background: #fff7ed;
  border-radius: 8px;
  padding: 8px 10px;
}

.hd-rows {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.hd-row {
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  line-height: 1.55;
  word-break: break-word;
}

.hd-row--add {
  background: #dcfce7;
  color: #166534;
}

.hd-row--del {
  background: #fee2e2;
  color: #991b1b;
}

.hd-row--change {
  background: #f8fafc;
  border: 1px solid var(--line, #e5e7eb);
}

.hd-time {
  display: block;
  margin-bottom: 4px;
  color: var(--muted, #6b7280);
  font-size: 12px;
}

.hd-from {
  display: inline;
  background: #fee2e2;
  color: #991b1b;
  text-decoration: line-through;
  border-radius: 4px;
  padding: 0 4px;
}

.hd-to {
  display: inline;
  background: #dcfce7;
  color: #166534;
  border-radius: 4px;
  padding: 0 4px;
}

.hd-arrow,
.hd-tag,
.hd-code {
  display: inline-block;
  margin-right: 6px;
  font-size: 11px;
  color: var(--muted, #6b7280);
}

.hd-code {
  background: #ede9fe;
  color: #5b21b6;
  border-radius: 999px;
  padding: 0 6px;
}

.hd-preview {
  font-size: 15px;
  line-height: 1.7;
  color: #111827;
  font-family: inherit;
}

.hd-preview :deep(pre),
.hd-preview :deep(code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

.hd-preview :deep(img),
.hd-preview :deep(video) {
  max-width: 100%;
  height: auto;
  border-radius: 8px;
}

.hd-preview :deep(pre) {
  background: #0f172a;
  color: #e2e8f0;
  padding: 12px;
  border-radius: 8px;
  overflow: auto;
}

.hd-preview :deep(p) {
  margin: 0 0 0.6em;
}
</style>
