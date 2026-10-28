<script setup lang="ts">
import { computed, ref } from "vue";

const props = defineProps<{
  open: boolean;
  maxEditors: number;
  session: {
    noteId: string;
    title: string;
    url: string;
    ip: string;
    port: number;
    peers?: { ip: string; mode: string; color: string }[];
  } | null;
}>();

const emit = defineEmits<{
  close: [];
  stop: [];
  setMax: [n: number];
  revoke: [];
  revokeIp: [ip: string];
}>();

const hover = ref(false);
const pinned = ref(false);
const showTip = computed(() => hover.value || pinned.value);
const online = computed(() => props.session?.peers ?? []);
const editing = computed(() => online.value.filter((peer) => peer.mode === "edit"));

function onMax(e: Event) {
  const n = Number((e.target as HTMLInputElement).value);
  emit("setMax", n);
}

const copied = ref(false);

async function copyUrl(url: string) {
  try {
    await navigator.clipboard.writeText(url);
    copied.value = true;
    window.setTimeout(() => {
      copied.value = false;
    }, 1600);
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open" class="pp-root">
      <button type="button" class="pp-mask" aria-label="关闭演示面板" @click="emit('close')" />
      <section class="pp-card" role="dialog" aria-label="实时演示">
        <div class="pp-hero">
          <span class="pp-live">实时</span>
          <h2>局域网演示</h2>
          <p>把链接发给同一网络里的人。对方用浏览器打开后，只能看到这一篇笔记的实时变化。</p>
        </div>
        <div v-if="session" class="pp-body">
          <div class="pp-online-wrap">
            <button
              type="button"
              class="pp-online"
              @mouseenter="hover = true"
              @mouseleave="hover = false"
              @click="pinned = !pinned"
            >
              {{ online.length }} 人在线
            </button>
            <div v-if="showTip" class="pp-tip" @mouseenter="hover = true" @mouseleave="hover = false">
              <p v-if="!online.length">还没有其他人打开这篇笔记</p>
              <p v-for="peer in online" :key="peer.ip" class="pp-peer">
                <i :style="{ background: peer.mode === 'edit' ? peer.color || '#1d4ed8' : '#94a3b8' }" />
                <span>{{ peer.ip }} 正在{{ peer.mode === "edit" ? "编辑" : "查看" }}该笔记</span>
                <button v-if="peer.mode === 'edit'" type="button" @click.stop="emit('revokeIp', peer.ip)">停止编辑</button>
              </p>
            </div>
          </div>
          <label class="pp-max">
            <span>最多同时在线编辑</span>
            <input type="number" min="1" max="12" :value="maxEditors" @change="onMax" />
            <em>人不含本人</em>
          </label>
          <div class="pp-ip">
            <span>演示设备 IP</span>
            <strong>{{ session.ip }}</strong>
          </div>
          <p v-if="session.ip === '127.0.0.1'" class="pp-warn">没有拿到局域网地址。请确认这台电脑已连接 Wi-Fi 或网线后再开始。</p>
          <label class="pp-url-label">分享链接</label>
          <div class="pp-url">
            <code>{{ session.url }}</code>
            <button type="button" @click="copyUrl(session.url)">{{ copied ? "已复制" : "复制" }}</button>
          </div>
          <p class="pp-note">正在演示：{{ session.title || "未命名笔记" }}</p>
          <div class="pp-actions">
            <button v-if="editing.length" type="button" class="pp-ghost" @click="emit('revoke')">全部停止编辑</button>
            <button type="button" class="pp-stop" @click="emit('stop')">停止演示</button>
            <button type="button" class="pp-ghost" @click="emit('close')">收起</button>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.pp-root {
  position: fixed;
  inset: 0;
  z-index: 75;
  display: grid;
  place-items: center;
  padding: 24px;
}

.pp-mask {
  position: absolute;
  inset: 0;
  border: none;
  background: rgba(15, 23, 42, 0.38);
  cursor: pointer;
}

.pp-card {
  position: relative;
  width: min(440px, 100%);
  border-radius: 18px;
  overflow: hidden;
  background: #fff;
  box-shadow: 0 24px 70px rgba(15, 23, 42, 0.22);
  font-family: var(--font);
}

.pp-hero {
  padding: 22px 22px 18px;
  background:
    radial-gradient(420px 160px at 100% 0%, rgba(255, 255, 255, 0.35), transparent 60%),
    linear-gradient(135deg, #1d4ed8, #4338ca);
  color: #fff;
}

.pp-live {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  letter-spacing: 0.08em;
}

.pp-live::before {
  content: "";
  width: 8px;
  height: 8px;
  border-radius: 99px;
  background: #4ade80;
  box-shadow: 0 0 0 5px rgba(74, 222, 128, 0.25);
}

.pp-hero h2 {
  margin: 8px 0 6px;
  font-size: 24px;
}

.pp-hero p {
  margin: 0;
  font-size: 13px;
  line-height: 1.55;
  color: rgba(255, 255, 255, 0.88);
}

.pp-body {
  padding: 18px 22px 20px;
}

.pp-ip {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 12px 14px;
  border-radius: 12px;
  background: #f8fafc;
  border: 1px solid #e5e7eb;
}

.pp-ip span {
  color: #6b7280;
  font-size: 13px;
}

.pp-ip strong {
  font-size: 20px;
  letter-spacing: 0.02em;
}

.pp-warn {
  margin: 10px 0 0;
  color: #9a3412;
  background: #fff7ed;
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 12px;
}

.pp-url-label {
  display: block;
  margin: 14px 0 6px;
  font-size: 12px;
  color: #6b7280;
}

.pp-url {
  display: flex;
  gap: 8px;
  align-items: center;
}

.pp-url code {
  flex: 1;
  min-width: 0;
  padding: 10px 12px;
  border-radius: 10px;
  background: #0f172a;
  color: #e2e8f0;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pp-url button,
.pp-stop,
.pp-ghost {
  border: none;
  border-radius: 10px;
  height: 36px;
  padding: 0 14px;
  cursor: pointer;
}

.pp-url button,
.pp-stop {
  background: #1d4ed8;
  color: #fff;
}

.pp-note {
  margin: 12px 0 0;
  font-size: 13px;
  color: #374151;
}

.pp-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}

.pp-ghost {
  background: #f3f4f6;
  color: #111827;
}

.pp-online-wrap {
  position: relative;
  margin-bottom: 12px;
}

.pp-online {
  height: 32px;
  padding: 0 12px;
  border: 1px solid #bfdbfe;
  border-radius: 999px;
  background: #eff6ff;
  color: #1d4ed8;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}

.pp-tip {
  position: absolute;
  z-index: 2;
  top: 38px;
  left: 0;
  width: min(320px, 100%);
  padding: 10px 12px;
  border-radius: 12px;
  background: #0f172a;
  color: #e2e8f0;
  box-shadow: 0 12px 30px rgba(15, 23, 42, 0.2);
}

.pp-tip p {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
}

.pp-tip p + p {
  margin-top: 6px;
}
.pp-peer span { flex: 1; min-width: 0; }
.pp-peer button {
  border: none;
  background: transparent;
  color: #93c5fd;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  padding: 0;
  flex-shrink: 0;
}

.pp-tip i {
  width: 8px;
  height: 8px;
  border-radius: 99px;
  flex-shrink: 0;
}

.pp-max {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 0 12px;
  font-size: 13px;
  color: #374151;
}

.pp-max input {
  width: 64px;
  height: 32px;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 0 8px;
  font: inherit;
}

.pp-max em {
  font-style: normal;
  color: #6b7280;
  font-size: 12px;
}
</style>
