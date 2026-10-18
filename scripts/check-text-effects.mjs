import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import ts from "typescript";

// 直接执行生产渲染器，检查 UTF-16 动画区间与逐字/追加/补全路径。
const source = await readFile(
  new URL("../src/utils/typewriter/charReveal.ts", import.meta.url),
  "utf8",
);
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
});
const { createCharRevealWriter } = await import(
  `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`
);

function harness() {
  const writes = [];
  const indices = [];
  const target = {
    innerHTML: "",
    insertAdjacentHTML(position, html) {
      assert.equal(position, "beforeend");
      this.innerHTML += html;
      writes.push(html);
    },
  };
  const writer = createCharRevealWriter({
    charHtml: (char, index, _text, animate) => {
      indices.push({ char, index, animate });
      return char;
    },
  });
  return { writer, target, writes, indices };
}

const typing = harness();
const text = "你😀好";
for (let i = 1; i <= text.length; i++) typing.writer.writeFn(typing.target, text.slice(0, i));
assert.equal(typing.target.innerHTML, text);
assert.deepEqual(typing.indices, [
  { char: "你", index: 0, animate: true },
  { char: "😀", index: 1, animate: true },
  { char: "好", index: 3, animate: true },
]);
const previousWrites = typing.writes.length;
typing.writer.writeFn(typing.target, "你😀好 后一句🌟");
assert.equal(typing.target.innerHTML, "你😀好 后一句🌟");
assert.equal(typing.writes.slice(0, previousWrites).join(""), text);
assert.equal(typing.indices.at(-1).index, 8);
assert.equal(typing.indices.at(-1).animate, false);

const instant = harness();
instant.writer.renderInstant(instant.target, text);
assert.equal(instant.target.innerHTML, text);
assert.deepEqual(
  instant.indices.map(({ char, index }) => [char, index]),
  [
    ["你", 0],
    ["😀", 1],
    ["好", 3],
  ],
);
assert(instant.indices.every(({ animate }) => !animate));
instant.writer.clear(instant.target);
assert.equal(instant.target.innerHTML, "");
instant.writer.writeFn(instant.target, "下一句");
assert.equal(instant.indices.at(-3).index, 0);
assert.equal(instant.target.innerHTML, "下一句");

console.log("文字演出渲染检查通过：Unicode 下标、逐字、追加、补全、瞬时恢复与清空。");
