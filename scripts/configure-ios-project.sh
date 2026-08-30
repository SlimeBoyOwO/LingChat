#!/usr/bin/env bash
# ============================================================================
# Configure LingChat's iOS Xcode project (macOS only).
#
# Responsibilities:
#   1. If src-tauri/gen/apple/ does not exist, run `tauri ios init` to
#      generate the Xcode project (XcodeGen under the hood; keeps project.yml
#      and <app>.xcodeproj).
#   2. Normalize TARGETED_DEVICE_FAMILY = "1,2" (iPhone + iPad).
#      XcodeGen's default is already '1,2'; we force it here to guarantee
#      iPhone & iPad compatibility.
#
# NOTE: tauri-cli does not ship the `ios` subcommand on Windows/Linux, so
# this script only runs on macOS.
# Requirements: Xcode (+ Command Line Tools), xcodegen (`brew install xcodegen`),
# Rust target aarch64-apple-ios (`rustup target add aarch64-apple-ios`).
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GEN_APPLE="src-tauri/gen/apple"

# --- 1. Initialize the Xcode project ----------------------------------------

if [ ! -d "$GEN_APPLE" ]; then
  echo "[configure-ios] gen/apple missing, running: pnpm tauri ios init --ci"
  if ! command -v xcodegen >/dev/null 2>&1; then
    echo "[configure-ios] ERROR: xcodegen not found. Install it with: brew install xcodegen" >&2
    exit 1
  fi
  pnpm tauri ios init --ci
  echo "[configure-ios] Xcode project generated"
else
  echo "[configure-ios] $GEN_APPLE already exists, skipping init"
fi

PBXPROJ="$(ls "$GEN_APPLE"/*.xcodeproj/project.pbxproj 2>/dev/null | head -1 || true)"
if [ -z "$PBXPROJ" ] || [ ! -f "$PBXPROJ" ]; then
  echo "[configure-ios] ERROR: project.pbxproj not found under $GEN_APPLE" >&2
  exit 1
fi

# --- 2. Normalize TARGETED_DEVICE_FAMILY = "1,2" (iPhone + iPad) -------------

if grep -q "TARGETED_DEVICE_FAMILY" "$PBXPROJ"; then
  # BSD sed (built into macOS): unify every device-family setting to iPhone + iPad
  sed -i '' -E 's/TARGETED_DEVICE_FAMILY = [^;]+;/TARGETED_DEVICE_FAMILY = "1,2";/g' "$PBXPROJ"
  echo "[configure-ios] TARGETED_DEVICE_FAMILY normalized to \"1,2\" (iPhone + iPad)"
else
  echo "[configure-ios] WARNING: TARGETED_DEVICE_FAMILY not found in pbxproj " \
    "(XcodeGen should generate '1,2' by default). Verify in Xcode Build Settings." >&2
fi

COUNT="$(grep -c 'TARGETED_DEVICE_FAMILY = "1,2"' "$PBXPROJ" || true)"
echo "[configure-ios] TARGETED_DEVICE_FAMILY=\"1,2\" occurrences: $COUNT"

# --- 3. Verify the iOS-specific Info.plist merge hook ------------------------

if [ -f "src-tauri/Info.ios.plist" ]; then
  echo "[configure-ios] src-tauri/Info.ios.plist present (Files-app visibility + device family); it is merged into Info.plist on every tauri ios build"
else
  echo "[configure-ios] WARNING: src-tauri/Info.ios.plist missing (UIFileSharingEnabled etc. will be absent)" >&2
fi

# --- 4. Bypass pnpm in the "Build Rust Code" Xcode phase ---------------------
# tauri ios init 在 pnpm 环境下会把 tauri-binary 渲染为 `pnpm`，于是 Xcode 脚本
# 阶段执行 `pnpm tauri ... xcode-script ...`。pnpm 11 的 verify-deps-before-run
# 会先跑一次自动 `pnpm install`；模块目录需要清空重装时，在无 TTY 环境下直接
# 中止（ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY -> xcodebuild exit 65）。
# CI=true 与 pnpm_config_* 环境变量都无法阻止（Xcode 脚本阶段不继承/或该检查
# 无视它们）。这里把 shellScript 开头的 `pnpm tauri` 替换为直接调用本仓库的
# tauri.js shim（node 直跑，等价于 pnpm tauri 但完全绕开包管理器）。
# tauri ios build 的 synchronize_project_config 不会重写 shellScript，patch 持久生效。

if grep -q 'shellScript = "pnpm tauri ' "$PBXPROJ"; then
  # 用 | 作 sed 分隔符避免转义 /；\\" 在 pbxproj 字符串内输出 \"（嵌套引号）
  # $PROJECT_DIR = src-tauri/gen/apple，需要 ../../.. 三级回到仓库根
  sed -i '' 's|shellScript = "pnpm tauri |shellScript = "node \\"$PROJECT_DIR/../../../node_modules/@tauri-apps/cli/tauri.js\\" |g' "$PBXPROJ"
  echo "[configure-ios] Build Rust Code phase patched: pnpm tauri -> node tauri.js (bypass pnpm)"
else
  echo "[configure-ios] Build Rust Code phase: no 'pnpm tauri' prefix found (already patched or not pnpm-wrapped)"
fi

# --- 5. Sync AppIcon from src-tauri/icons/ios --------------------------------
# gen/apple 是本地持久产物（gitignored），configure 时若已存在会跳过 init，
# 图标可能停留在旧版本（Android 每次都由 tauri icon 按 icon.png 重新生成）。
# 这里把 src-tauri/icons/ios/（tauri icon 的输出，与 Android 同源 icon.png）
# 同步到 Xcode 工程的 AppIcon.appiconset，保证 iOS 图标与 Android 一致。

IOS_ICON_SRC="src-tauri/icons/ios"
APPICON_DIR="$GEN_APPLE/Assets.xcassets/AppIcon.appiconset"
if [ -d "$IOS_ICON_SRC" ] && [ -d "$APPICON_DIR" ]; then
  cp "$IOS_ICON_SRC"/AppIcon-*.png "$APPICON_DIR/"
  echo "[configure-ios] AppIcon synced from $IOS_ICON_SRC (same source as Android)"
else
  echo "[configure-ios] WARNING: AppIcon dirs missing (src=$IOS_ICON_SRC dst=$APPICON_DIR);" \
    "run 'pnpm run init' first (generates src-tauri/icons/ios via 'tauri icon')" >&2
fi

# --- 6. Show the patched shell script for verification -----------------------

echo "[configure-ios] Build Rust Code shellScript (first 220 chars):"
grep -o 'shellScript = ".*' "$PBXPROJ" | head -3 | cut -c1-220 || true

echo "[configure-ios] iOS project configured: iPhone + iPad, data dir visible in the Files app"
