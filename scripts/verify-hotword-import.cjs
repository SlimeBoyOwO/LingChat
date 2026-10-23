/**
 * 批量热词解析器的可复跑 fixture。
 *
 * 用法：
 *   node scripts/verify-hotword-import.cjs
 *
 * 每次运行都用 tsc 把纯函数模块重新编译到系统临时目录（不在仓库里留产物），
 * 并在 Node 侧打桩 @tauri-apps/api/core（解析器只依赖 hotword.ts 的长度判据，不会调用 IPC）。
 */
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const Module = require("node:module");
const os = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "..");

// 不复用旧产物：源码改动后重跑立即生效
const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "hotword-import-"));
const modulePath = path.join(outDir, "utils", "hotwordImport.js");
const tscPath = require.resolve("typescript/bin/tsc");
execFileSync(
  process.execPath,
  [
    tscPath,
    "src/utils/hotwordImport.ts",
    "--outDir",
    outDir,
    "--rootDir",
    "src",
    "--module",
    "commonjs",
    "--target",
    "es2022",
    "--skipLibCheck",
    "--esModuleInterop",
  ],
  { cwd: root, stdio: "inherit" },
);

// 仓库根 package.json 是 "type": "module"；给临时产物一个 CommonJS 边界。
fs.writeFileSync(path.join(outDir, "package.json"), '{"type":"commonjs"}');

const originalLoad = Module._load;
Module._load = function (request, parent, isMain) {
  if (request === "@tauri-apps/api/core") {
    return { invoke: async () => {
      throw new Error("IPC is not available in the parser fixture");
    } };
  }
  return originalLoad.call(this, request, parent, isMain);
};

const { parseHotwordImport } = require(modulePath);

const parse = (text, existing = [], capacity = 200, defaultWeight = 4) =>
  parseHotwordImport(text, defaultWeight, existing, capacity);

const rowsOf = (text, existing, capacity, defaultWeight) =>
  parse(text, existing, capacity, defaultWeight).rows;

