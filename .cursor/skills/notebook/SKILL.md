---
name: notebook
description: >-
  NoteBook 桌面笔记（Tauri 2 + Vue 3）的界面、字体、主题、分享编辑、导入导出和版本记录规范。
  在修改 NoteBook 项目、调整样式、暗色主题、字体、设置、局域网演示、.tbook、changelog 或左上角布局时使用。
---

# NoteBook

只约束本仓库。不要套用其他项目的 rem、Vant 或教师端规范。界面文案用中文。回复用户用中文，短句，先给结论。

## 字体

全站界面和笔记正文只用左侧栏这一套，定义在 `html` 的 `--font`：

`ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif`

- 新界面写 `font-family: var(--font)` 或 `inherit`，不要再写一套字体栈。
- 代码块、行内代码、十六进制输入保持等宽字体。
- 局域网协作里用来区分不同 IP 的字体保留，不要改回 `--font`。

## 颜色

颜色只走 `html` 上的变量，浅色和 `html[data-theme="dark"]` 成对给出。组件里不要写死 `#fff`、`#111` 当背景或正文。

常用变量：`--bg-side` `--bg-middle` `--bg` `--paper` `--toolbar` `--card` `--card-hover` `--card-active` `--field` `--chip` `--surface` `--text` `--muted` `--line` `--accent`。

暗色要让侧栏、列表、编辑区、弹层是同一套深色，字用 `--text` / `--muted`。用户在正文里手动设过的字色和底色不要清掉。

## 布局

- 左上：`public/screen.png` + NoteBook +「本地笔记，安全不联网不泄露」。Logo 不要裁成圆。
- 左下：`NoteBook v` + `APP_DISPLAY_VERSION`。
- 废纸篓下有「软件设置」，和系统菜单打开同一个设置。
- 换 logo 时同步 `index.html` 图标，并用 `npm run icons:rebuild` 从 `public/screen.png` 生成 `src-tauri/icons`。不要再引用已删除的 `notebook-logo.png`、`public/logo.png`。

## 设置

设置写入和 `notebook.db` 同一目录的 `settings.json`，不写安装包目录。

- Mac：`~/Library/Application Support/com.tal.notebook/settings.json`
- Windows：`C:\Users\<用户名>\AppData\Local\com.tal.notebook\settings.json`

包含：浅色 / 深色 / 跟随系统，皮肤，本机昵称，新建笔记和文件夹是否用默认名。默认仍弹出命名框。关于：作者 T+，747896298@qq.com，https://github.com/1305119763/NoteBook。

Mac 菜单在屏幕左上角，用中文。Windows 同一套菜单，并放在托盘图标右键里。

## 数据与分享

- 导出 `.tbook` 不能丢正文、收藏、置顶、历史和媒体。导入先校验，失败不能清空现有笔记。
- 局域网演示只分享当前一篇，不过外网。观看端约 1.2 秒拉一次，连续失败 3 次才显示中断。
- 被分享人可停止编辑；笔记本人可全部停止或停止某一个 IP。被停的一侧必须看到通知。
- 改了 `src-tauri` 后要提醒重新执行 `npm run dev:tauri`。

## 版本记录

当前版本是 1.2.0。没有用户明确要求时，不要改 `package.json`、`Cargo.toml`、`tauri.conf.json`、`appMeta.ts` 里的版本号。

用户要 changelog 时，更新根目录 `CHANGELOG.md`，写进当前版本，不要另开版本。按版本写，只保留有内容的小节，条目从 1 编号：

```markdown
## x.y.z（日期）

### 新增
1. …

### 修改
1. …

### 删除
1. …

### 优化
1. …
```

只有用户明确要求发新版本时，才同时改 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src/constants/appMeta.ts`。不要改媒体清单里的格式版本，除非导入导出格式变了。
