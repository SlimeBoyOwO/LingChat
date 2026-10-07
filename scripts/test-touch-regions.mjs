import assert from "node:assert/strict";
import {
  hitRegion,
  hitRegions,
  touchRegionMessage,
  parseBodyPart,
  serializeBodyPart,
} from "../src/components/game/standard/touch-regions.ts";
const square = (x, y, size) => [
  [x, y],
  [x + size, y],
  [x + size, y + size],
  [x, y + size],
];
const parts = {
  head: { message: "摸头", polygons: [square(0.1, 0.1, 0.5), square(0.2, 0.2, 0.2)] },
  body: { message: "", polygons: [square(0.3, 0.3, 0.5)] },
};
const original = JSON.stringify(parts);
assert.deepEqual(hitRegions(parts, 0.4, 0.4), ["head", "body"]);
assert.deepEqual(hitRegions(parts, 0.25, 0.25), ["head"]); // 同一部位多个多边形不重复。
assert.deepEqual(hitRegions(parts, 0.9, 0.9), []);
for (let x = 0; x <= 1; x += 0.05) {
  for (let y = 0; y <= 1; y += 0.05) {
    assert.equal(hitRegions(parts, x, y)[0] ?? null, hitRegion(parts, x, y));
  }
}
assert.equal(touchRegionMessage(parts.head.message, "玩家"), "摸头");
assert.equal(touchRegionMessage(parts.body.message, "玩家"), "玩家戳了一下你");
assert.equal(JSON.stringify(parts), original); // 测试命中不会更改待保存的配置。
const serialized = serializeBodyPart({ 默认: parts });
const parsed = parseBodyPart(serialized, null).costumes;
assert.deepEqual(hitRegions(parsed.默认, 0.4, 0.4), ["head", "body"]);
assert.equal(hitRegion({ body: parts.body, head: parts.head }, 0.4, 0.4), "body");
console.log(
  "touch regions: overlap order, runtime parity, fallback and immutable round-trip passed",
);
