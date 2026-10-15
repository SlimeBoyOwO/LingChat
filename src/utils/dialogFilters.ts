// 各模块 dialog filter 的 Android / 桌面端分流
//
// 新增导入类型时按需在两个表里同步加；不要让某一端漏掉。

import { isAndroid } from "./platform";

/** Android 端 dialog 给 Intent.EXTRA_MIME_TYPES 的 mime 字符串。 */
const ANDROID_MUSIC_MIME = [
  "audio/mpeg", // mp3
  "audio/x-wav", // wav
  "audio/x-flac", // flac —— 桌面端常见的 flac 在部分 Android MimeTypeMap 里缺失
  "audio/ogg", // ogg / oga
  "audio/webm", // webm / weba（容器里包 opus/vorbis）
];

/** 桌面端用扩展名（Tauri 走 rfd，不经过 mime 转换）。 */
const DESKTOP_MUSIC_EXT = ["mp3", "wav", "flac", "webm", "weba", "ogg"];

/**
 * 音乐 / 环境音导入 dialog 的 filter。Android / 桌面端各一套，避免 MimeTypeMap
 * 丢扩展名导致部分格式在系统文件选择器里不可见。
 */
export function musicDialogFilters(): { name: string; extensions: string[] }[] {
  if (isAndroid()) {
    return [{ name: "Music", extensions: ANDROID_MUSIC_MIME }];
  }
  return [{ name: "Music", extensions: DESKTOP_MUSIC_EXT }];
}

/** Android 端参考音频的 mime（对应后端可解码的容器/编码）。 */
const ANDROID_AUDIO_MIME = [
  "audio/mpeg", // mp3
  "audio/x-wav", // wav —— Android MimeTypeMap 的标准映射
  "audio/wav", // 少数 ROM 用这个
  "audio/x-flac", // flac —— 部分 ROM 的 MimeTypeMap 没注册
  "audio/flac",
  "audio/ogg", // ogg / oga（Vorbis；Opus 在 ogg 里后端解不了，会明确报错）
  "audio/mp4", // m4a / mp4 容器里的 AAC
  "audio/x-m4a",
];

/** 桌面端用扩展名（Tauri 走 rfd，不经过 mime 转换）。 */
const DESKTOP_AUDIO_EXT = ["mp3", "wav", "flac", "ogg", "oga", "m4a", "mp4"];

/**
 * 参考音频导入 dialog 的 filter。
 *
 * 后端 `import_sherpa_ref_audio` 会用 symphonia 解码后统一转成 16-bit PCM WAV，
 * 所以这里放开常见音频格式；真正能不能解由后端判定（Opus/WebM 不支持，会报
 * 「格式不支持」）。注意别把原始 `.aac`/`.webm` 列进来：symphonia 0.5 没有 ADTS
 * 与 Matroska 读取器，选了必然失败。
 */
export function audioDialogFilters(): { name: string; extensions: string[] }[] {
  if (isAndroid()) {
    return [{ name: "Audio", extensions: ANDROID_AUDIO_MIME }];
  }
  return [{ name: "Audio", extensions: DESKTOP_AUDIO_EXT }];
}
