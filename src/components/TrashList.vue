<script setup lang="ts">
import type { TrashItemRow } from "../types/notebook";
import { trashItemKey, trashRemainingDays } from "../utils/trash";

defineProps<{
  items: TrashItemRow[];
  selectedKey: string | null;
}>();

const emit = defineEmits<{
  select: [item: TrashItemRow];
  restore: [item: TrashItemRow, e?: Event];
}>();
</script>

<template>
  <div class="trash-list-panel">
  <div v-if="items.length === 0" class="list-empty-hint">废纸篓为空</div>
  <ul v-else class="note-list note-list-cards">
    <li
      v-for="t in items"
      :key="trashItemKey(t)"
      class="note-row trash-row"
      :class="{ active: selectedKey === trashItemKey(t) }"
      @click="emit('select', t)"
    >
      <div class="note-main">
        <div class="trash-row-head">
          <div class="trash-row-text">
            <div class="note-title-row trash-title-row">
              <span class="trash-kind-label">{{
                t.kind === "folder" ? "文件夹" : "笔记"
              }}</span>
              <span class="note-title">{{ t.title }}</span>
            </div>
            <p v-if="t.kind === 'folder'" class="note-preview trash-folder-meta">
              {{ t.noteCount ?? 0 }} 个笔记
            </p>
          </div>
          <span class="note-time trash-days">剩余 {{ trashRemainingDays(t.deletedAt) }} 天</span>
        </div>
      </div>
      <button
        type="button"
        class="trash-restore-btn"
        title="恢复"
        @click="emit('restore', t, $event)"
      >
        恢复
      </button>
    </li>
  </ul>
  </div>
</template>

<style scoped>
.trash-list-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
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

.note-list-cards :deep(.note-row) {
  margin: 0 12px 10px;
}

.trash-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 14px 14px 14px 18px;
  cursor: pointer;
  position: relative;
  border-radius: 14px;
  border: 1px solid transparent;
  background: transparent;
  transition:
    background 0.15s ease,
    box-shadow 0.15s ease,
    border-color 0.15s ease;
}

.trash-row:hover {
  background: rgba(255, 255, 255, 0.72);
  border-color: #c5d0e0;
}

.trash-row.active {
  background: #fff;
  border-color: #b8c5d8;
  box-shadow: 0 4px 18px rgba(15, 23, 42, 0.1);
}

.note-main {
  flex: 1;
  min-width: 0;
}

.trash-row-head {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
}

.trash-row-text {
  flex: 1;
  min-width: 0;
}

.trash-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.trash-kind-label {
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 600;
  color: #9ca3af;
  letter-spacing: 0.02em;
}

.note-title {
  font-weight: 700;
  font-size: 14px;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  letter-spacing: -0.01em;
}

.trash-days {
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 500;
  color: var(--muted);
  align-self: center;
  line-height: 1.2;
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

.trash-folder-meta {
  margin-top: 6px;
  color: var(--muted);
}

.trash-restore-btn {
  flex-shrink: 0;
  border: 1px solid var(--line);
  background: #fff;
  color: var(--accent);
  font-size: 12px;
  font-weight: 600;
  padding: 6px 12px;
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
}

.trash-restore-btn:hover {
  background: var(--accent-soft);
  border-color: var(--accent);
}
</style>
