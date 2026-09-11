// 平台判断工具
//
// 在 Tauri 2 的 Android WebView 里,navigator.userAgent 包含 "Android"
// 字符串(标准 Chromium WebView UA)。这是判断"是否在安卓上跑"最简单
// 也最稳的运行时信号——不需要任何 Tauri API,也不依赖注入的全局变量。
//
// 这里仅用于"是否走 Android 拍照/相册路径"的分支判断,不要拿来做精细 UA 嗅探。

export function isAndroid(): boolean {
  if (typeof navigator === "undefined") return false;
  return /android/i.test(navigator.userAgent);
}

/** 是否 Windows 桌面端（WebView2 UA 含 "Windows NT"，Linux 为 "X11; Linux"）。
 *  用于「仅 Windows 可用」的工具/功能标注。 */
export function isWindows(): boolean {
  if (typeof navigator === "undefined") return false;
  return /windows/i.test(navigator.userAgent);
}

/** 是否 iOS（WKWebView UA 含 iPhone/iPad/iPod；现代 iPadOS 桌面版 UA 为
 *  MacIntel + 多点触控）。用于「仅 iOS 生效」的安全区/键盘适配分支判断。 */
export function isIOS(): boolean {
  if (typeof navigator === "undefined") return false;
  return (
    /iphone|ipad|ipod/i.test(navigator.userAgent) ||
    (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1)
  );
}

/** 是否移动端（Android / iOS）。App.vue 的键盘/安全区/滚动适配仅移动端挂载，桌面端不运行。 */
export function isMobile(): boolean {
  return isAndroid() || isIOS();
}
