import { LEVELS, makeLevel } from "./levels.js";

export const STEP = 1 / 120;
const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
export const overlaps = (a, b) =>
  a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;

/** Deterministic simulation; the renderer and audio consume events without owning game state. */
export class Adventure {
  constructor() {
    this.mode = "title";
    this.levelIndex = 0;
    this.time = 0;
    this.score = 0;
    this.crystals = 0;
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
    body.x += body.vx * dt;
    for (const solid of this.level.solids)
      if (solid.floor && overlaps(body, solid)) {
        if (body.vx > 0) body.x = solid.x - body.w;
        else if (body.vx < 0) body.x = solid.x + solid.w;
        body.vx = 0;
      }
    body.grounded = false;
    const previousBottom = body.y + body.h;
    body.y += body.vy * dt;
    for (const solid of this.level.solids)
      if (
        overlaps(body, solid) &&
        (solid.floor || (body.vy > 0 && previousBottom <= solid.y + 0.1))
      ) {
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
    p.hp = Math.max(0, p.hp - amount);
    p.invincible = 1.25;
    this.emit("hurt");
    if (!p.hp) {
      this.mode = "dead";
      this.emit("dead");
    }
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
    p.shot -= dt;
    p.coyote = p.grounded ? 0.11 : Math.max(0, p.coyote - dt);
    p.jumpBuffer = Math.max(0, p.jumpBuffer - dt);
    if (input.jump && !this.previousJump) p.jumpBuffer = 0.13;
    if (!input.jump && this.previousJump && p.vy < -145) p.vy *= 0.48;
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
      p.shot = 0.19;
      this.bullets.push({
        x: p.x + (p.face > 0 ? p.w : -10),
        y: p.y + 12,
        w: 10,
        h: 5,
        vx: p.face * 570,
        life: 1.05,
      });
      this.emit("shoot", p.x + p.w / 2, p.y + 14);
    }
    p.vy = Math.min(600, p.vy + 1080 * dt);
    const oldBottom = p.y + p.h;
    this.move(p, dt);
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
    for (const coin of this.level.coins)
      if (!coin.taken && overlaps(p, coin)) {
        coin.taken = true;
        this.visited.add(coin.id);
        this.crystals++;
        this.score += 25;
        this.emit("coin", coin.x, coin.y);
      }
    for (const heal of this.level.heals)
      if (!heal.taken && p.hp < 5 && overlaps(p, heal)) {
        heal.taken = true;
        this.visited.add(heal.id);
        p.hp = Math.min(5, p.hp + 2);
        this.emit("heal", heal.x, heal.y);
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
