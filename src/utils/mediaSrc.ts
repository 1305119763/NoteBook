import { convertFileSrc, invoke } from "@tauri-apps/api/core";

const absCache = new Map<string, string>();

const SRC_RE = /\bsrc\s*=\s*(?:"([^"]+)"|'([^']+)')/gi;
const KEY_RE =
  /(?:notebook-media:\/\/|[/\\]media[/\\])([A-Za-z0-9][A-Za-z0-9._-]{0,180}\.(?:jpg|jpeg|png|gif|webp|svg|mp4|webm|mov))/i;

export function keyFromSrc(src: string): string | null {
  let decoded = src;
  try {
    decoded = decodeURIComponent(src);
  } catch {
    decoded = src;
  }
  const matched = decoded.match(KEY_RE);
  return matched?.[1] ?? null;
}

export function extractMediaKeys(html: string): string[] {
  const keys = new Set<string>();
  for (const match of html.matchAll(SRC_RE)) {
    const src = match[1] || match[2] || "";
    const key = keyFromSrc(src);
    if (key) keys.add(key);
  }
  return [...keys];
}

export function canonicalMediaSrc(src: string): string {
  const key = keyFromSrc(src);
  return key ? `notebook-media://${key}` : src;
}

export function rememberMedia(key: string, absolutePath: string) {
  if (key && absolutePath) absCache.set(key, absolutePath);
}

export function displaySrc(src: string): string {
  if (!src) return src;
  const key = keyFromSrc(src);
  if (!key) return src;
  const abs = absCache.get(key);
  return abs ? convertFileSrc(abs) : src;
}

export async function primeMediaInHtml(html: string) {
  const missing = extractMediaKeys(html).filter((key) => !absCache.has(key));
  if (missing.length === 0) return;
  try {
    const rows = await invoke<Array<{ storageKey: string; absolutePath: string }>>(
      "resolve_media_keys",
      { keys: missing },
    );
    for (const row of rows) absCache.set(row.storageKey, row.absolutePath);
  } catch {
    /* 浏览器里没有 Tauri 命令时保持原样 */
  }
}

export function htmlWithDisplaySrc(html: string): string {
  return html.replace(SRC_RE, (_all, doubleQ: string | undefined, singleQ: string | undefined) => {
    const src = doubleQ || singleQ || "";
    const quote = doubleQ != null ? '"' : "'";
    return `src=${quote}${displaySrc(src)}${quote}`;
  });
}
