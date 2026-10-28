<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  COLOR_PALETTE,
  cssColorToHex,
  hexToHsv,
  hsvToRgb,
  pushRecentColor,
  readRecentColors,
  rgbToHex,
} from "../utils/palette";

const props = defineProps<{
  kind: "text" | "back";
  disabled: boolean;
  current: string | null;
}>();

const emit = defineEmits<{
  pick: [color: string | null];
}>();

const open = ref(false);
const wheelOpen = ref(false);
const rootRef = ref<HTMLElement | null>(null);
const popRef = ref<HTMLElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const popStyle = ref<Record<string, string>>({});
const hexInput = ref("");
const recent = ref<string[]>(readRecentColors());
const hue = ref(210);
const sat = ref(1);
const val = ref(1);

const title = computed(() => (props.kind === "text" ? "文字颜色" : "选中背景"));
const clearLabel = computed(() => (props.kind === "text" ? "默认颜色" : "无背景颜色"));
const barColor = computed(() => props.current || (props.kind === "text" ? "#111827" : "#fde68a"));
const workingHex = computed(() => {
  const [r, g, b] = hsvToRgb(hue.value, sat.value, val.value);
  return rgbToHex(r, g, b);
});

let dismiss: ((e: MouseEvent) => void) | null = null;

function place() {
  const el = rootRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const width = 292;
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
  const openUp = rect.bottom + 380 > window.innerHeight && rect.top > 220;
  popStyle.value = openUp
    ? { left: `${left}px`, bottom: `${window.innerHeight - rect.top + 8}px` }
    : { left: `${left}px`, top: `${rect.bottom + 8}px` };
}

function syncFromCurrent() {
  const hex = props.current ? cssColorToHex(props.current) : null;
  const hsv = hex ? hexToHsv(hex) : null;
  if (hsv) {
    hue.value = hsv.h;
    sat.value = hsv.s;
    val.value = hsv.v;
    hexInput.value = hex || "";
  } else {
    hue.value = props.kind === "text" ? 220 : 45;
    sat.value = props.kind === "text" ? 0.8 : 0.7;
    val.value = 1;
    hexInput.value = workingHex.value;
  }
}

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
  if (!open.value) {
    wheelOpen.value = false;
    return;
  }
  recent.value = readRecentColors();
  syncFromCurrent();
  void nextTick(() => place());
}

function close() {
  open.value = false;
  wheelOpen.value = false;
}

function commit(color: string | null) {
  if (color) {
    const hex = cssColorToHex(color);
    if (!hex) return;
    recent.value = pushRecentColor(hex);
    hexInput.value = hex;
    emit("pick", hex);
    return;
  }
  emit("pick", null);
}

function onHexCommit() {
  const hex = cssColorToHex(hexInput.value.startsWith("#") ? hexInput.value : `#${hexInput.value}`);
  if (!hex) {
    hexInput.value = workingHex.value;
    return;
  }
  const hsv = hexToHsv(hex);
  if (hsv) {
    hue.value = hsv.h;
    sat.value = hsv.s;
    val.value = hsv.v;
  }
  commit(hex);
}

function drawWheel() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const size = 168;
  const dpr = window.devicePixelRatio || 1;
  canvas.width = size * dpr;
  canvas.height = size * dpr;
  canvas.style.width = `${size}px`;
  canvas.style.height = `${size}px`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  const cx = size / 2;
  const radius = size / 2 - 2;
  const image = ctx.createImageData(size, size);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x - cx;
      const dy = y - cx;
      const dist = Math.hypot(dx, dy) / radius;
      const i = (y * size + x) * 4;
      if (dist > 1) {
        image.data[i + 3] = 0;
        continue;
      }
      let angle = (Math.atan2(dy, dx) * 180) / Math.PI;
      if (angle < 0) angle += 360;
      const [r, g, b] = hsvToRgb(angle, dist, 1);
      image.data[i] = r;
      image.data[i + 1] = g;
      image.data[i + 2] = b;
      image.data[i + 3] = 255;
    }
  }
  ctx.putImageData(image, 0, 0);
}

function pickWheel(event: PointerEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const dx = event.clientX - rect.left - rect.width / 2;
  const dy = event.clientY - rect.top - rect.height / 2;
  const radius = rect.width / 2 - 2;
  sat.value = Math.max(0, Math.min(1, Math.hypot(dx, dy) / radius));
  let angle = (Math.atan2(dy, dx) * 180) / Math.PI;
  if (angle < 0) angle += 360;
  hue.value = angle;
  hexInput.value = workingHex.value;
}

