// Android NDK 目录解析（公共逻辑）。
//
// 两处使用：
//   - scripts/resolve-android-ndk.mjs      —— CLI，只为打印目录（package.json 用）
//   - scripts/build-android-sherpa-nnapi.mjs —— 自编 NNAPI 版 sherpa 要用
//     $NDK/build/cmake/android.toolchain.cmake
//
// 解析优先级（与 Tauri / cargo-mobile2 的探测习惯保持一致）：
//   1. 已设置的 ANDROID_NDK_HOME（尊重用户显式指定）
//   2. NDK_HOME
//   3. $ANDROID_HOME/ndk、$ANDROID_SDK_ROOT/ndk 以及各平台默认安装位置里版本最高的那个

import { existsSync, readdirSync } from "fs";
import { homedir } from "os";
import { join } from "path";

/** 目录存在则返回，否则 undefined。 */
function existingDir(dir) {
  return dir && existsSync(dir) ? dir : undefined;
}

/** 把 "30.0.16248370" 这样的版本号切成数字数组用于比较。 */
function versionParts(name) {
  return name.split(".").map((part) => Number.parseInt(part, 10) || 0);
}

/** 取 `<root>/ndk` 下版本号最高的 NDK 目录。 */
function newestNdk(root) {
  const ndkRoot = existingDir(root && join(root, "ndk"));
  if (!ndkRoot) return undefined;
  let entries;
  try {
    entries = readdirSync(ndkRoot, { withFileTypes: true });
  } catch {
    return undefined;
  }
  const versions = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    // 按段比较：字符串排序会把 "9.x" 排到 "30.x" 之后
    .sort((a, b) => {
      const left = versionParts(a);
      const right = versionParts(b);
      for (let i = 0; i < Math.max(left.length, right.length); i += 1) {
        const diff = (left[i] || 0) - (right[i] || 0);
        if (diff !== 0) return diff;
      }
      return 0;
    });
  return versions.length ? join(ndkRoot, versions[versions.length - 1]) : undefined;
}

/** 按上述优先级找 NDK 目录；找不到返回 undefined。 */
export function resolveNdk(env = process.env, home = homedir()) {
  const candidates = [
    existingDir(env.ANDROID_NDK_HOME),
    existingDir(env.NDK_HOME),
    newestNdk(env.ANDROID_HOME),
    newestNdk(env.ANDROID_SDK_ROOT),
    newestNdk(join(home, "Android", "Sdk")), // Linux
    newestNdk(join(home, "Library", "Android", "sdk")), // macOS
    newestNdk(env.LOCALAPPDATA && join(env.LOCALAPPDATA, "Android", "Sdk")), // Windows
  ];
  return candidates.find(Boolean);
}

/** 找不到 NDK 时的排错提示（多个脚本共用同一段文案）。 */
export function ndkHintLines() {
  return [
    "  已查找: ANDROID_NDK_HOME、NDK_HOME、$ANDROID_HOME/ndk、$ANDROID_SDK_ROOT/ndk、默认安装位置",
    "  解决办法: 导出 ANDROID_NDK_HOME 指向 NDK 目录，例如",
    "    export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/30.0.16248370",
  ];
}
