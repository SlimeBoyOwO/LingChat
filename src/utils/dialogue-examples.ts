/** 对话示例仍保存为原有文本；解析只为卡片编辑，不改写未编辑的前缀、换行或内容。 */
export type ExampleRole = "user" | "assistant";
export interface ExampleMessage {
  role: ExampleRole;
  prefix: string;
  content: string;
  separator: string;
}
export interface ExampleDocument {
  leading: string;
  newline: string;
  messages: ExampleMessage[];
}
const USER_LABELS = ["用户", "玩家", "user", "human", "{{user}}"];
const ASSISTANT_LABELS = ["角色", "助手", "assistant", "ai", "{{char}}"];
function escapeRegex(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function parseDialogueExamples(
  text: string,
  userName = "",
  characterName = "",
): ExampleDocument {
  const users = [...USER_LABELS, ...(userName ? [userName] : [])];
  const assistants = [...ASSISTANT_LABELS, ...(characterName ? [characterName] : [])];
  const labels = [...new Set([...users, ...assistants])]
    .sort((a, b) => b.length - a.length)
    .map(escapeRegex)
    .join("|");
  const header = new RegExp(
    String.raw`^[ \t]*(?:(?:\d+[.．、][ \t]*)?(?:${labels})[：:][ \t]*|\d+[.．、][ \t]*)`,
    "gmi",
  );
  const matches = Array.from(text.matchAll(header));
  const document: ExampleDocument = {
    leading: "",
    newline: text.includes("\r\n") ? "\r\n" : "\n",
    messages: [],
  };
  const append = (prefix: string, body: string) => {
    const separator = body.match(/(?:\r?\n[ \t]*)+$/)?.[0] ?? "";
    const label = prefix.replace(/^[ \t]*(?:\d+[.．、][ \t]*)?/, "").replace(/[：:][ \t]*$/, "");
    document.messages.push({
      role: users.some((name) => name.toLowerCase() === label.toLowerCase()) ? "user" : "assistant",
      prefix,
      content: body.slice(0, body.length - separator.length),
      separator,
    });
  };
  if (!matches.length) {
    if (text.trim()) append("", text);
    else document.leading = text;
    return document;
  }
  document.leading = text.slice(0, matches[0]!.index);
  matches.forEach((match, index) => {
    const start = match.index! + match[0].length;
    const end = matches[index + 1]?.index ?? text.length;
    append(match[0], text.slice(start, end));
  });
  return document;
}

export function serializeDialogueExamples(document: ExampleDocument): string {
  return (
    document.leading +
    document.messages
      .map(
        (message, index) =>
          message.prefix +
          message.content +
          (message.separator || (index < document.messages.length - 1 ? document.newline : "")),
      )
      .join("")
  );
}

export function setExampleRole(message: ExampleMessage, role: ExampleRole) {
  if (message.role === role) return;
  const leader = message.prefix.match(/^[ \t]*(?:\d+[.．、][ \t]*)?/)?.[0] ?? "";
  message.role = role;
  message.prefix = leader + (role === "user" ? "用户：" : "角色：");
}

/** 只有增删和排序才重排已有编号；输入消息正文不会动原编号及其标点。 */
export function renumberExamples(messages: ExampleMessage[]) {
  let number = 0;
  for (const message of messages) {
    if (/^[ \t]*\d+[.．、]/.test(message.prefix)) {
      message.prefix = message.prefix.replace(
        /^([ \t]*)\d+([.．、])/,
        (_, indent, separator) => `${indent}${++number}${separator}`,
      );
    }
  }
}
