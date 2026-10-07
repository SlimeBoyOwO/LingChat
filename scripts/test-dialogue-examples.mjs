import assert from "node:assert/strict";
import {
  parseDialogueExamples,
  serializeDialogueExamples,
  setExampleRole,
  renumberExamples,
} from "../src/utils/dialogue-examples.ts";
const samples = [
  "",
  "   ",
  "没有标签的台词\n第二行\n",
  "1.“【高兴】一起吃蛋糕？（拿起蛋糕）<ケーキを食べましょうか？>”\n2.“【生气】不允许！”",
  "说明：保留此前置文本\n\nUser: Hello\r\nAssistant: Hi\r\n继续说话\r\n\r\n",
  "用户：今天怎么样？\n角色：【高兴】很好！\n用户：再见\n角色：再见。",
  "  02．角色：多行台词\n未标记的后续内容\n  03、用户：保留缩进\n",
  '{{user}}: test\n{{char}}: <tag> & "quoted"',
  "玩家(甲)：你好\n灵灵+：你好呀",
];
for (const sample of samples)
  assert.equal(
    serializeDialogueExamples(parseDialogueExamples(sample, "玩家(甲)", "灵灵+")),
    sample,
  );
const numbered = parseDialogueExamples(samples[3]);
assert.equal(numbered.messages.length, 2);
assert.equal(numbered.messages[0].role, "assistant");
numbered.messages[0].content = "新台词";
assert.equal(serializeDialogueExamples(numbered), "1.新台词\n2.“【生气】不允许！”");
numbered.messages.reverse();
renumberExamples(numbered.messages);
assert.equal(serializeDialogueExamples(numbered), "1.“【生气】不允许！”\n2.新台词\n");
setExampleRole(numbered.messages[0], "user");
assert.equal(numbered.messages[0].prefix, "1.用户：");
const dialogue = parseDialogueExamples(samples[5]);
assert.deepEqual(
  dialogue.messages.map((x) => x.role),
  ["user", "assistant", "user", "assistant"],
);
const names = parseDialogueExamples(samples[8], "玩家(甲)", "灵灵+");
assert.deepEqual(
  names.messages.map((x) => x.role),
  ["user", "assistant"],
);
const empty = parseDialogueExamples("");
empty.messages.push(
  { role: "user", prefix: "用户：", content: "你好", separator: "" },
  { role: "assistant", prefix: "角色：", content: "【高兴】你好", separator: "" },
);
assert.equal(serializeDialogueExamples(empty), "用户：你好\n角色：【高兴】你好");
console.log(
  "dialogue examples: lossless legacy/bilingual/CRLF parsing, editing, roles and reordering passed",
);
