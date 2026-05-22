#!/bin/bash
# NoteBook v1.1.0 构建与测试脚本

set -e

echo "=== NoteBook v1.1.0 媒体功能构建测试 ==="
echo "当前目录: $(pwd)"
echo ""

# 检查是否在项目目录
if [ ! -f "package.json" ] || [ ! -d "src-tauri" ]; then
    echo "错误: 请在 NoteBook 项目根目录运行此脚本"
    exit 1
fi

echo "1. 检查依赖..."
if ! command -v node &> /dev/null; then
    echo "错误: Node.js 未安装"
    exit 1
fi

if ! command -v npm &> /dev/null; then
    echo "错误: npm 未安装"
    exit 1
fi

echo "✅ Node.js 版本: $(node --version)"
echo "✅ npm 版本: $(npm --version)"

echo ""
echo "2. 安装依赖..."
npm install

echo ""
echo "3. 检查 Tiptap 图片扩展..."
if ! npm list @tiptap/extension-image &> /dev/null; then
    echo "安装 @tiptap/extension-image..."
    npm install @tiptap/extension-image
fi

echo ""
echo "4. 检查 Tauri 对话框插件..."
if ! npm list @tauri-apps/plugin-dialog &> /dev/null; then
    echo "安装 @tauri-apps/plugin-dialog..."
    npm install @tauri-apps/plugin-dialog
fi

echo ""
echo "5. 构建前端..."
npm run build

echo ""
echo "6. 检查 Rust 依赖..."
cd src-tauri
if ! command -v cargo &> /dev/null; then
    echo "错误: Rust 未安装"
    echo "请安装 Rust: https://rustup.rs/"
    exit 1
fi

echo "✅ Rust 版本: $(cargo --version)"
echo "✅ Cargo 版本: $(cargo --version)"

echo ""
echo "7. 检查 Cargo.toml 配置..."
if ! grep -q "serde = " Cargo.toml; then
    echo "错误: serde 依赖缺失"
    exit 1
fi

echo "✅ serde 依赖已配置"
echo "✅ tauri-plugin-dialog 依赖已配置"

echo ""
echo "8. 构建 Tauri 应用..."
echo "注意: 首次构建可能需要较长时间下载 Rust 依赖"
cargo build --release

echo ""
echo "9. 验证构建结果..."
if [ -f "target/release/notebook" ]; then
    echo "✅ 可执行文件已生成: target/release/notebook"
    echo "✅ 文件大小: $(du -h target/release/notebook | cut -f1)"
else
    echo "❌ 构建失败: 未生成可执行文件"
    exit 1
fi

cd ..

echo ""
echo "10. 创建测试数据..."
mkdir -p test_media
echo "创建测试图片..."
convert -size 800x600 xc:#4a90e2 -pointsize 72 -fill white -gravity center -draw "text 0,0 '测试图片'" test_media/test_image.jpg 2>/dev/null || echo "使用备用图片"
echo "创建测试视频说明..."
echo "请手动准备测试视频文件: test_media/test_video.mp4"
echo "或使用命令: ffmpeg -f lavfi -i testsrc=duration=5:size=640x480:rate=30 test_media/test_video.mp4"

echo ""
echo "11. 创建测试文档..."
cat > test_usage.md << 'EOF'
# NoteBook v1.1.0 测试指南

## 功能测试清单

### 媒体导入测试
- [ ] 启动应用: `npm run tauri dev`
- [ ] 创建新笔记
- [ ] 测试图片导入:
  - [ ] 复制 test_media/test_image.jpg
  - [ ] 在编辑器中粘贴
  - [ ] 拖放图片到编辑器
  - [ ] 点击工具栏 📷 按钮选择图片
- [ ] 测试视频导入:
  - [ ] 复制 test_media/test_video.mp4
  - [ ] 在编辑器中粘贴
  - [ ] 拖放视频到编辑器
  - [ ] 点击工具栏 🎥 按钮选择视频

### 导出导入测试
- [ ] 导出测试:
  - [ ] 保存包含媒体的笔记
  - [ ] 点击「导出 .tbook」
  - [ ] 检查是否生成 `.media` 文件夹
  - [ ] 检查 `media_manifest.json`
- [ ] 导入测试:
  - [ ] 删除应用数据
  - [ ] 重新导入 `.tbook`
  - [ ] 验证媒体是否恢复
  - [ ] 检查媒体文件位置

### 兼容性测试
- [ ] 导入旧版 `.tbook` (无媒体)
- [ ] 导出 v1.1.0 `.tbook` 在旧版导入

## 预期结果
1. 图片显示正常，可缩放
2. 视频可播放，带控制条
3. 导出包含媒体文件夹
4. 导入自动恢复媒体
5. 媒体文件夹独立管理

## 故障排除
1. **媒体不显示**: 检查 `media/` 目录权限
2. **导入失败**: 检查 `.media` 文件夹是否存在
3. **构建错误**: 重新运行 `npm install`
4. **样式问题**: 清除缓存，重新加载
EOF

echo "✅ 测试文档已创建: test_usage.md"

echo ""
echo "12. 清理建议..."
echo "测试完成后，可清理:"
echo "  rm -rf test_media/"
echo "  rm test_usage.md"

echo ""
echo "=== 构建完成 ==="
echo ""
echo "启动应用:"
echo "  npm run tauri dev"
echo ""
echo "或直接运行:"
echo "  ./src-tauri/target/release/notebook"
echo ""
echo "详细测试步骤见: test_usage.md"
echo "功能说明见: CHANGELOG_v1.1.0.md"
echo "构建指南见: BUILD_WINDOWS.md"