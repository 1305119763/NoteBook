import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { platform } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(new URL(".", import.meta.url)), "..");
const isWin = platform() === "win32";
const target = "x86_64-pc-windows-msvc";

function run(cmd, args, opts = {}) {
  const result = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: isWin,
    env: { ...process.env, CI: undefined },
    ...opts,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function hasCargoXwin() {
  const result = spawnSync("cargo-xwin", ["--version"], {
    cwd: root,
    encoding: "utf8",
    shell: isWin,
  });
  return result.status === 0;
}

run("npm", ["run", "build"]);

const tauriArgs = ["tauri", "build", "--target", target];

if (isWin) {
  run("npx", tauriArgs);
  console.log("\nWindows 安装包已生成，请查看 src-tauri/target/release/bundle/");
} else {
  if (!hasCargoXwin()) {
    console.error("\n在 macOS / Linux 上交叉编译 Windows 版本需要先安装 cargo-xwin：\n");
    console.error("  rustup target add x86_64-pc-windows-msvc");
    console.error("  cargo install cargo-xwin --locked\n");
    process.exit(1);
  }

  // NSIS / MSI 安装包只能在 Windows 主机上可靠生成
  tauriArgs.push("--runner", "cargo-xwin", "--no-bundle");
  run("npx", tauriArgs);

  const exePath = join(
    root,
    "src-tauri/target",
    target,
    "release/notebook.exe",
  );
  console.log("\nWindows 可执行文件已生成（未打包安装器）：");
  console.log(`  ${exePath}`);
  if (existsSync(exePath)) {
    console.log("\n可直接分发该 .exe，或在 Windows 上运行 npm run build:win:installer 生成安装包。");
  }
}
