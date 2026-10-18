// Android 构建前：解析 NDK 目录并打印，供 package.json 的 android:* 脚本导出成
// ANDROID_NDK_HOME。
//
// 背景：参考音频的 Opus 解码用 `opus` crate（crates/ling-chat-main），它的
// opusic-sys 默认 bundled —— 自带 libopus 源码，用 cmake 编成静态库。选择 NDK 的
// cmake toolchain 时它**只认 ANDROID_NDK_HOME**（见其 build.rs 的 get_android_vars），
// 而 Tauri CLI 只处理 ANDROID_HOME / NDK_HOME 并自己探测 NDK，不会导出这个变量。
// 缺了它 opusic-sys 会用宿主编译器编出 x86_64 的目标文件，链接阶段直接失败。
//
// 解析逻辑见 scripts/lib/android-ndk.mjs（自编 NNAPI 版 sherpa 的脚本也用同一份）。
// stdout 只输出解析出的目录（供命令替换捕获），人类可读日志一律走 stderr。

import { ndkHintLines, resolveNdk } from "./lib/android-ndk.mjs";

const ndk = resolveNdk();
if (!ndk) {
  console.error(
    [
      "[android-ndk] 找不到 Android NDK，无法交叉编译 libopus（参考音频的 Opus 解码）。",
      ...ndkHintLines(),
    ].join("\n"),
  );
  process.exit(1);
}
console.log(ndk);
