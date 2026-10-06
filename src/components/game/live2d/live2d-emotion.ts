/** 情绪到表情与动作绑定的查表规则，设置界面与运行时共用 */

import { EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import type { Live2dVariant } from "@/types/live2d";

/** 原始情绪词优先，映射词兜底：少了前者会让「哭泣」「难为情」变成死键 */
function emotionBindingKeys(emotion: string): string[] {
  const mapped = EMOTION_CONFIG_EMO[emotion] || "正常";
  return emotion === mapped ? [emotion] : [emotion, mapped];
}

/** 判空用 !== undefined 而不是真值判断：空串表示用户显式关掉了这个情绪的表情 */
export function pickEmotionBinding<T>(table: Record<string, T>, emotion: string): T | undefined {
  for (const key of emotionBindingKeys(emotion)) {
    const value = table[key];
    if (value !== undefined) return value;
  }
  return undefined;
}

/** 当前情绪对应的表情。抚摸结束后用它把表情收回去，必须与情绪路径取同一份。 */
export function emotionExpression(variant: Live2dVariant, emotion: string): string | undefined {
  return (
    pickEmotionBinding(variant.expressions, emotion) ?? variant.default_expression ?? undefined
  );
}
