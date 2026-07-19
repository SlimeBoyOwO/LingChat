// 平台判断工具
//
// 在 Tauri 2 的 Android WebView 里,navigator.userAgent 包含 "Android"
// 字符串(标准 Chromium WebView UA)。这是判断"是否在安卓上跑"最简单
// 也最稳的运行时信号——不需要任何 Tauri API,也不依赖注入的全局变量。
//
// 这里仅用于"是否走 Android 拍照/相册路径"的分支判断,不要拿来做精细 UA 嗅探。

export function isAndroid(): boolean {
  if (typeof navigator === 'undefined') return false
  return /android/i.test(navigator.userAgent)
}
