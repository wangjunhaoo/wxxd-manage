#!/bin/bash
# 一键构建并安装到 /Applications：解决「tauri build 后忘记覆盖旧 app、双击跑旧版」的坑。
# 用法：bash scripts/build_install.sh [--skip-build]
#   --skip-build  跳过构建，只把已有产物覆盖安装（构建已完成、只想重装时用）
set -euo pipefail

cd "$(dirname "$0")/.."

APP_NAME="微信小店铺货中台"
BUNDLE_APP="src-tauri/target/release/bundle/macos/${APP_NAME}.app"
INSTALLED_APP="/Applications/${APP_NAME}.app"

if [[ "${1:-}" != "--skip-build" ]]; then
    echo "==> 构建 tauri app（含前端 build:tauri）..."
    npm run tauri build
fi

if [[ ! -d "$BUNDLE_APP" ]]; then
    echo "错误：构建产物不存在：$BUNDLE_APP" >&2
    exit 1
fi

# 退出正在运行的旧 app（不在运行则忽略）
echo "==> 退出正在运行的旧版本..."
osascript -e "tell application \"${APP_NAME}\" to quit" 2>/dev/null || true
sleep 1

echo "==> 覆盖安装到 /Applications ..."
rm -rf "$INSTALLED_APP"
cp -R "$BUNDLE_APP" "$INSTALLED_APP"

# 主二进制 hash 对比，确认装的就是刚构建的版本（防止缓存/复制失败装了旧版）
BUILT_HASH=$(shasum -a 256 "$BUNDLE_APP/Contents/MacOS/"* | awk '{print $1}' | head -1)
INSTALLED_HASH=$(shasum -a 256 "$INSTALLED_APP/Contents/MacOS/"* | awk '{print $1}' | head -1)
if [[ "$BUILT_HASH" != "$INSTALLED_HASH" ]]; then
    echo "错误：安装后 hash 不一致，覆盖可能失败！" >&2
    echo "  构建产物: $BUILT_HASH" >&2
    echo "  已安装:   $INSTALLED_HASH" >&2
    exit 1
fi
echo "==> 安装成功（hash 一致: ${BUILT_HASH:0:12}...）"

echo "==> 启动新版本..."
open "$INSTALLED_APP"

echo ""
echo "注意：新签名首次访问钥匙串会弹 SecurityAgent 授权窗，请人工点「始终允许」"
echo "（不点的话 AI 调用会同步阻塞 driver，铺货静默卡死）"
