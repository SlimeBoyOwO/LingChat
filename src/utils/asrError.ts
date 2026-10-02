/**
 * ASR 错误解析（后端 err_to_user 的配套工具）。
 *
 * 后端 `err_to_user` 输出 `{"code":"<i18n_code>","detail":"<详情>"}` JSON——
 * 结构化契约避免旧 `CODE|detail` 拼接中 detail 含 `|` 导致解析错位。
 * 这里 JSON.parse 失败时回退旧格式与原始字符串（向后兼容），
 * 调用方只依赖 { code, detail? } 形状。
 */

export interface AsrErrorInfo {
  code: string;
  detail?: string;
}

export function parseAsrError(raw: unknown): AsrErrorInfo {
  if (typeof raw !== "string" || !raw) return { code: String(raw ?? "") };
  // 新格式：JSON 错误对象
  try {
    const parsed = JSON.parse(raw) as { code?: unknown; detail?: unknown };
    if (parsed && typeof parsed.code === "string") {
      return {
        code: parsed.code,
        detail: typeof parsed.detail === "string" ? parsed.detail : undefined,
      };
    }
  } catch {
    /* 非 JSON——回退旧格式 */
  }
  // 旧格式：`CODE|detail`（detail 可能含 |，取首段为 code）
  const idx = raw.indexOf("|");
  if (idx > 0) return { code: raw.slice(0, idx), detail: raw.slice(idx + 1) || undefined };
  return { code: raw };
}