function onWheelDown(event: PointerEvent) {
  pickWheel(event);
  const move = (ev: PointerEvent) => pickWheel(ev);
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    commit(workingHex.value);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

const wheelCursor = computed(() => {
  const size = 168;
  const radius = size / 2 - 2;
  const rad = (hue.value * Math.PI) / 180;
  const x = size / 2 + Math.cos(rad) * sat.value * radius;
  const y = size / 2 + Math.sin(rad) * sat.value * radius;
  return { left: `${x}px`, top: `${y}px` };
});

function onPopMouseDown(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (target.closest("input")) return;
  event.preventDefault();
}

watch(val, () => {
  hexInput.value = workingHex.value;
});

watch(wheelOpen, async (shown) => {
  if (!shown) return;
  await nextTick();
  drawWheel();
});

watch(open, (shown) => {
  if (dismiss) {
    document.removeEventListener("mousedown", dismiss, true);
    dismiss = null;
  }
  if (!shown) return;
  dismiss = (event: MouseEvent) => {
    const target = event.target as Node;
    if (rootRef.value?.contains(target) || popRef.value?.contains(target)) return;
    close();
  };
  document.addEventListener("mousedown", dismiss, true);
});

onBeforeUnmount(() => {
  if (dismiss) document.removeEventListener("mousedown", dismiss, true);
});
</script>

<template>
  <div ref="rootRef" class="cm-wrap">
    <button
      type="button"
      class="cm-btn"
      :class="{ on: open || !!current }"
      :disabled="disabled"
      :title="title"
      @click="toggle"
    >
      <span class="cm-a">{{ kind === "text" ? "A" : "底" }}</span>
      <span class="cm-bar" :style="{ background: barColor }" />
    </button>
    <Teleport to="body">
      <div
        v-if="open"
        ref="popRef"
        class="cm-pop"
        :style="popStyle"
        role="dialog"
        :aria-label="title"
        @mousedown="onPopMouseDown"
      >
        <button type="button" class="cm-clear" @click="commit(null)">{{ clearLabel }}</button>
        <p class="cm-label">常用颜色</p>
        <div class="cm-grid">
          <button
            v-for="color in COLOR_PALETTE"
            :key="color"
            type="button"
            class="cm-swatch"
            :class="{ 'cm-swatch--on': current === color }"
            :style="{ background: color }"
            :title="color"
            @click="commit(color)"
          />
        </div>
        <p class="cm-label">十六进制</p>
        <div class="cm-hex-row">
          <input
            v-model="hexInput"
            class="cm-hex"
            spellcheck="false"
            maxlength="7"
            placeholder="#3366ff"
            @keydown.enter.prevent="onHexCommit"
            @blur="onHexCommit"
          />
          <span class="cm-hex-preview" :style="{ background: workingHex }" />
        </div>
        <button type="button" class="cm-wheel-toggle" @click="wheelOpen = !wheelOpen">
          {{ wheelOpen ? "收起选色盘" : "选色盘" }}
        </button>
        <div v-if="wheelOpen" class="cm-wheel-box">
          <div class="cm-wheel-stage">
            <canvas ref="canvasRef" class="cm-wheel" @pointerdown.prevent="onWheelDown" />
            <span class="cm-wheel-cursor" :style="wheelCursor" />
          </div>
          <input
            v-model.number="val"
            class="cm-val"
            type="range"
            min="0"
            max="1"
            step="0.01"
            @change="commit(workingHex)"
          />
        </div>
        <p class="cm-label">最近使用</p>
        <div class="cm-recent">
          <button
            v-for="index in 10"
            :key="index"
            type="button"
            class="cm-swatch cm-swatch--recent"
            :style="recent[index - 1] ? { background: recent[index - 1] } : undefined"
            :title="recent[index - 1] || '空'"
            :disabled="!recent[index - 1]"
            @click="recent[index - 1] && commit(recent[index - 1])"
          />
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.cm-wrap {
  display: inline-flex;
}

.cm-btn {
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  min-width: 28px;
  height: 28px;
  padding: 2px 6px 3px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: #374151;
  cursor: pointer;
}

.cm-btn:hover:not(:disabled) {
  background: #f3f4f6;
}

.cm-btn.on {
  background: #e8f1ff;
  border-color: #bfdbfe;
}

.cm-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.cm-a {
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
}

.cm-bar {
  width: 14px;
  height: 3px;
  border-radius: 2px;
}

.cm-pop {
  position: fixed;
  z-index: 80;
  width: 292px;
  padding: 12px;
  border-radius: 12px;
  background: var(--surface, #fff);
  color: var(--surface-text, #111827);
  border: 1px solid var(--line, #e5e7eb);
  box-shadow: 0 16px 40px rgba(15, 23, 42, 0.16);
}

.cm-clear {
  width: 100%;
  height: 30px;
  border: 1px solid var(--line, #e5e7eb);
  border-radius: 8px;
  background: var(--chip, #f8fafc);
  color: var(--chip-text, #374151);
  cursor: pointer;
  font-size: 12px;
}

.cm-clear:hover {
  border-color: #cbd5e1;
}

.cm-label {
  margin: 10px 0 6px;
  font-size: 11px;
  color: #6b7280;
}

.cm-grid,
.cm-recent {
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 5px;
}

.cm-swatch {
  width: 18px;
  height: 18px;
  padding: 0;
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 3px;
  cursor: pointer;
}

.cm-swatch--on {
  outline: 2px solid #2563eb;
  outline-offset: 1px;
}

.cm-swatch--recent {
  background: #f3f4f6;
}

.cm-swatch:disabled {
  cursor: default;
}

.cm-hex-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cm-hex {
  flex: 1;
  height: 30px;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  padding: 0 8px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
}

.cm-hex:focus {
  outline: none;
  border-color: #93c5fd;
}

.cm-hex-preview {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
}

.cm-wheel-toggle {
  width: 100%;
  margin-top: 8px;
  height: 30px;
  border: none;
  border-radius: 8px;
  background: #111827;
  color: #fff;
  cursor: pointer;
  font-size: 12px;
}

.cm-wheel-box {
  margin-top: 10px;
  padding: 10px;
  border-radius: 10px;
  background: #1f2430;
}

.cm-wheel-stage {
  position: relative;
  width: 168px;
  height: 168px;
  margin: 0 auto;
}

.cm-wheel {
  width: 168px;
  height: 168px;
  border-radius: 50%;
  cursor: crosshair;
  display: block;
}

.cm-wheel-cursor {
  position: absolute;
  width: 12px;
  height: 12px;
  margin: -6px 0 0 -6px;
  border: 2px solid #fff;
  border-radius: 50%;
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.45);
  pointer-events: none;
}

.cm-val {
  width: 100%;
  margin-top: 10px;
}
</style>
