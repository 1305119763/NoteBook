/** 10 列 × 6 行，常用配色 */
export const COLOR_PALETTE: string[] = [
  "#000000", "#434343", "#666666", "#999999", "#b7b7b7", "#cccccc", "#d9d9d9", "#efefef", "#f3f3f3", "#ffffff",
  "#7f1d1d", "#9a3412", "#854d0e", "#3f6212", "#115e59", "#1e3a8a", "#312e81", "#581c87", "#831843", "#111827",
  "#dc2626", "#ea580c", "#d97706", "#65a30d", "#0d9488", "#2563eb", "#4f46e5", "#9333ea", "#db2777", "#374151",
  "#f87171", "#fb923c", "#fbbf24", "#a3e635", "#2dd4bf", "#60a5fa", "#818cf8", "#c084fc", "#f472b6", "#6b7280",
  "#fecaca", "#fed7aa", "#fde68a", "#d9f99d", "#99f6e4", "#bfdbfe", "#c7d2fe", "#e9d5ff", "#fbcfe8", "#d1d5db",
  "#450a0a", "#7c2d12", "#713f12", "#365314", "#134e4a", "#1e40af", "#3730a3", "#6b21a8", "#9d174d", "#030712",
];

export const FONT_SIZES = [12, 14, 15, 16, 18, 20, 24, 28, 32, 36, 48, 72] as const;

const RECENT_KEY = "notebook.recentColors";

export function cssColorToHex(input: string): string | null {
  const raw = input.trim();
  const short = raw.match(/^#([0-9a-fA-F]{3})$/);
  if (short) {
    const [r, g, b] = short[1].split("");
    return `#${r}${r}${g}${g}${b}${b}`.toLowerCase();
  }
  const full = raw.match(/^#([0-9a-fA-F]{6})$/);
  if (full) return `#${full[1].toLowerCase()}`;
  const rgb = raw.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i);
  if (!rgb) return null;
  const hex = (n: string) => Math.max(0, Math.min(255, Number(n))).toString(16).padStart(2, "0");
  return `#${hex(rgb[1])}${hex(rgb[2])}${hex(rgb[3])}`;
}

export function readRecentColors(): string[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]") as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.map((item) => (typeof item === "string" ? cssColorToHex(item) : null)).filter((item): item is string => !!item).slice(0, 10);
  } catch {
    return [];
  }
}

export function pushRecentColor(color: string): string[] {
  const hex = cssColorToHex(color);
  if (!hex) return readRecentColors();
  const next = [hex, ...readRecentColors().filter((item) => item !== hex)].slice(0, 10);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    /* ignore */
  }
  return next;
}

export function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  const c = v * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  let r = 0;
  let g = 0;
  let b = 0;
  if (hp < 1) [r, g, b] = [c, x, 0];
  else if (hp < 2) [r, g, b] = [x, c, 0];
  else if (hp < 3) [r, g, b] = [0, c, x];
  else if (hp < 4) [r, g, b] = [0, x, c];
  else if (hp < 5) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  const m = v - c;
  return [Math.round((r + m) * 255), Math.round((g + m) * 255), Math.round((b + m) * 255)];
}

export function rgbToHex(r: number, g: number, b: number): string {
  const hex = (n: number) => Math.max(0, Math.min(255, n)).toString(16).padStart(2, "0");
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

export function hexToHsv(hex: string): { h: number; s: number; v: number } | null {
  const full = cssColorToHex(hex);
  if (!full) return null;
  const r = parseInt(full.slice(1, 3), 16) / 255;
  const g = parseInt(full.slice(3, 5), 16) / 255;
  const b = parseInt(full.slice(5, 7), 16) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d !== 0) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}
