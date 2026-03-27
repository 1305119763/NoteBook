/**
 * 从 src/assets/notebook-logo.png 裁正方形并缩放至 1024，写入临时文件后调用 tauri icon（需 macOS + sips）。
 */
import { execSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

if (process.platform !== "darwin") {
  console.error("rebuild-icons: 当前脚本依赖 macOS 的 sips，请在 Mac 上执行。");
  process.exit(1);
}

const logoSrc = path.join(root, "src/assets/notebook-logo.png");
const tmpCrop = path.join(os.tmpdir(), `nb-icon-crop-${Date.now()}.png`);
const tmp1024 = path.join(os.tmpdir(), `nb-icon-1024-${Date.now()}.png`);

if (!fs.existsSync(logoSrc)) {
  console.error("缺少", logoSrc);
  process.exit(1);
}

try {
  const out = execSync(`sips -g pixelWidth -g pixelHeight "${logoSrc}"`, {
    encoding: "utf-8",
  });
  const w = Number(out.match(/pixelWidth:\s*(\d+)/)?.[1] ?? 0);
  const h = Number(out.match(/pixelHeight:\s*(\d+)/)?.[1] ?? 0);
  const s = Math.min(w, h);
  if (!s) {
    console.error("无法解析图片尺寸");
    process.exit(1);
  }

  execSync(`sips -c ${s} ${s} "${logoSrc}" --out "${tmpCrop}"`, { stdio: "inherit" });
  execSync(`sips -z 1024 1024 "${tmpCrop}" --out "${tmp1024}"`, { stdio: "inherit" });

  execSync(`npx tauri icon "${tmp1024}"`, {
    cwd: root,
    stdio: "inherit",
  });

  const pub = path.join(root, "public/logo.png");
  const i128 = path.join(root, "src-tauri/icons/128x128.png");
  fs.copyFileSync(i128, pub);

  const hero = path.join(root, "docs/readme-hero.png");
  fs.copyFileSync(logoSrc, hero);

  console.log("完成：src-tauri/icons/*、public/logo.png、docs/readme-hero.png");
} finally {
  for (const p of [tmpCrop, tmp1024]) {
    try {
      if (fs.existsSync(p)) fs.unlinkSync(p);
    } catch {
      /* ignore */
    }
  }
}
