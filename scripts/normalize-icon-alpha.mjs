/**
 * 从图像四边（整条边框）flood-fill，把「假透明」的浅/中灰棋盘格或白底改为真透明。
 * 只认近似无彩的灰（RGB 接近），避免吃掉主体里的彩色；与边框不连通的内部像素不会变。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function isSeededBackground(r, g, b, a) {
  if (a < 16) return true;
  if (a < 240) return false;
  const mx = Math.max(r, g, b);
  const mn = Math.min(r, g, b);
  const spread = mx - mn;
  const sum = r + g + b;
  if (r > 248 && g > 248 && b > 248) return true;
  if (spread < 12 && sum > 690) return true;
  if (spread < 16 && sum > 560 && sum < 695) return true;
  // 较深的中性灰棋盘格（如 #808080 / #a0a0a0），四角规则原先够不到
  if (spread < 14 && sum >= 270 && sum <= 780) return true;
  return false;
}

function normalizePngFile(inputPath, outputPath) {
  const buf = fs.readFileSync(inputPath);
  const png = PNG.sync.read(buf);
  const { width: w, height: h, data } = png;
  const visited = new Uint8Array(w * h);
  const q = [];

  function idx(x, y) {
    return (y * w + x) * 4;
  }

  function tryPush(x, y) {
    if (x < 0 || y < 0 || x >= w || y >= h) return;
    const i = y * w + x;
    if (visited[i]) return;
    visited[i] = 1;
    const p = idx(x, y);
    const r = data[p];
    const g = data[p + 1];
    const b = data[p + 2];
    const a = data[p + 3];
    if (!isSeededBackground(r, g, b, a)) return;
    data[p + 3] = 0;
    q.push(x, y);
  }

  for (let x = 0; x < w; x++) {
    tryPush(x, 0);
    tryPush(x, h - 1);
  }
  for (let y = 0; y < h; y++) {
    tryPush(0, y);
    tryPush(w - 1, y);
  }

  while (q.length) {
    const y = q.pop();
    const x = q.pop();
    tryPush(x + 1, y);
    tryPush(x - 1, y);
    tryPush(x, y + 1);
    tryPush(x, y - 1);
  }

  fs.writeFileSync(outputPath, PNG.sync.write(png));
}

const root = path.resolve(__dirname, "..");
const input = process.argv[2] || path.join(root, "src/assets/notebook-logo.png");
const out = process.argv[3] || path.join(root, "src/assets/notebook-logo.png");
normalizePngFile(input, out);
console.log("Wrote", out);
