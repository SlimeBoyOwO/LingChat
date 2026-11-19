// 各模块 dialog filter 的 Android / 桌面端分流；新增格式时两端同步。

import { isAndroid } from "./platform";

/** Android 端 mime 列表（Intent.EXTRA_MIME_TYPES）。 */
const ANDROID_MUSIC_MIME = [
  "audio/mpeg", // mp3
  "audio/x-wav", // wav
  "audio/x-flac", // flac（部分 ROM 缺失）
  "audio/ogg", // ogg / oga
  "audio/webm", // webm / weba
];

/** 桌面端扩展名（不经 mime 转换）。 */
const DESKTOP_MUSIC_EXT = ["mp3", "wav", "flac", "webm", "weba", "ogg"];

/** 音乐/环境音导入 filter；两端分流，避免 MimeTypeMap 丢扩展名。 */
export function musicDialogFilters(): { name: string; extensions: string[] }[] {
  if (isAndroid()) {
    return [{ name: "Music", extensions: ANDROID_MUSIC_MIME }];
  }
  return [{ name: "Music", extensions: DESKTOP_MUSIC_EXT }];
}

/** Android 端参考音频 mime。 */
const ANDROID_AUDIO_MIME = [
  "audio/mpeg", // mp3
  "audio/x-wav", // wav（标准映射）
  "audio/wav", // wav（少数 ROM）
  "audio/x-flac", // flac（部分 ROM 缺失）
  "audio/flac",
  "audio/ogg", // ogg / oga
  "audio/opus", // .opus
  "audio/aac", // .aac（ADTS）
  "audio/mp4", // m4a / mp4
  "audio/x-m4a",
];

/** 桌面端扩展名（不经 mime 转换）。 */
const DESKTOP_AUDIO_EXT = ["mp3", "wav", "flac", "ogg", "oga", "opus", "aac", "m4a", "mp4"];

/**
 * 参考音频导入 filter：后端用 symphonia 解码后统一转 16-bit PCM WAV，能否解码由后端判定。
 * 别加 webm（symphonia 0.5 无 Matroska 读取器）。
 */
export function audioDialogFilters(): { name: string; extensions: string[] }[] {
  if (isAndroid()) {
    return [{ name: "Audio", extensions: ANDROID_AUDIO_MIME }];
  }
  return [{ name: "Audio", extensions: DESKTOP_AUDIO_EXT }];
}
