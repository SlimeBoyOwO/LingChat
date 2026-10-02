/**
 * 逐字符淡入+上浮的字符 HTML 生成器。
 *
 * 这段逻辑此前在三处各抄了一份、逐字相同：主界面台词区、主界面动作区
 * （GameDialog.vue 的 charReveal / motionReveal）与桌宠 DialogueBox.vue。
 *
 * 引用的 `tw-char-rise` keyframes 是**全局**的，见 src/assets/styles/dialogue-text.css
 * ——span 由 JS 动态插入，scoped 选择器命中不了。
 */
import { escapeHtml } from "@/utils/escapeHtml";

/** 单字符动画的 inline 样式片段；不需要动画时拼空串即可 */
export const TW_CHAR_RISE = ";animation:tw-char-rise .28s cubic-bezier(.22, 1, .36, 1) forwards";

/** createCharRevealWriter 的 charHtml 实现（全项目唯一一份） */
export function charRevealCharHtml(
  char: string,
  _index: number,
  _rawText: string,
  animate: boolean,
): string {
  if (char === "\n") return "<br>";
  if (char === " ") return " ";
  return `<span style="display:inline-block${animate ? TW_CHAR_RISE : ""}">${escapeHtml(char)}</span>`;
}
