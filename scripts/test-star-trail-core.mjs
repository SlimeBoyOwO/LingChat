import assert from "node:assert/strict";
import { Adventure, STEP, overlaps } from "../src/minigames/star-trail/core.js";

const run = (game, frames, input = {}) => {
  for (let i = 0; i < frames; i++) game.step(STEP, input);
};
const fresh = () => {
  const game = new Adventure();
  game.start();
  return game;
};

const movement = fresh();
run(movement, 60, { right: true });
assert(movement.player.x > 135 && movement.player.grounded);
run(movement, 180, { left: true });
assert.equal(movement.player.x, 0);

function apex(release) {
  const game = fresh();
  let top = game.player.y;
  for (let i = 0; i < 110; i++) {
    game.step(STEP, { jump: i < release });
    top = Math.min(top, game.player.y);
  }
  assert(game.player.grounded);
  return top;
}
assert(apex(60) < apex(4) - 35, "holding jump must reach noticeably higher");
const coyote = fresh();
Object.assign(coyote.player, { x: 675, grounded: false, coyote: 0.07 });
coyote.step(STEP, { jump: true });
assert(coyote.player.vy < -400);
const buffered = fresh();
Object.assign(buffered.player, { y: 256, vy: 150, grounded: false });
run(buffered, 15, { jump: true });
assert(buffered.player.vy < -250, "buffered input should jump on landing");

const shooter = fresh();
shooter.player.x = 365;
run(shooter, 70, { fire: true });
assert.equal(shooter.level.enemies[0].hp, 0);
assert.equal(shooter.score, 100 + shooter.crystals * 25);
const stomper = fresh(),
  enemy = stomper.level.enemies[0];
Object.assign(stomper.player, { x: enemy.x, y: enemy.y - 31, vy: 170, grounded: false });
stomper.step(STEP);
assert.equal(enemy.hp, 0);
assert(stomper.player.vy < 0);

const checkpoint = fresh();
checkpoint.player.x = checkpoint.level.checkpoint;
checkpoint.player.hp = 1;
checkpoint.step(STEP);
assert(checkpoint.checkpoint);
assert.equal(checkpoint.player.hp, 5);
const coin = checkpoint.level.coins[0];
Object.assign(checkpoint.player, { x: coin.x, y: coin.y, vy: 0 });
checkpoint.step(STEP);
assert.equal(checkpoint.crystals, 1);
checkpoint.player.y = 450;
checkpoint.step(STEP);
assert.equal(checkpoint.mode, "dead");
checkpoint.retry();
assert.equal(checkpoint.player.x, checkpoint.level.checkpoint);
assert.equal(checkpoint.crystals, 1);
assert(!checkpoint.level.coins.some((item) => item.id === coin.id));
checkpoint.hurt();
const health = checkpoint.player.hp;
checkpoint.hurt();
assert.equal(checkpoint.player.hp, health);
checkpoint.pause();
const paused = JSON.stringify([checkpoint.time, checkpoint.player]);
run(checkpoint, 240, { right: true, fire: true });
assert.equal(JSON.stringify([checkpoint.time, checkpoint.player]), paused);
checkpoint.resume();
assert.equal(checkpoint.mode, "playing");

for (let level = 0; level < 3; level++) {
  const terrain = fresh();
  terrain.loadLevel(level);
  terrain.boss.hp = 0;
  terrain.level.enemies.forEach((item) => (item.hp = 0));
  let jumping = false;
  for (let frame = 0; frame < 8000 && terrain.mode === "playing"; frame++) {
    const p = terrain.player;
    const ahead = { x: p.x + p.w + 17, y: p.y + p.h + 2, w: 2, h: 5 };
    const wall = { x: p.x + p.w + 12, y: p.y + 3, w: 3, h: p.h - 5 };
    if (p.grounded)
      jumping =
        !terrain.level.solids.some((s) => overlaps(ahead, s)) ||
        terrain.level.solids.some((s) => overlaps(wall, s));
    else if (p.vy >= 0) jumping = false;
    terrain.step(STEP, { right: true, jump: jumping, fire: true });
  }
  assert.equal(
    terrain.mode,
    level === 2 ? "won" : "cleared",
    `level ${level + 1} terrain must be traversable; stopped at x=${terrain.player.x}, y=${terrain.player.y}`
  );
}

const campaign = fresh();
for (let level = 0; level < 3; level++) {
  assert.equal(campaign.levelIndex, level);
  campaign.player.x = campaign.level.arena + 30;
  campaign.player.invincible = 100;
  campaign.boss.active = true;
  campaign.boss.phase = "charge";
  campaign.boss.timer = 0;
  run(campaign, level === 0 ? 150 : 20);
  assert(
    campaign.threats.length > 0 || campaign.warnings.length > 0,
    `boss ${level + 1} must attack`
  );
  for (let i = 0; i < campaign.boss.maxHP; i++) {
    const b = campaign.boss;
    campaign.bullets.push({ x: b.x + 20, y: b.y + 15, w: 10, h: 5, vx: 0, life: 1 });
    campaign.step(STEP);
  }
  assert.equal(campaign.boss.hp, 0);
  assert.equal(campaign.threats.length, 0);
  campaign.player.x = campaign.level.width - 105;
  campaign.step(STEP);
  assert.equal(campaign.mode, level === 2 ? "won" : "cleared");
  if (level < 2) campaign.next();
}
assert(campaign.score >= 3000);
console.log(
  "PASS: movement, variable jump, coyote time, jump buffering, shots, stomps, damage immunity, checkpoint retry, collectible persistence, pause, all three traversable maps, three boss attacks and complete campaign progression."
);
