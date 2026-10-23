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
    return {
      invoke: async () => {
        throw new Error("IPC is not available in the parser fixture");
      },
    };
  }
  return originalLoad.call(this, request, parent, isMain);
};

const { parseHotwordImport } = require(modulePath);

const parse = (text, existing = [], capacity = 200, targetCategory) =>
  parseHotwordImport(text, existing, capacity, targetCategory);
const rowsOf = (...args) => parse(...args).rows;

// 严格格式：序号.词条(权重)，全角数字与括号等价
assert.deepEqual(rowsOf("1.钦灵(5)\n2.deepseek(2)"), [
  { text: "钦灵", weight: 5 },
  { text: "deepseek", weight: 2 },
]);
assert.deepEqual(rowsOf("１．钦灵（５）"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1. 钦灵(５)"), [{ text: "钦灵", weight: 5 }]);
assert.deepEqual(rowsOf("1.诺一（限定）(5)"), [{ text: "诺一（限定）", weight: 5 }]);

// 格式错误：没有权重括号或没有序号都拒绝
for (const line of ["1.钦灵", "钦灵(5)", "1.钦灵(abc)", "1.钦灵(权重 5)"]) {
  const review = parse(line);
  assert.equal(review.invalidCount, 1, `${line} should be invalid`);
  assert.equal(review.details[0].reason, "badFormat", `${line} should be badFormat`);
}

// 空词条：匹配了格式但正文为空白，必须拒绝且不占容量
for (const line of ["1. (5)", "1.　(5)", "1.\t(5)"]) {
  const review = parse(line);
  assert.equal(review.rows.length, 0, `${line} should not produce a row`);
  assert.equal(review.invalidCount, 1, `${line} should be invalid`);
  assert.equal(review.details[0].reason, "badFormat", `${line} should be badFormat`);
}
const emptyCapacity = parse("1. (5)\n2.真词(4)", [], 1);
assert.deepEqual(emptyCapacity.rows, [{ text: "真词", weight: 4 }]);
assert.equal(emptyCapacity.overCapacityCount, 0, "空词条不应占用容量");

// 权重只接受 1、2、3、4、5、50
for (const weight of [0, 6, 49, 51]) {
  const review = parse(`1.甲(${weight})`);
  assert.equal(review.invalidCount, 1, `weight ${weight} should be invalid`);
  assert.equal(review.details[0].reason, "badWeight", `weight ${weight} should be badWeight`);
}
assert.deepEqual(rowsOf("1.甲(50)"), [{ text: "甲", weight: 50 }]);

// 长度边界：含非 ASCII 按码点 ≤ 15；纯 ASCII 按空白片段 ≤ 7
const fifteenChars = "一".repeat(15);
const fifteenEmoji = "😀".repeat(15);
const sevenSegments = "one two three four five six seven";
assert.equal(parse(`1.${fifteenChars}(4)`).rows.length, 1);
assert.equal(parse(`1.${fifteenChars}一(4)`).details[0].reason, "tooLongChars");
assert.equal(parse(`1.${fifteenEmoji}(4)`).rows.length, 1);
assert.equal(parse(`1.${fifteenEmoji}😀(4)`).details[0].reason, "tooLongChars");
assert.equal(parse(`1.${sevenSegments}(4)`).rows.length, 1);
assert.equal(parse(`1.${sevenSegments} eight(4)`).details[0].reason, "tooLongSegments");

// 输入内去重：保留首现行，权重取最大
const inputDup = parse("1.甲(2)\n2.甲(5)\n3.乙(4)");
assert.deepEqual(inputDup.rows, [
  { text: "甲", weight: 5 },
  { text: "乙", weight: 4 },
]);
assert.equal(inputDup.inputDuplicateCount, 1);
assert.equal(inputDup.newCount, 2);
assert.equal(inputDup.importableCount, inputDup.newCount + inputDup.mergeCount);

// 与草稿重复：只更新权重，不占容量
const merged = parse("1.甲(5)", [{ text: "甲" }]);
assert.deepEqual(merged.rows, [{ text: "甲", weight: 5 }]);
assert.equal(merged.mergeCount, 1);
assert.equal(merged.newCount, 0);
const mergeAtFull = parse("1.甲(5)", [{ text: "甲" }], 0);
assert.equal(mergeAtFull.rows.length, 1);
assert.equal(mergeAtFull.overCapacityCount, 0);

// 词库目标分类：判重只看目标分类，同词在别的分类算新增
const sameTextOtherCategory = parse(
  "1.甲(5)",
  [
    { text: "甲", category_index: 0 },
    { text: "甲", category_index: 1 },
  ],
  200,
  0,
);
assert.equal(sameTextOtherCategory.mergeCount, 1);
assert.equal(sameTextOtherCategory.newCount, 0);
const sameTextNewCategory = parse("1.甲(5)", [{ text: "甲", category_index: 0 }], 200, 2);
assert.equal(sameTextNewCategory.mergeCount, 0);
assert.equal(sameTextNewCategory.newCount, 1);

// 容量：超出的新增词阻止保存，重复词不占额度
const overCapacity = parse("1.甲(4)\n2.乙(4)", [], 1);
assert.equal(overCapacity.rows.length, 1);
assert.equal(overCapacity.newCount, 1);
assert.equal(overCapacity.overCapacityCount, 1);
assert.equal(overCapacity.details[0].reason, "overCapacity");

// CRLF、BOM 与全角空格
assert.deepEqual(rowsOf("﻿1.热词(4)\r\n2.词条(5)"), [
  { text: "热词", weight: 4 },
  { text: "词条", weight: 5 },
]);
assert.deepEqual(rowsOf("　1.热词(4)　"), [{ text: "热词", weight: 4 }]);

fs.rmSync(outDir, { recursive: true, force: true });
console.log("hotword import fixture: all assertions passed");
