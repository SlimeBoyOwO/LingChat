import { ARMOR_TIERS, LEVELS, makeLevel } from "./levels.js";

export const STEP = 1 / 120;
const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
export const overlaps = (a, b) =>
  a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
const touchesPickup = (player, pickup) =>
  overlaps(player, { x: pickup.x - 3, y: pickup.y - 3, w: pickup.w + 6, h: pickup.h + 6 });

/** Deterministic simulation; the renderer and audio consume events without owning game state. */
export class Adventure {
  constructor() {
    this.mode = "title";
    this.levelIndex = 0;
    this.time = 0;
    this.score = 0;
    this.crystals = 0;
    this.wallet = 0;
    this.armorLevel = 0;
    this.deaths = 0;
    this.events = [];
    this.visited = new Set();
    this.loadLevel(0);
  }
  emit(type, x = this.player.x, y = this.player.y, value = 0) {
    this.events.push({ type, x, y, value });
  }
  takeEvents() {
    return this.events.splice(0);
  }
  loadLevel(index, retry = false) {
    this.levelIndex = index;
    this.level = makeLevel(index);
    if (!retry) {
      this.visited.clear();
      this.checkpoint = false;
    }
    this.level.coins = this.level.coins.filter((coin) => !this.visited.has(coin.id));
    this.level.heals = this.level.heals.filter((heal) => !this.visited.has(heal.id));
    this.level.crates.forEach((crate) => {
      if (crate.reward === "heal" && this.visited.has(crate.id)) crate.opened = true;
    });
    this.player = {
      x: retry && this.checkpoint ? this.level.checkpoint : 65,
      y: 270,
      w: 18,
      h: 30,
      vx: 0,
      vy: 0,
      face: 1,
      grounded: true,
      hp: 5,
      invincible: 0,
      coyote: 0,
      jumpBuffer: 0,
      shot: 0,
      shield: 0,
      armor: ARMOR_TIERS[this.armorLevel].capacity,
      rapid: 0,
      magnet: 0,
      springBoost: 0,
    };
    this.bullets = [];
    this.threats = [];
    this.warnings = [];
    this.boss = {
      x: this.level.width - 260,
      y: 236,
      w: 64,
      h: 64,
      hp: this.level.bossHP,
      maxHP: this.level.bossHP,
      active: false,
      phase: "rest",
      timer: 1.3,
      time: 0,
      vy: 0,
      vx: 0,
      flash: 0,
      attack: 0,
      grounded: false,
    };
    this.previousJump = false;
    this.camera = Math.max(0, this.player.x - 110);
  }
  start() {
    this.score = 0;
    this.crystals = 0;
    this.wallet = 0;
    this.armorLevel = 0;
    this.deaths = 0;
    this.time = 0;
    this.events.length = 0;
    this.loadLevel(0);
    this.mode = "playing";
    this.emit("start");
  }
  retry() {
    this.deaths++;
    this.loadLevel(this.levelIndex, true);
    this.mode = "playing";
    this.emit("retry");
  }
  next() {
    if (this.mode !== "cleared") return;
    this.loadLevel(this.levelIndex + 1);
    this.mode = "playing";
    this.emit("start");
  }
  pause() {
    if (this.mode === "playing") {
      this.mode = "paused";
      this.previousJump = true;
    }
  }
  resume() {
    if (this.mode === "paused") this.mode = "playing";
  }
  move(body, dt) {
    const solids = [...this.level.solids, ...this.level.crates.filter((crate) => !crate.opened)];
    body.x += body.vx * dt;
    for (const solid of solids)
      if (solid.floor && overlaps(body, solid)) {
        if (body.vx > 0) body.x = solid.x - body.w;
        else if (body.vx < 0) body.x = solid.x + solid.w;
        body.vx = 0;
      }
    body.grounded = false;
    const previousBottom = body.y + body.h;
    body.y += body.vy * dt;
    for (const solid of solids)
      if (
        overlaps(body, solid) &&
        (solid.floor || (body.vy > 0 && previousBottom <= solid.y + 0.1))
      ) {
        if (solid.crate && body === this.player && body.vy < 0) {
          this.openCrate(solid);
          continue;
        }
        if (body.vy > 0) {
          body.y = solid.y - body.h;
          body.grounded = true;
        } else if (body.vy < 0) body.y = solid.y + solid.h;
        body.vy = 0;
      }
  }
  hurt(amount = 1) {
    const p = this.player;
    if (p.invincible > 0 || this.mode !== "playing") return;
    if (p.shield > 0) {
      p.shield--;
      p.invincible = 0.75;
      this.emit("shield-break");
      return;
    }
    const absorbed = Math.min(p.armor, amount);
    p.armor -= absorbed;
    p.hp = Math.max(0, p.hp - (amount - absorbed));
    p.invincible = 1.25;
    this.emit(absorbed === amount ? "armor-hit" : "hurt");
    if (!p.hp) {
      this.mode = "dead";
      this.emit("dead");
    }
  }
  openCrate(crate) {
    if (crate.opened) return;
    crate.opened = true;
    this.emit("crate", crate.x, crate.y);
    const pickup = { id: crate.id, x: crate.x + 4, y: crate.y - 18, w: 16, h: 16 };
    if (crate.reward === "heal") {
      if (!this.visited.has(crate.id)) this.level.heals.push(pickup);
    } else this.level.powers.push({ ...pickup, kind: crate.reward });
  }
  interactionHint() {
    const p = this.player,
      nearby = (item) =>
        Math.hypot(item.x + item.w / 2 - p.x - p.w / 2, item.y + item.h / 2 - p.y - p.h / 2) < 76;
    if (this.level.crates.some((crate) => !crate.opened && nearby(crate)))
      return "补给箱 · 空格射击或从下方顶开";
    if (this.level.heals.some((heal) => !heal.taken && nearby(heal)))
      return p.hp === 5 ? "药瓶 · 满血拾取奖励 50 分" : "药瓶 · 恢复 2 格生命";
    const power = this.level.powers.find((item) => !item.taken && nearby(item));
    if (power)
      return {
        shield: "护盾 · 抵挡一次伤害，最多储存两次",
        rapid: "连射核心 · 强化射击 12 秒",
        magnet: "磁力星 · 自动吸引附近星晶 10 秒",
      }[power.kind];
    if (this.level.springs.some(nearby)) return "弹簧 · 踩上去高高跃起";
    return "";
  }
  nearShop() {
    const s = this.level.shop,
      p = this.player;
    return Math.hypot(s.x + s.w / 2 - p.x - p.w / 2, s.y + s.h / 2 - p.y - p.h / 2) < 90;
  }
  shopItems() {
    const p = this.player,
      next = ARMOR_TIERS[this.armorLevel + 1],
      capacity = ARMOR_TIERS[this.armorLevel].capacity;
    return [
      {
        id: "heal",
        name: "暖光药瓶",
        detail: "恢复 2 格生命",
        cost: 4,
        blocked: p.hp >= 5 ? "生命已满" : "",
      },
      {
        id: "repair",
        name: "修复护甲",
        detail: `护甲恢复至 ${capacity} 点`,
        cost: 5,
        blocked: p.armor >= capacity ? "护甲完好" : "",
      },
      {
        id: "upgrade",
        name: next?.name || "辉光护甲",
        detail: next ? `装备升级至 ${next.capacity} 点护甲并修复` : "已达最高等级",
        cost: next?.cost || 0,
        blocked: next ? "" : "已满级",
      },
      {
        id: "shield",
        name: "星光护盾",
        detail: "抵挡一次攻击，最多储存两次",
        cost: 6,
        blocked: p.shield >= 2 ? "护盾已满" : "",
      },
      { id: "rapid", name: "连射核心", detail: "强化射速 12 秒", cost: 6, blocked: "" },
      { id: "magnet", name: "磁力星", detail: "吸引附近星晶 10 秒", cost: 5, blocked: "" },
    ].map((item) => ({
      ...item,
      reason:
        item.blocked || (this.wallet < item.cost ? `还差 ${item.cost - this.wallet} 星晶` : ""),
      available: !item.blocked && this.wallet >= item.cost,
    }));
  }
  buy(id) {
    if (this.mode !== "paused") return { ok: false, message: "请先暂停游戏" };
    const item = this.shopItems().find((entry) => entry.id === id);
    if (!item) return { ok: false, message: "未知商品" };
    if (!item.available) return { ok: false, message: item.reason };
    this.wallet -= item.cost;
    if (id === "heal") this.player.hp = Math.min(5, this.player.hp + 2);
    else if (id === "repair") this.player.armor = ARMOR_TIERS[this.armorLevel].capacity;
    else if (id === "upgrade") {
      this.armorLevel++;
      this.player.armor = ARMOR_TIERS[this.armorLevel].capacity;
    } else if (id === "shield") this.player.shield++;
    else if (id === "rapid") this.player.rapid = 12;
    else if (id === "magnet") this.player.magnet = 10;
    this.emit("purchase");
    return {
      ok: true,
      message:
        id === "upgrade" ? `已装备${ARMOR_TIERS[this.armorLevel].name}` : `${item.name}已生效`,
    };
  }
  fireEnemy(x, y, vx, vy, kind = "orb") {
    this.threats.push({
      x,
      y,
      vx,
      vy,
      w: kind === "wave" ? 24 : 10,
      h: kind === "wave" ? 15 : 10,
      life: 5,
      kind,
    });
  }
  step(dt, input = {}) {
    if (this.mode !== "playing") return;
    this.time += dt;
    const p = this.player;
    p.invincible = Math.max(0, p.invincible - dt);
    p.rapid = Math.max(0, p.rapid - dt);
    p.magnet = Math.max(0, p.magnet - dt);
    p.springBoost = Math.max(0, p.springBoost - dt);
    p.shot -= dt;
    p.coyote = p.grounded ? 0.11 : Math.max(0, p.coyote - dt);
    p.jumpBuffer = Math.max(0, p.jumpBuffer - dt);
    if (input.jump && !this.previousJump) p.jumpBuffer = 0.13;
    if (!input.jump && this.previousJump && p.vy < -145 && p.springBoost <= 0) p.vy *= 0.48;
    this.previousJump = !!input.jump;
    const axis = Number(!!input.right) - Number(!!input.left);
    const target = axis * 190;
    p.vx += clamp(target - p.vx, -1300 * dt, 1300 * dt);
    if (axis) p.face = axis;
    if (p.jumpBuffer > 0 && p.coyote > 0) {
      p.vy = -445;
      p.grounded = false;
      p.coyote = 0;
      p.jumpBuffer = 0;
      this.emit("jump");
    }
    if (input.fire && p.shot <= 0) {
      p.shot = p.rapid > 0 ? 0.11 : 0.19;
      this.bullets.push({
        x: p.x + (p.face > 0 ? p.w : -10),
        y: p.y + 12,
        w: 10,
        h: 5,
        vx: p.face * 570,
        life: 1.05,
        powered: p.rapid > 0,
      });
      this.emit("shoot", p.x + p.w / 2, p.y + 14);
    }
    p.vy = Math.min(600, p.vy + 1080 * dt);
    const oldBottom = p.y + p.h;
    this.move(p, dt);
    for (const spring of this.level.springs) {
      spring.cooldown = Math.max(0, spring.cooldown - dt);
      if (spring.cooldown <= 0 && p.vy >= 0 && overlaps(p, spring)) {
        p.y = spring.y - p.h;
        p.vy = -640;
        p.grounded = false;
        p.coyote = 0;
        p.jumpBuffer = 0;
        p.springBoost = 0.3;
        spring.cooldown = 0.4;
        this.emit("spring", spring.x, spring.y);
      }
    }
    p.x = clamp(
      p.x,
      this.boss.active && this.boss.hp > 0 ? this.level.arena : 0,
      this.level.width - p.w
    );
    if (p.y > 410) {
      p.hp = 0;
      this.mode = "dead";
      this.emit("dead");
      return;
    }
    if (!this.checkpoint && p.x >= this.level.checkpoint) {
      this.checkpoint = true;
      p.hp = 5;
      this.emit("checkpoint", this.level.checkpoint, 270);
    }
    for (const coin of this.level.coins) {
      if (coin.taken) continue;
      if (p.magnet > 0) {
        const dx = p.x + p.w / 2 - coin.x - coin.w / 2,
          dy = p.y + p.h / 2 - coin.y - coin.h / 2;
        const distance = Math.hypot(dx, dy);
        if (distance > 1 && distance < 130) {
          const pull = Math.min(distance, dt * 230);
          coin.x += (dx / distance) * pull;
          coin.y += (dy / distance) * pull;
        }
      }
      if (touchesPickup(p, coin)) {
        coin.taken = true;
        this.visited.add(coin.id);
        this.crystals++;
        this.wallet++;
        this.score += 25;
        this.emit("coin", coin.x, coin.y);
      }
    }
    for (const heal of this.level.heals)
      if (!heal.taken && touchesPickup(p, heal)) {
        heal.taken = true;
        this.visited.add(heal.id);
        const recovered = Math.min(2, 5 - p.hp);
        p.hp = Math.min(5, p.hp + 2);
        if (!recovered) this.score += 50;
        this.emit("heal", heal.x, heal.y, recovered);
      }
    for (const power of this.level.powers)
      if (!power.taken && touchesPickup(p, power)) {
        power.taken = true;
        if (power.kind === "shield") p.shield = Math.min(2, p.shield + 1);
        else if (power.kind === "rapid") p.rapid = 12;
        else if (power.kind === "magnet") p.magnet = 10;
        this.emit(power.kind, power.x, power.y);
      }
    for (const enemy of this.level.enemies) {
      if (enemy.hp <= 0 || Math.abs(enemy.x - p.x) > 680) continue;
      enemy.time += dt;
      enemy.cooldown -= dt;
      if (enemy.kind === "drone") {
        enemy.x = enemy.origin + Math.sin(enemy.time * 1.1) * 65;
        enemy.y = 208 + Math.sin(enemy.time * 2) * 24;
      } else if (enemy.kind === "slime") {
        if (enemy.grounded) {
          const ahead = {
            x: enemy.x + (enemy.face > 0 ? enemy.w + 4 : -5),
            y: enemy.y + enemy.h + 2,
            w: 2,
            h: 5,
          };
          if (!this.level.solids.some((solid) => overlaps(ahead, solid))) enemy.face *= -1;
        }
        enemy.vx = enemy.face * 39;
        enemy.vy = Math.min(500, enemy.vy + 1080 * dt);
        this.move(enemy, dt);
        if (!enemy.vx) enemy.face *= -1;
      }
      if (enemy.kind !== "slime" && enemy.cooldown <= 0 && Math.abs(enemy.x - p.x) < 430) {
        const dx = p.x - enemy.x,
          dy = p.y + 10 - enemy.y,
          length = Math.hypot(dx, dy) || 1;
        this.fireEnemy(enemy.x + 10, enemy.y + 8, (dx / length) * 125, (dy / length) * 125);
        enemy.cooldown = enemy.kind === "turret" ? 1.8 : 2.5;
      }
      if (overlaps(p, enemy)) {
        if (p.vy > 0 && oldBottom <= enemy.y + 9) {
          enemy.hp = 0;
          p.vy = -320;
          this.score += 100;
          this.emit("burst", enemy.x, enemy.y);
        } else this.hurt();
      }
    }
    if (!this.boss.active && p.x >= this.level.arena + 20) {
      this.boss.active = true;
      this.emit("boss");
    }
    this.updateBoss(dt);
    for (const bullet of this.bullets) {
      bullet.x += bullet.vx * dt;
      bullet.life -= dt;
      if (this.level.solids.some((solid) => overlaps(bullet, solid))) bullet.life = 0;
      if (bullet.life <= 0) continue;
      const crate = this.level.crates.find((item) => !item.opened && overlaps(bullet, item));
      if (crate) {
        this.openCrate(crate);
        bullet.life = 0;
        continue;
      }
      const enemy = this.level.enemies.find((e) => e.hp > 0 && overlaps(bullet, e));
      if (enemy) {
        enemy.hp--;
        bullet.life = 0;
        this.emit("hit", enemy.x, enemy.y);
        if (!enemy.hp) {
          this.score += 100;
          this.emit("burst", enemy.x, enemy.y);
        }
      } else if (this.boss.active && this.boss.hp > 0 && overlaps(bullet, this.boss)) {
        this.boss.hp--;
        this.boss.flash = 0.09;
        bullet.life = 0;
        this.emit("hit", bullet.x, bullet.y);
        if (this.boss.hp <= 0) {
          this.threats.length = 0;
          this.warnings.length = 0;
          this.score += 1000;
          p.hp = 5;
          this.emit("boss-down", this.boss.x, this.boss.y);
        }
      }
    }
    this.bullets = this.bullets.filter((bullet) => bullet.life > 0);
    for (const warning of this.warnings) {
      warning.timer -= dt;
      if (warning.timer <= 0 && !warning.fired) {
        warning.fired = true;
        this.fireEnemy(warning.x, -15, 0, 235, "star");
      }
    }
    this.warnings = this.warnings.filter((warning) => !warning.fired);
    for (const threat of this.threats) {
      threat.x += threat.vx * dt;
      threat.y += threat.vy * dt;
      threat.life -= dt;
      if (overlaps(p, threat)) {
        this.hurt();
        threat.life = 0;
      }
      if (threat.kind !== "wave" && this.level.solids.some((solid) => overlaps(threat, solid)))
        threat.life = 0;
    }
    this.threats = this.threats.filter((threat) => threat.life > 0);
    if (this.mode === "playing" && this.boss.hp <= 0 && p.x > this.level.width - 110) {
      this.mode = this.levelIndex === LEVELS.length - 1 ? "won" : "cleared";
      this.emit("clear");
    }
  }
  updateBoss(dt) {
    const b = this.boss,
      p = this.player;
    if (!b.active || b.hp <= 0) return;
    b.time += dt;
    b.timer -= dt;
    b.flash = Math.max(0, b.flash - dt);
    const rage = b.hp <= b.maxHP / 2;
    if (this.levelIndex === 0) {
      if (b.phase === "rest" && b.timer <= 0) {
        b.phase = "charge";
        b.timer = 0.75;
      } else if (b.phase === "charge" && b.timer <= 0) {
        b.phase = "leap";
        b.vy = -365;
        b.vx = Math.sign(p.x - b.x) * (rage ? 180 : 130);
      }
      if (b.phase === "leap") {
        b.vy += 800 * dt;
        this.move(b, dt);
        b.x = clamp(b.x, this.level.arena + 8, this.level.width - 80);
        if (b.grounded) {
          b.phase = "rest";
          b.timer = rage ? 0.65 : 1.2;
          b.vx = 0;
          this.fireEnemy(b.x, 285, -155, 0, "wave");
          this.fireEnemy(b.x + b.w, 285, 155, 0, "wave");
          this.emit("slam", b.x, 300);
        }
      }
    } else if (this.levelIndex === 1) {
      if (b.phase === "rest" && b.timer <= 0) {
        b.phase = "charge";
        b.timer = 0.7;
      } else if (b.phase === "charge" && b.timer <= 0) {
        b.attack++;
        b.phase = "rest";
        b.timer = rage ? 0.8 : 1.3;
        const face = Math.sign(p.x - b.x) || -1;
        for (const vy of rage ? [-95, -40, 15, 70] : [-65, 0, 65])
          this.fireEnemy(b.x + 28, b.y + 28, face * 165, vy);
        if (b.attack % 2 === 0) {
          b.vx = face * 185;
          b.phase = "dash";
          b.timer = 0.9;
        }
        this.emit("boss-shot", b.x, b.y);
      } else if (b.phase === "dash") {
        b.x = clamp(b.x + b.vx * dt, this.level.arena + 25, this.level.width - 100);
        if (b.timer <= 0) {
          b.phase = "rest";
          b.timer = 1.1;
        }
      }
    } else {
      b.x = this.level.width - 325 + Math.sin(b.time * 0.65) * 125;
      b.y = 203 + Math.sin(b.time * 1.4) * 28;
      if (b.phase === "rest" && b.timer <= 0) {
        b.phase = "charge";
        b.timer = 0.8;
      } else if (b.phase === "charge" && b.timer <= 0) {
        b.phase = "rest";
        b.timer = rage ? 1 : 1.65;
        const dx = p.x - b.x,
          dy = p.y - b.y,
          angle = Math.atan2(dy, dx);
        for (const spread of [-0.3, 0, 0.3])
          this.fireEnemy(
            b.x + 25,
            b.y + 35,
            Math.cos(angle + spread) * 150,
            Math.sin(angle + spread) * 150,
            "star"
          );
        this.warnings.push({ x: p.x, timer: 0.9, fired: false });
        if (rage)
          this.warnings.push({
            x: clamp(p.x + p.vx * 0.65, this.level.arena, this.level.width - 50),
            timer: 1.2,
            fired: false,
          });
        this.emit("boss-shot", b.x, b.y);
      }
    }
    if (overlaps(p, b)) this.hurt();
  }
}
