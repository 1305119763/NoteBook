export type ChangeRow =
  | { kind: "change"; code: boolean; from: string; to: string }
  | { kind: "add"; code: boolean; text: string }
  | { kind: "del"; code: boolean; text: string };

export type NoteDiff = {
  rows: ChangeRow[];
  added: number;
  removed: number;
  mediaNote: string | null;
  formatNote: string | null;
  truncated: boolean;
};

type Line = { code: boolean; text: string };

const BLOCK = "p,h1,h2,h3,h4,h5,h6,li,blockquote,pre";

function mediaLabel(el: Element): string {
  const src = el.getAttribute("src") || "";
  let decoded = src;
  try {
    decoded = decodeURIComponent(src);
  } catch {
    decoded = src;
  }
  const file = decoded.split(/[/\\]/).filter(Boolean).pop()?.replace(/\?.*$/, "") || "未命名";
  const kind =
    el.tagName === "VIDEO" ? "视频" : el.tagName === "AUDIO" ? "音频" : el.tagName === "IMG" ? "图片" : "文件";
  return `${kind} ${file}`;
}

/** 历史里图片、视频只留名称，不引用原文件。 */
export function historyTranscript(html: string): string {
  const doc = new DOMParser().parseFromString(html || "", "text/html");
  doc.querySelectorAll("[data-collab-ip]").forEach((el) => {
    const parent = el.parentNode;
    if (!parent) return;
    while (el.firstChild) parent.insertBefore(el.firstChild, el);
    parent.removeChild(el);
  });
  doc.querySelectorAll("img,video,audio,iframe,object,source").forEach((el) => {
    el.replaceWith(doc.createTextNode(mediaLabel(el)));
  });
  return doc.body.innerHTML;
}

function linesFrom(html: string): Line[] {
  const doc = new DOMParser().parseFromString(historyTranscript(html), "text/html");
  const nodes = [...doc.body.querySelectorAll(BLOCK)];
  const lines: Line[] = [];
  for (const el of nodes) {
    if (el.matches("li,blockquote") && el.querySelector("p,h1,h2,h3,h4,h5,h6,li,pre")) continue;
    const text = (el.textContent || "").replace(/\u00a0/g, " ").trim();
    if (!text) continue;
    const code = el.tagName === "PRE";
    for (const raw of text.split(/\n/)) {
      const line = raw.trim();
      if (line) lines.push({ code, text: line });
    }
  }
  if (lines.length === 0) {
    const fallback = (doc.body.textContent || "").replace(/\u00a0/g, " ").trim();
    if (fallback) lines.push({ code: false, text: fallback });
  }
  return lines;
}

function diffLines(a: Line[], b: Line[]): Array<{ kind: "same" | "add" | "del"; line: Line }> {
  const n = a.length;
  const m = b.length;
  if (n * m > 360000) {
    const out: Array<{ kind: "same" | "add" | "del"; line: Line }> = [];
    for (const line of a.slice(0, 400)) out.push({ kind: "del", line });
    for (const line of b.slice(0, 400)) out.push({ kind: "add", line });
    return out;
  }
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      if (a[i].code === b[j].code && a[i].text === b[j].text) dp[i][j] = dp[i + 1][j + 1] + 1;
      else dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }
  const out: Array<{ kind: "same" | "add" | "del"; line: Line }> = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i].code === b[j].code && a[i].text === b[j].text) {
      out.push({ kind: "same", line: a[i] });
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ kind: "del", line: a[i] });
      i += 1;
    } else {
      out.push({ kind: "add", line: b[j] });
      j += 1;
    }
  }
  while (i < n) out.push({ kind: "del", line: a[i++] });
  while (j < m) out.push({ kind: "add", line: b[j++] });
  return out;
}

function toRows(ops: Array<{ kind: "same" | "add" | "del"; line: Line }>): ChangeRow[] {
  const rows: ChangeRow[] = [];
  let index = 0;
  while (index < ops.length) {
    if (ops[index].kind === "same") {
      index += 1;
      continue;
    }
    const dels: Line[] = [];
    const adds: Line[] = [];
    while (index < ops.length && ops[index].kind !== "same") {
      if (ops[index].kind === "del") dels.push(ops[index].line);
      else adds.push(ops[index].line);
      index += 1;
    }
    const pair = Math.min(dels.length, adds.length);
    for (let k = 0; k < pair; k++) {
      rows.push({
        kind: "change",
        code: dels[k].code || adds[k].code,
        from: dels[k].text,
        to: adds[k].text,
      });
    }
    for (let k = pair; k < dels.length; k++) {
      rows.push({ kind: "del", code: dels[k].code, text: dels[k].text });
    }
    for (let k = pair; k < adds.length; k++) {
      rows.push({ kind: "add", code: adds[k].code, text: adds[k].text });
    }
  }
  return rows;
}

export function diffNoteHtml(previousHtml: string, nextHtml: string): NoteDiff {
  const rowsAll = toRows(diffLines(linesFrom(previousHtml), linesFrom(nextHtml)));
  const truncated = rowsAll.length > 200;
  const rows = truncated ? rowsAll.slice(0, 200) : rowsAll;
  let added = 0;
  let removed = 0;
  for (const row of rows) {
    if (row.kind === "add" || row.kind === "change") added += 1;
    if (row.kind === "del" || row.kind === "change") removed += 1;
  }
  const formatNote =
    rows.length === 0 && previousHtml !== nextHtml
      ? "文字和代码没有变化。颜色、字号等格式若有调整，这里不逐项展开。"
      : null;
  return { rows, added, removed, mediaNote: null, formatNote, truncated };
}