// 基本格式与纯数字权重
assert.deepEqual(rowsOf("1.钦灵（5）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1. 钦灵（权重 5）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1. 钦灵（权重5）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1. 钦灵（权重＝５）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1、钦灵（５）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("一、钦灵(5）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("• 钦灵（权重：5）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("-40℃"), [{ text: "-40℃", weight: 4 }]);
assert.deepEqual(rowsOf("*NSYNC"), [{ text: "*NSYNC", weight: 4 }]);
assert.deepEqual(rowsOf("3.5mm"), [{ text: "3.5mm", weight: 4 }]);
assert.deepEqual(rowsOf("2026.10.20"), [{ text: "2026.10.20", weight: 4 }]);
assert.deepEqual(rowsOf("诺一（限定）"), [{ text: "诺一（限定）", weight: 4 }]);
assert.deepEqual(rowsOf("钦灵（5人）"), [{ text: "钦灵（5人）", weight: 4 }]);
assert.deepEqual(rowsOf("钦灵（别名）（5）"), [{ text: "钦灵（别名）", weight: 5 }]);
assert.deepEqual(rowsOf("钦灵（5）（备注）"), [{ text: "钦灵（5）（备注）", weight: 4 }]);

// 全角序号与项目符号
assert.deepEqual(rowsOf("１．热词"), [{ text: "热词", weight: 4 }]);
assert.deepEqual(rowsOf("１、热词"), [{ text: "热词", weight: 4 }]);
assert.equal(parse("１．").details[0].label, "1");
assert.equal(parse("１、").details[0].label, "1");
assert.equal(parse("一、").details[0].label, "一");
for (const prefix of ["- ", "* ", "+ ", "• "]) {
  assert.deepEqual(rowsOf(`${prefix}热词`), [{ text: "热词", weight: 4 }], `bullet ${prefix}`);
}
assert.deepEqual(rowsOf("1) 热词"), [{ text: "热词", weight: 4 }]);

// Markdown 包装
assert.deepEqual(rowsOf("1. **厄洛替尼盐酸盐**"), [
  { text: "厄洛替尼盐酸盐", weight: 4 },
]);
assert.deepEqual(rowsOf("1. `DeepSeek-harness`"), [{ text: "DeepSeek-harness", weight: 4 }]);
assert.deepEqual(rowsOf("1. __诺一钦灵__"), [{ text: "诺一钦灵", weight: 4 }]);
assert.deepEqual(rowsOf("> 1. 热词"), [{ text: "热词", weight: 4 }]);
// 缩进引用块：先 trim 再剥 >，行首缩进不影响引用与噪音识别
assert.deepEqual(rowsOf("　> 1. 热词"), [{ text: "热词", weight: 4 }]);
const quotedNoise = parse("  > ## 标题\n\n> 1. 热词");
assert.equal(quotedNoise.ignoredCount, 1);
assert.equal(quotedNoise.rows.length, 1);
assert.deepEqual(rowsOf("```\n1. 热词\n```"), [{ text: "热词", weight: 4 }]);
assert.deepEqual(rowsOf("## 词表\n---\n1. 热词"), [{ text: "热词", weight: 4 }]);
assert.deepEqual(rowsOf("**热词**（5）"), [{ text: "热词", weight: 5 }]);
assert.deepEqual(rowsOf("**热词（5）**"), [{ text: "热词", weight: 5 }]);

// 权重异常
const badWeights = ["0", "6", "49", "51"];
for (const raw of badWeights) {
  const review = parse(`钦灵（${raw}）`);
  assert.equal(review.invalidCount, 1, `weight ${raw} should be invalid`);
  assert.equal(review.details[0].reason, "badWeight");
}
const badWeightTexts = ["权重 abc", "权重", "权重说明"];
for (const raw of badWeightTexts) {
  const review = parse(`钦灵（${raw}）`);
  assert.equal(review.invalidCount, 1, `weight ${raw} should be invalid`);
  assert.equal(review.details[0].reason, "badWeight");
}
assert.deepEqual(rowsOf("钦灵（abc）"), [{ text: "钦灵（abc）", weight: 4 }]);
assert.equal(parse("钦灵（权重 5").details[0].reason, "unclosedWeight");
assert.equal(parse("钦灵(权重 5").details[0].reason, "unclosedWeight");
assert.deepEqual(rowsOf("钦灵（5"), [{ text: "钦灵（5", weight: 4 }]);

// 长度判据：首个中文样本实际为 12 个字符，低于 15 字符上限（并非边界样本）
assert.equal(parse("１５个字符的中文热词词条").rows.length, 1);
assert.equal(parse("１２３４５６７８９０１２３４５６").invalidCount, 1);
assert.equal(parse("one two three four five six seven").rows.length, 1);
assert.equal(parse("one two three four five six seven eight").invalidCount, 1);

// 非 ASCII 按码点计数：15 字符通过，16 字符拒绝
const fifteenChars = "一二三四五六七八九十甲乙丙丁戊";
assert.equal([...fifteenChars].length, 15);
assert.equal(parse(fifteenChars).rows.length, 1);
assert.equal(parse(`${fifteenChars}己`).invalidCount, 1);
assert.equal(parse(`${fifteenChars}己`).details[0].reason, "tooLongChars");

// emoji 代理对同样按码点计数：15 通过，16 拒绝（String.length 会是 30/32）
const fifteenEmoji = "😀😁😂🤣😃😄😅😆😉😊😋😎😍😘🥰";
assert.equal([...fifteenEmoji].length, 15);
assert.equal(parse(fifteenEmoji).rows.length, 1);
assert.equal(parse(`${fifteenEmoji}🤩`).invalidCount, 1);

// 去重与容量
const dup = parse("A\nA\nB", [], 200);
assert.equal(dup.rows.length, 2);
assert.equal(dup.inputDuplicateCount, 1);
assert.equal(dup.rows[0].text, "A");

const merge = parse("A（5）\nB", [{ text: "A" }], 200);
assert.equal(merge.rows.length, 2);
assert.equal(merge.mergeCount, 1);
assert.equal(merge.newCount, 1);

const full = parse("A\nB", [{ text: "x" }, { text: "y" }], 2);
assert.equal(full.rows.length, 0);
assert.equal(full.overCapacityCount, 2);

const partial = parse("A\nB", [{ text: "x" }], 2);
assert.equal(partial.rows.length, 1);
assert.equal(partial.overCapacityCount, 1);

const mergeAtFull = parse("A", [{ text: "A" }], 1);
assert.equal(mergeAtFull.rows.length, 1);
assert.equal(mergeAtFull.overCapacityCount, 0);

// 输入内重复取最大权重，保留首现位置
const dupMax = parse("甲（2）\n乙\n甲（5）");
assert.deepEqual(dupMax.rows, [
  { text: "甲", weight: 5 },
  { text: "乙", weight: 4 },
]);
assert.equal(dupMax.inputDuplicateCount, 1);
const dupMaxReversed = parse("甲（5）\n甲（2）");
assert.deepEqual(dupMaxReversed.rows, [{ text: "甲", weight: 5 }]);

// 容量 0：重复词仍可合并保存，新增词全部超限
const zeroCapacityMerge = parse("甲（5）", [{ text: "甲" }], 0);
assert.equal(zeroCapacityMerge.rows.length, 1);
assert.equal(zeroCapacityMerge.mergeCount, 1);
assert.equal(zeroCapacityMerge.overCapacityCount, 0);

const zeroCapacityNew = parse("甲", [], 0);
assert.equal(zeroCapacityNew.rows.length, 0);
assert.equal(zeroCapacityNew.newCount, 0);
assert.equal(zeroCapacityNew.overCapacityCount, 1);
assert.equal(zeroCapacityNew.details[0].reason, "overCapacity");

// 统计口径：importableCount = newCount + mergeCount，ignored/invalid/overCapacity 各计各的
const stats = parse(
  "## 词表\n1. 甲\n2. 甲（5）\n3. 乙\n4. 乙（6）\n5. 丙（权重\n6. 丁",
  [{ text: "甲" }],
  2,
);
assert.equal(stats.ignoredCount, 1);
assert.equal(stats.invalidCount, 2);
assert.equal(stats.inputDuplicateCount, 1);
assert.equal(stats.mergeCount, 1);
assert.equal(stats.newCount, 1);
assert.equal(stats.overCapacityCount, 1);
assert.equal(stats.importableCount, stats.newCount + stats.mergeCount);
assert.deepEqual(stats.rows, [
  { text: "甲", weight: 5 },
  { text: "乙", weight: 4 },
]);
assert.equal(stats.details.find((detail) => detail.reason === "overCapacity").line, 7);

// CRLF / BOM / 全角空格
assert.deepEqual(rowsOf("﻿1. 热词\r\n2. 词条"), [
  { text: "热词", weight: 4 },
  { text: "词条", weight: 4 },
]);
assert.deepEqual(rowsOf("　1. 热词　"), [{ text: "热词", weight: 4 }]);

// 断言全部通过后清理临时构建目录；失败时保留现场便于排查
fs.rmSync(outDir, { recursive: true, force: true });
console.log("hotword import fixture: all assertions passed");
