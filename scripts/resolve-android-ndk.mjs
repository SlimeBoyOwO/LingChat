// Android 构建前：解析 NDK 目录并打印，供 package.json 的 android:* 脚本导出成
// ANDROID_NDK_HOME。
//
// 背景：参考音频的 Opus 解码用 `opus` crate（crates/ling-chat-main），它的
// opusic-sys 默认 bundled —— 自带 libopus 源码，用 cmake 编成静态库。选择 NDK 的
// cmake toolchain 时它**只认 ANDROID_NDK_HOME**（见其 build.rs 的 get_android_vars），
// 而 Tauri CLI 只处理 ANDROID_HOME / NDK_HOME 并自己探测 NDK，不会导出这个变量。
// 缺了它 opusic-sys 会用宿主编译器编出 x86_64 的目标文件，链接阶段直接失败。
//
// 解析优先级（与 Tauri / cargo-mobile2 的探测习惯保持一致）：
//   1. 已设置的 ANDROID_NDK_HOME（尊重用户显式指定）
//   2. NDK_HOME
//   3. $ANDROID_HOME/ndk、$ANDROID_SDK_ROOT/ndk 以及各平台默认安装位置里版本最高的那个
//
// stdout 只输出解析出的目录（供命令替换捕获），人类可读日志一律走 stderr。

import { existsSync, readdirSync } from "fs";
import { join } from "path";
import { homedir } from "os";

const log = console.error;

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

const candidates = [
  existingDir(process.env.ANDROID_NDK_HOME),
  existingDir(process.env.NDK_HOME),
  newestNdk(process.env.ANDROID_HOME),
  newestNdk(process.env.ANDROID_SDK_ROOT),
  newestNdk(join(homedir(), "Android", "Sdk")), // Linux
  newestNdk(join(homedir(), "Library", "Android", "sdk")), // macOS
  newestNdk(process.env.LOCALAPPDATA && join(process.env.LOCALAPPDATA, "Android", "Sdk")), // Windows
];

const ndk = candidates.find(Boolean);
if (!ndk) {
  log(
    [
      "[android-ndk] 找不到 Android NDK，无法交叉编译 libopus（参考音频的 Opus 解码）。",
      "  已查找: ANDROID_NDK_HOME、NDK_HOME、$ANDROID_HOME/ndk、$ANDROID_SDK_ROOT/ndk、默认安装位置",
      "  解决办法: 导出 ANDROID_NDK_HOME 指向 NDK 目录，例如",
      "    export ANDROID_NDK_HOME=$HOME/Android/Sdk/ndk/30.0.16248370",
    ].join("\n"),
  );
  process.exit(1);
}
console.log(ndk);
