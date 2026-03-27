<script setup lang="ts">
import type { ListSortMode } from "../types/notebook";

defineProps<{
  open: boolean;
  sortMode: ListSortMode;
  onlyWithText: boolean;
  disabled: boolean;
}>();

const emit = defineEmits<{
  "update:open": [v: boolean];
  "update:sortMode": [v: ListSortMode];
  "update:onlyWithText": [v: boolean];
}>();

const sortOptions: { value: ListSortMode; label: string }[] = [
  { value: "updated_desc", label: "最近更新" },
  { value: "updated_asc", label: "最早更新" },
  { value: "title_asc", label: "标题 A → Z" },
  { value: "title_desc", label: "标题 Z → A" },
];

function pickSort(mode: ListSortMode) {
  emit("update:sortMode", mode);
  emit("update:open", false);
}
</script>

<template>
  <div class="lfm">
    <button
      type="button"
      class="list-filter-btn"
      title="排序与筛选"
      aria-label="排序与筛选"
      aria-haspopup="true"
      :aria-expanded="open"
      :disabled="disabled"
      @click="disabled ? undefined : emit('update:open', !open)"
    >
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
      >
        <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
      </svg>
    </button>
    <Transition name="lfm-fade">
      <div v-if="open && !disabled" class="lfm-popover" role="menu" @mousedown.stop>
        <div class="lfm-section">
          <div class="lfm-label">排序</div>
          <button
            v-for="o in sortOptions"
            :key="o.value"
            type="button"
            class="lfm-item"
            :class="{ active: sortMode === o.value }"
            role="menuitem"
            @click="pickSort(o.value)"
          >
            {{ o.label }}
          </button>
        </div>
        <label class="lfm-check">
          <input
            type="checkbox"
            :checked="onlyWithText"
            @change="emit('update:onlyWithText', ($event.target as HTMLInputElement).checked)"
          />
          <span>仅显示有正文字数的笔记</span>
        </label>
        <p class="lfm-hint">
          在「全部笔记」或文件夹内，按住笔记左侧握把拖动排序，松手后保存；拖到左侧文件夹可移动笔记。开启「仅显示有正文字数」时不支持排序。
        </p>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.lfm {
  position: relative;
  flex-shrink: 0;
}

.list-filter-btn {
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.75);
  color: var(--muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(15, 23, 42, 0.06);
}

.list-filter-btn:hover:not(:disabled) {
  color: var(--accent);
}

.list-filter-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.lfm-popover {
  position: absolute;
  right: 0;
  top: calc(100% + 6px);
  z-index: 50;
  min-width: 220px;
  padding: 10px 0;
  background: #fff;
  border: 1px solid var(--line);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(15, 23, 42, 0.12);
}

.lfm-section {
  padding: 0 4px 8px;
  border-bottom: 1px solid var(--line);
  margin-bottom: 8px;
}

.lfm-label {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: #9ca3af;
  padding: 4px 12px 6px;
}

.lfm-item {
  display: block;
  width: calc(100% - 8px);
  margin: 0 4px;
  padding: 8px 12px;
  border: none;
  border-radius: 8px;
  background: transparent;
  text-align: left;
  font-size: 13px;
  font-family: inherit;
  color: var(--text);
  cursor: pointer;
}

.lfm-item:hover {
  background: rgba(37, 99, 235, 0.08);
}

.lfm-item.active {
  color: var(--accent);
  font-weight: 600;
  background: var(--accent-soft);
}

.lfm-check {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 14px 10px;
  font-size: 13px;
  color: var(--text);
  cursor: pointer;
  user-select: none;
}

.lfm-check input {
  width: 15px;
  height: 15px;
  accent-color: var(--accent);
}

.lfm-hint {
  margin: 0;
  padding: 0 14px 4px;
  font-size: 11px;
  line-height: 1.45;
  color: #9ca3af;
}

.lfm-fade-enter-active,
.lfm-fade-leave-active {
  transition: opacity 0.12s ease;
}

.lfm-fade-enter-from,
.lfm-fade-leave-to {
  opacity: 0;
}
</style>
