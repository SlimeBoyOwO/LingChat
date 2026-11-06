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
