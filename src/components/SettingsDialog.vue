<script setup lang="ts">
import { watch, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";

export type AppSettings = {
  theme: "system" | "light" | "dark";
  skin: "blue" | "green" | "violet" | "amber" | "slate";
  nickname: string;
  useDefaultNames: boolean;
};

const props = defineProps<{
  open: boolean;
  settings: AppSettings;
  path: string;
  focus: "general" | "about";
}>();

const emit = defineEmits<{
  close: [];
  save: [value: AppSettings];
}>();

const draft = ref<AppSettings>({ ...props.settings });
const aboutRef = ref<HTMLElement | null>(null);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    draft.value = { ...props.settings };
    if (props.focus === "about") {
      requestAnimationFrame(() => aboutRef.value?.scrollIntoView({ block: "start" }));
    }
  },
);

const skins: { id: AppSettings["skin"]; label: string; color: string }[] = [
  { id: "blue", label: "蓝", color: "#2563eb" },
  { id: "green", label: "绿", color: "#059669" },
  { id: "violet", label: "紫", color: "#7c3aed" },
  { id: "amber", label: "琥珀", color: "#d97706" },
  { id: "slate", label: "石板", color: "#475569" },
];

function commit() {
  emit("save", { ...draft.value, nickname: draft.value.nickname.trim() });
}

function openLink(url: string) {
  void openUrl(url).catch(() => window.open(url));
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="set-root">
      <button type="button" class="set-mask" aria-label="关闭设置" @click="emit('close')" />
      <section class="set-card" role="dialog" aria-label="软件设置">
        <header class="set-head">
          <h2>软件设置</h2>
          <button type="button" class="set-x" aria-label="关闭" @click="emit('close')">×</button>
        </header>
        <div class="set-body">
          <section>
            <h3>外观</h3>
            <div class="set-row">
              <span>风格</span>
              <div class="set-seg">
                <button type="button" :class="{ on: draft.theme === 'light' }" @click="draft.theme = 'light'">浅色</button>
                <button type="button" :class="{ on: draft.theme === 'dark' }" @click="draft.theme = 'dark'">深色</button>
                <button type="button" :class="{ on: draft.theme === 'system' }" @click="draft.theme = 'system'">跟随系统</button>
              </div>
            </div>
            <div class="set-row">
              <span>皮肤</span>
              <div class="set-skins">
                <button
                  v-for="skin in skins"
                  :key="skin.id"
                  type="button"
                  class="set-skin"
                  :class="{ on: draft.skin === skin.id }"
                  :title="skin.label"
                  @click="draft.skin = skin.id"
                >
                  <i :style="{ background: skin.color }" />
                  {{ skin.label }}
                </button>
              </div>
            </div>
          </section>
          <section>
            <h3>本机身份</h3>
            <label class="set-field">
              <span>本 IP 昵称</span>
              <input v-model="draft.nickname" maxlength="24" placeholder="留空则只显示 IP" />
            </label>
            <p class="set-hint">分享笔记时，对方看到的来源会带上这个昵称。</p>
          </section>
          <section>
            <h3>新建</h3>
            <label class="set-check">
              <input v-model="draft.useDefaultNames" type="checkbox" />
              <span>新建笔记和文件夹时使用默认名称，不再弹出填写框</span>
            </label>
            <p class="set-hint">关闭时保持现在的弹窗。开启后默认名为「未命名笔记」「新建文件夹」，重名会自动加序号。</p>
          </section>
          <section ref="aboutRef" class="set-about">
            <h3>关于</h3>
            <p>作者 T+</p>
            <p>邮箱 <a href="mailto:747896298@qq.com" @click.prevent="openLink('mailto:747896298@qq.com')">747896298@qq.com</a></p>
            <p>
              开源地址
              <a href="https://github.com/1305119763/NoteBook" @click.prevent="openLink('https://github.com/1305119763/NoteBook')">github.com/1305119763/NoteBook</a>
            </p>
            <p v-if="path" class="set-path">设置文件：{{ path }}</p>
          </section>
        </div>
        <footer class="set-foot">
          <button type="button" class="set-ghost" @click="emit('close')">取消</button>
          <button type="button" class="set-ok" @click="commit">保存</button>
        </footer>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.set-root {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  place-items: center;
  padding: 24px;
}
.set-mask {
  position: absolute;
  inset: 0;
  border: none;
  background: rgba(15, 23, 42, 0.38);
  cursor: pointer;
}
.set-card {
  position: relative;
  width: min(480px, 100%);
  max-height: min(78vh, 640px);
  display: flex;
  flex-direction: column;
  background: var(--surface, #fff);
  color: var(--surface-text, #111827);
  border-radius: 16px;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.22);
  font-family: inherit;
}
.set-head, .set-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
}
.set-head { border-bottom: 1px solid var(--line, #e5e7eb); }
.set-foot { border-top: 1px solid var(--line, #e5e7eb); gap: 8px; justify-content: flex-end; }
.set-head h2 { margin: 0; font-size: 16px; }
.set-x, .set-ghost, .set-ok {
  border: none;
  background: transparent;
  cursor: pointer;
  font-family: inherit;
}
.set-x { font-size: 22px; color: #6b7280; line-height: 1; }
.set-body { overflow: auto; padding: 8px 18px 16px; }
.set-body h3 { margin: 14px 0 8px; font-size: 13px; color: var(--muted, #6b7280); }
.set-row, .set-field { display: flex; flex-direction: column; gap: 8px; margin-bottom: 10px; font-size: 13px; }
.set-field input {
  border: 1px solid var(--line, #e5e7eb);
  border-radius: 8px;
  padding: 8px 10px;
  font: inherit;
  background: var(--field, #fff);
  color: var(--text, #111827);
}
.set-seg, .set-skins { display: flex; flex-wrap: wrap; gap: 6px; }
.set-seg button, .set-skin, .set-ghost, .set-ok {
  border-radius: 8px;
  padding: 6px 10px;
  font-size: 13px;
}
.set-seg button, .set-skin {
  border: 1px solid var(--line, #e5e7eb);
  background: var(--chip, #f8fafc);
  color: var(--chip-text, #111827);
  cursor: pointer;
  font-family: inherit;
}
.set-seg button.on, .set-skin.on { border-color: #2563eb; background: #eff6ff; color: #1d4ed8; }
.set-skin { display: inline-flex; align-items: center; gap: 6px; }
.set-skin i { width: 10px; height: 10px; border-radius: 99px; }
.set-check { display: flex; gap: 8px; align-items: flex-start; font-size: 13px; line-height: 1.45; }
.set-hint, .set-about p, .set-path { margin: 6px 0 0; font-size: 12px; color: var(--muted, #6b7280); line-height: 1.5; }
.set-about a { color: #2563eb; }
.set-ok { background: #2563eb; color: #fff; }
.set-ghost { background: var(--chip, #f3f4f6); color: var(--chip-text, #111827); }
</style>
