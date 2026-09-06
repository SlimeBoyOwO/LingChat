import assert from "node:assert/strict";
import { Adventure, STEP } from "../src/minigames/star-trail/core.js";
import { ARMOR_TIERS } from "../src/minigames/star-trail/levels.js";
const run = (g, count, input = {}) => {
  for (let i = 0; i < count; i++) g.step(STEP, input);
};
const fresh = () => {
  const g = new Adventure();
  g.start();
  g.level.coins = [];
  g.level.enemies.forEach((e) => (e.hp = 0));
  return g;
};
for (const health of [3, 4, 5]) {
  const g = fresh(),
    heal = g.level.heals[0];
  Object.assign(g.player, { x: heal.x, y: heal.y - 10, hp: health });
  g.step(STEP);
  assert(heal.taken, `health ${health}: bottle must always be collected`);
  assert.equal(g.player.hp, 5);
  assert.equal(g.score, health === 5 ? 50 : 0);
  assert.equal(g.takeEvents().find((e) => e.type === "heal").value, 5 - health);
  g.retry();
  assert(!g.level.heals.some((h) => h.id === heal.id));
}
for (let level = 0; level < 3; level++) {
  const g = fresh();
  g.loadLevel(level);
  for (const item of [
    ...g.level.heals,
    ...g.level.springs,
    ...g.level.crates,
    ...g.level.powers,
    g.level.shop,
  ]) {
    assert(Number.isFinite(item.y));
    assert(
      g.level.solids.some(
        (s) => s.x <= item.x && s.x + s.w >= item.x + item.w && s.y >= item.y + item.h
      )
    );
  }
}
const crateGame = fresh(),
  crate = crateGame.level.crates[0];
Object.assign(crateGame.player, { x: crate.x - 50, y: crate.y, grounded: false });
run(crateGame, 16, { fire: true });
assert(crate.opened);
assert.equal(crateGame.level.powers.filter((p) => p.id === crate.id).length, 1);
crateGame.openCrate(crate);
assert.equal(crateGame.level.powers.filter((p) => p.id === crate.id).length, 1);
const headGame = fresh(),
  floating = headGame.level.crates.find((c) => c.y < 180);
Object.assign(headGame.player, {
  x: floating.x,
  y: floating.y + floating.h + 1,
  vy: -250,
  grounded: false,
});
headGame.step(STEP);
assert(floating.opened, "jumping into the bottom opens a crate");
const springGame = fresh(),
  spring = springGame.level.springs[0];
Object.assign(springGame.player, { x: spring.x + 2, y: spring.y - 32, vy: 150, grounded: false });
run(springGame, 3);
assert(springGame.player.vy < -600);
assert(spring.cooldown > 0);
springGame.previousJump = true;
springGame.step(STEP, { jump: false });
assert(springGame.player.vy < -580);

const shield = fresh();
for (const kind of ["shield", "rapid", "magnet"]) {
  const pickupGame = fresh();
  pickupGame.level.powers.push({ id: "pickup-test", kind, x: 65, y: 270, w: 16, h: 16 });
  pickupGame.step(STEP);
  assert.equal(pickupGame.player[kind], kind === "shield" ? 1 : kind === "rapid" ? 12 : 10);
  assert(pickupGame.level.powers.at(-1).taken);
}
shield.player.shield = 1;
shield.hurt();
assert.equal(shield.player.shield, 0);
assert.equal(shield.player.hp, 5);
assert.equal(shield.player.armor, 2);
shield.player.invincible = 0;
shield.hurt();
assert.equal(shield.player.armor, 1);
assert.equal(shield.player.hp, 5);
shield.player.invincible = 0;
shield.hurt(3);
assert.equal(shield.player.armor, 0);
assert.equal(shield.player.hp, 3);
shield.hurt(3);
assert.equal(shield.player.hp, 3);
const magnet = fresh();
magnet.player.magnet = 10;
magnet.level.coins.push(
  { id: "magnet-coin", x: 150, y: 276, w: 10, h: 12 },
  { id: "far-coin", x: 230, y: 276, w: 10, h: 12 }
);
run(magnet, 60);
assert(magnet.level.coins[0].taken);
assert(!magnet.level.coins[1].taken);
assert.equal(magnet.wallet, 1);
assert.equal(magnet.crystals, 1);
const shots = (boost) => {
  const g = fresh();
  g.player.rapid = boost;
  run(g, 120, { fire: true });
  return g.takeEvents().filter((e) => e.type === "shoot").length;
};
assert(shots(12) > shots(0));
const timer = fresh();
timer.player.rapid = 12;
timer.player.magnet = 10;
timer.pause();
run(timer, 240);
assert.equal(timer.player.rapid, 12);
assert.equal(timer.player.magnet, 10);
timer.resume();
run(timer, 1441);
assert.equal(timer.player.rapid, 0);
assert.equal(timer.player.magnet, 0);

const shop = fresh();
shop.wallet = 40;
assert(!shop.buy("upgrade").ok);
shop.pause();
assert(!shop.buy("heal").ok);
assert(!shop.buy("repair").ok);
assert.equal(shop.wallet, 40);
assert(shop.buy("upgrade").ok);
assert.equal(shop.armorLevel, 1);
assert.equal(shop.wallet, 30);
assert.equal(shop.player.armor, 4);
assert(shop.buy("upgrade").ok);
assert.equal(shop.armorLevel, 2);
assert.equal(shop.wallet, 12);
assert.equal(shop.player.armor, 6);
assert(!shop.buy("upgrade").ok);
assert.equal(shop.wallet, 12);
shop.player.armor = 1;
assert(shop.buy("repair").ok);
assert.equal(shop.player.armor, 6);
assert.equal(shop.wallet, 7);
assert(shop.buy("shield").ok);
assert.equal(shop.wallet, 1);
assert(!shop.buy("shield").ok);
assert.equal(shop.player.shield, 1);
assert(!shop.buy("invalid").ok);
shop.retry();
assert.equal(shop.armorLevel, 2);
assert.equal(shop.player.armor, ARMOR_TIERS[2].capacity);
assert.equal(shop.wallet, 1);
assert.equal(shop.player.shield, 0);
assert(!shop.level.crates[0].opened, "temporary supplies return on retry");
shop.loadLevel(1);
assert.equal(shop.armorLevel, 2);
assert.equal(shop.wallet, 1);
shop.start();
assert.equal(shop.armorLevel, 0);
assert.equal(shop.wallet, 0);
console.log(
  "PASS: full-health and wounded pickups, pickup persistence and placement, shooting/head-bumping crates, springs, shields, armor absorption, magnet, timed rapid fire, pause freezing, shop validation, exact spending and equipment lifetime."
);
