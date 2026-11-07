/**
 * 把 `uiStore.currentBackground` 里的值解析成 webview 能直接加载的 URL。
 *
 * 设置里存的可能是构建期别名（`@/assets/images/default_bg...`）、绝对路径、
 * data URL 或 http 地址。别名在运行时解析不了，回落到打包进来的默认背景；
 * 本地绝对路径要走 `convertFileSrc` 换成 asset 协议。
 *
 * 抽成独立函数是因为背景现在有两个消费者：旧的 `GameBackground.vue`（CSS 滤镜路径）
 * 和新的 `LightingStage.vue`（WebGL 路径）。两边必须用同一套规则，否则同一场景在
 * 两条路径下会显示不同的图。
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import defaultBackgroundUrl from "@/assets/images/default_bg.webp";

export function resolveBackgroundSrc(raw: string | null | undefined): string {
  const bg = raw ?? "";
  // 设置里保存的是构建期别名（稳定标识），运行时浏览器无法解析，落到打包后的默认背景
  if (bg.startsWith("@/assets/images/default_bg")) return defaultBackgroundUrl;
  if (
    !bg ||
    bg.startsWith("http://") ||
    bg.startsWith("https://") ||
    bg.startsWith("@/") ||
    bg.startsWith("data:")
  ) {
    return bg;
  }
  return convertFileSrc(bg);
}
