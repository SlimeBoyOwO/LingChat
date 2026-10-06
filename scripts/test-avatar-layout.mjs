import assert from "node:assert/strict";
import { avatarObjectFit, standardAvatarStyle } from "../src/utils/avatar-layout.ts";

const role = { scale: 1, offsetX: 24, offsetY: 12 };
assert.deepEqual(standardAvatarStyle(role, { width: 1280, height: 720 }), {
  left: "calc(50% + 24px)",
  top: "12px",
  transform: "translateX(-50%) scale(1)",
});
// 竖屏补偿与游戏一致：最多上移视口高度的 20%。
assert.equal(standardAvatarStyle(role, { width: 720, height: 1280 }).top, "-212px");
assert.equal(avatarObjectFit(720 / 1280), "auto 83%");
// 投屏下移不得把缩小后的人物脚底推到视口外，角色自身偏移保留。
assert.equal(
  standardAvatarStyle({ ...role, scale: 0.5 }, { width: 1280, height: 720 }, 25, 1, 500).top,
  "360px",
);
assert.equal(standardAvatarStyle(role, { width: 1280, height: 720 }, 50, 1, -30).top, "-18px");
assert.equal(avatarObjectFit(16 / 9), "contain");
console.log("avatar layout: 6 checks passed");
