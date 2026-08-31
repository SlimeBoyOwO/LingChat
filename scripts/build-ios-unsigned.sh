#!/usr/bin/env bash
# ============================================================================
# Build LingChat's iOS unsigned IPA (macOS only).
#
# Pipeline:
#   1. configure-ios-project.sh -- init/configure the Xcode project (iPhone + iPad)
#   2. prepare-bundled-resources.mjs -- pack default resources into data.7z and
#      deploy to gen/apple/assets/data/ (folder reference -> app bundle root)
#   3. pnpm tauri ios build --no-sign -- build frontend + cross-compile Rust and
#      produce an UNSIGNED IPA (tauri-cli has a built-in create_ipa step)
#
# Output: src-tauri/gen/apple/target/**/*.ipa
#   Install via sideloading tools (Sideloadly / AltStore / 3uTools) or sign it
#   with a developer certificate for distribution.
#
# Requirements (macOS only): Xcode, xcodegen, Rust target aarch64-apple-ios
# (aarch64-apple-ios-sim optional for the simulator).
# ============================================================================
set -euo pipefail

# 构建环境与 CI（build-ios.yml）保持完全一致（单一事实来源），确保本地与
# GitHub Actions 产出等价 IPA：
# - CI=true + pnpm_config_*：pnpm 11 的 verify-deps-before-run 在无 TTY 环境下
#   （脚本/CI 触发、Xcode Build Rust Code 阶段）会因模块目录需要清空重装而直接
#   中止（ERR_PNPM_ABORTED_REMOVE_MODULES_DIR_NO_TTY）；.npmrc 中的同名键会被
#   pnpm 11 过滤，因此用环境变量。
# - CARGO_PROFILE_RELEASE_*：等价于 [profile.release] opt-level="s" + strip=true
#   （项目 Cargo.toml 未覆盖 release profile，CI 由此控制；原先本地未设置，
#   会得到未 strip 的 opt-level=3 二进制，IPA 偏大）。
export CI=true
export pnpm_config_verify_deps_before_run=false
export pnpm_config_confirm_modules_purge=false
export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
export CARGO_PROFILE_RELEASE_STRIP=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# --- 1. Configure the Xcode project ------------------------------------------
echo "[build-ios] Step 1/3: configuring the iOS Xcode project"
bash scripts/configure-ios-project.sh

# --- 2. Pack resources into data.7z ------------------------------------------
echo "[build-ios] Step 2/3: packing default resources into data.7z (incl. third_party models)"
# 压缩等级 0-9 可选（默认 9），与 prepare-bundled-resources.mjs 的参数约定一致；
# CI 通过 IOS_BUNDLED_7Z_LEVEL 环境变量透传 workflow 的 compression 输入。
node scripts/prepare-bundled-resources.mjs "${IOS_BUNDLED_7Z_LEVEL:-9}"

# --- 3. Unsigned build --------------------------------------------------------
echo "[build-ios] Step 3/3: pnpm tauri ios build --no-sign (unsigned IPA)"
# beforeBuildCommand automatically runs prepare-desktop-resources.mjs + pnpm build
pnpm tauri ios build --no-sign "$@"

# --- 4. Locate the artifact ---------------------------------------------------
# cargo-mobile2 把产物放在 gen/apple/build/<arch>/ 下（如 build/arm64/LingChat.ipa）；
# 兼容旧目录 gen/apple/target 一并查找
IPA="$(find src-tauri/gen/apple/build src-tauri/gen/apple/target -name "*.ipa" 2>/dev/null | head -1 || true)"
if [ -n "$IPA" ]; then
  echo ""
  echo "[build-ios] unsigned IPA ready: $(pwd)/$IPA"
  echo "[build-ios] sideload to iPhone/iPad with Sideloadly / AltStore / 3uTools"
else
  echo "[build-ios] ERROR: no .ipa found under src-tauri/gen/apple/build or target, check the build log above" >&2
  exit 1
fi
