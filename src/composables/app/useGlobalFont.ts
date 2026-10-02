/**
 * 全局字体 Composable
 *
 * 把设置中的自定义字体名同步到 <html> 的 --font-app；为空时 base.css 中的
 * 回退栈 --font-sans 生效。初始菜单 / 加载页因自带显式 font-family 不会继承
 * 此变量，自动保持原有字体。
 *
 * 另外在启动时预取系统字体列表并注册已导入字体的 @font-face 规则：前者避免
 * 用户打开设置页时才触发 IPC 造成可感知卡顿，后者确保 store 恢复字体选择前
 * 用户之前导入的字体已可用。
 *
 * 在 App.vue 中调用一次以激活全局字体。
 */
import { watch } from "vue";
import { useSettingsStore } from "@/stores/modules/settings";
import { listSystemFonts, getImportedFonts, registerAllImportedFonts } from "@/api/services/font";

export function useGlobalFont() {
  const settingsStore = useSettingsStore();

  function applyFont(font?: string) {
    // 留空 → 软件默认（base.css 的 --font-sans 原版字体栈）
    document.documentElement.style.setProperty("--font-app", font ? `'${font}'` : "");
  }
  watch(() => settingsStore.text.fontFamily, applyFont, { immediate: true });

  // 提前预取系统字体列表：在应用初始化时即调用一次 Rust 枚举并入内存缓存，
  // 避免打开设置页时才触发 IPC 造成可感知的卡顿。注：忽略结果即可，
  // SettingsText 进入时直接命中 font.ts 的缓存。
  void listSystemFonts();

  // 启动时加载导入字体并注册 @font-face 规则，确保用户之前导入的字
  // 体在 settings store 恢复字体选择前已可用。
  void getImportedFonts().then((fonts) => {
    registerAllImportedFonts(fonts);
  });
}
