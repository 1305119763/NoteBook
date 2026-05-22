#!/bin/bash
# Docker 交叉编译脚本 - 在 macOS/Linux 上构建 Windows 版本
# 需要先安装 Docker: https://docs.docker.com/get-docker/

set -e

echo "=== NoteBook Windows 交叉编译 ==="
echo "当前目录: $(pwd)"
echo ""

# 检查 Docker 是否安装
if ! command -v docker &> /dev/null; then
    echo "错误: Docker 未安装"
    echo "请先安装 Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

# 检查是否在项目根目录
if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
    echo "错误: 请在 NoteBook 项目根目录运行此脚本"
    exit 1
fi

# 创建临时构建目录
BUILD_DIR="$(pwd)/.docker-build"
mkdir -p "$BUILD_DIR"

echo "1. 复制项目文件到临时目录..."
rsync -av --exclude='.git' --exclude='node_modules' --exclude='src-tauri/target' --exclude='dist' ./ "$BUILD_DIR/"

echo "2. 构建 Docker 镜像..."
docker build -t notebook-windows-builder -f - "$BUILD_DIR" << 'EOF'
FROM rust:1.75-slim

# 安装必要的工具
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 安装 Node.js 20
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs

# 添加 Windows 目标
RUN rustup target add x86_64-pc-windows-msvc

# 安装 Tauri CLI
RUN npm install -g @tauri-apps/cli

WORKDIR /app

# 复制项目文件
COPY . .

# 安装依赖
RUN npm ci

# 构建
RUN npm run build && npm run tauri build -- --target x86_64-pc-windows-msvc

# 设置输出目录
VOLUME /output
EOF

echo "3. 运行构建容器..."
docker run --rm \
    -v "$(pwd)/output:/output" \
    notebook-windows-builder \
    bash -c "cp -r src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/*.msi /output/ && cp -r src-tauri/target/x86_64-pc-windows-msvc/release/notebook.exe /output/"

echo ""
echo "4. 构建完成！"
echo "Windows 安装包已生成到:"
echo "  $(pwd)/output/"
ls -la "$(pwd)/output/" 2>/dev/null || echo "   (目录为空，请检查构建日志)"

echo ""
echo "5. 清理临时文件..."
rm -rf "$BUILD_DIR"

echo ""
echo "=== 构建总结 ==="
echo "✅ Windows 构建脚本已执行"
echo ""
echo "后续步骤："
echo "1. 检查 output/ 目录下的 .msi 和 .exe 文件"
echo "2. 在 Windows 系统上测试安装包"
echo "3. 如需发布，建议使用 GitHub Actions 自动构建"
echo ""
echo "GitHub Actions 使用方法："
echo "1. 推送代码到 GitHub 仓库"
echo "2. 创建版本标签: git tag v1.1.0"
echo "3. 推送标签: git push origin v1.1.0"
echo "4. GitHub 会自动构建并发布 Release"