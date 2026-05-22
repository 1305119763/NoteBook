# Windows 构建指南

本文档说明如何为 NoteBook 项目构建 Windows 版本。

## 构建命令

### 在 Windows 系统上直接构建

如果你在 Windows 系统上开发，可以直接运行：

```bash
# 安装依赖
npm install

# 构建 Windows 版本
npm run build:win
```

这会生成：
- `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/notebook_1.1.0_x64_en-US.msi` - Windows 安装包
- `src-tauri/target/x86_64-pc-windows-msvc/release/notebook.exe` - 可执行文件

### 在 macOS/Linux 上构建 Windows 版本

由于 Rust 的跨平台编译限制，在 macOS 或 Linux 上**无法直接**构建 Windows 可执行文件。有以下两种方案：

#### 方案一：使用 GitHub Actions 自动构建（推荐）

1. 为项目创建 GitHub 仓库
2. 推送代码到仓库
3. 创建版本标签：
   ```bash
   git tag v1.1.0
   git push origin v1.1.0
   ```
4. GitHub Actions 会自动构建 Windows 和 macOS 版本，并创建 Release

#### 方案二：使用 Docker 交叉编译

创建 Docker 构建环境：

```bash
# 创建 Dockerfile.windows
FROM rust:1.75

# 安装 Windows 目标
RUN rustup target add x86_64-pc-windows-msvc

# 安装 Node.js
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs

# 设置工作目录
WORKDIR /app

# 复制项目文件
COPY . .

# 安装 npm 依赖
RUN npm ci

# 构建
RUN npm run build && npm run tauri build -- --target x86_64-pc-windows-msvc
```

## 系统要求

### 开发环境要求
- Node.js >= 20.0.0
- Rust 1.75+
- Tauri CLI v2
- Windows 10/11 (用于本地构建)

### 运行时要求
- Windows 10/11 (64位)
- WebView2 运行时 (Tauri 会自动安装)

## 配置说明

### package.json 新增命令
```json
"build:win": "npm run build && tauri build --target x86_64-pc-windows-msvc"
```

### Tauri 配置
项目已配置为支持所有平台：
```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico"  # Windows 图标
  ]
}
```

## 常见问题

### 1. 构建时提示 "cannot find -luserenv"
需要安装 Windows 目标：
```bash
rustup target add x86_64-pc-windows-msvc
```

### 2. WebView2 运行时
Tauri 2 使用 WebView2，Windows 10 需要安装 WebView2 运行时。安装包会自动包含运行时安装器。

### 3. 图标问题
确保 `src-tauri/icons/` 目录包含：
- `icon.ico` - Windows 图标
- `icon.icns` - macOS 图标
- 各种尺寸的 PNG 图标

### 4. 版本号更新
版本号在以下位置需要同步更新：
1. `package.json` 中的 `"version"`
2. `src-tauri/tauri.conf.json` 中的 `"version"`
3. `src-tauri/Cargo.toml` 中的 `version`

## 发布流程

1. 更新版本号到 1.1.0
2. 提交代码到 Git
3. 创建版本标签：`git tag v1.1.0`
4. 推送标签：`git push origin v1.1.0`
5. GitHub Actions 自动构建并发布 Release

## 手动构建检查清单

- [ ] Node.js 版本 >= 20.0.0
- [ ] Rust 工具链已安装
- [ ] Windows 目标已添加：`rustup target add x86_64-pc-windows-msvc`
- [ ] Tauri CLI 已安装：`npm install -g @tauri-apps/cli`
- [ ] 项目依赖已安装：`npm install`
- [ ] 版本号已更新到 1.1.0
- [ ] Windows 图标 (.ico) 已准备
- [ ] 构建命令：`npm run build:win`