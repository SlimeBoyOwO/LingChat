/**
 * 消息语义分类（玩家 / AI / 旁白）共享工具。
 *
 * 历史消息优先读取 messageType（convertInitLines 与实时事件处理器都会写入）；
 * 旧数据没有 messageType 时，用旁白姓名集合 + type 做启发式兜底。
 * system 仅作为类型保留：历史列表不渲染 system 行，因此本工具对 system 返回 null。
 */

/** 旁白/系统提示的展示名集合（兼容旧数据启发式判断） */
export const NARRATION_NAMES = new Set(["", "旁白", "系统", "Narrator", "System"]);

/** 历史列表实际渲染的消息分类 */
export type MessageKind = "player" | "ai" | "narrator";

/** `{角色: 正文}` 包裹的解析结果 */
export interface UnwrappedPromptRole {
  /** 是否识别为完整角色包裹 */
  wrapped: boolean;
  /** 包裹角色名（如「旁白」）；非包裹时为 null */
  role: string | null;
  /** 包裹内正文；非包裹时返回原文 */
  text: string;
}

/** 解析 `{旁白: ...}` 这类角色包裹，返回正文与角色名 */
export function unwrapPromptRole(content: string): UnwrappedPromptRole {
  const match = content.match(/^\s*\{\s*([^{}\s:：]+)\s*[:：]\s*([\s\S]*?)\s*\}\s*$/);
  if (!match) {
    return { wrapped: false, role: null, text: content };
  }
  return { wrapped: true, role: match[1], text: match[2].trim() };
}

/**
 * 计算消息在历史列表中的分类。
 * - messageType 优先：player / ai / narrator 原样返回；
 * - messageType 为 system 时返回 null（不渲染）；
 * - 旧数据无 messageType：旁白姓名 → narrator，否则 message → player、reply → ai。
 */
export function messageKindOf(message: {
  messageType?: "player" | "narrator" | "ai" | "system";
  displayName?: string;
  type?: "message" | "reply";
}): MessageKind | null {
  if (message.messageType === "player") return "player";
  if (message.messageType === "ai") return "ai";
  if (message.messageType === "narrator") return "narrator";
  if (message.messageType === "system") return null;

  // 旧数据兜底：保持与历史版本一致的旁白姓名启发式
  if (NARRATION_NAMES.has(message.displayName || "")) return "narrator";
  return message.type === "message" ? "player" : "ai";
}
