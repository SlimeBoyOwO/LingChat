const pixel = (ctx, x, y, w, h, color) => {
  ctx.fillStyle = color;
  ctx.fillRect(Math.round(x), Math.round(y), Math.ceil(w), Math.ceil(h));
};
const noise = (n) => {
  const v = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return v - Math.floor(v);
};

export function star(ctx, x, y, size, color) {
  pixel(ctx, x - size, y - 1, size * 2 + 1, 3, color);
  pixel(ctx, x - 1, y - size, 3, size * 2 + 1, color);
  pixel(ctx, x - size / 2, y - size / 2, size + 1, size + 1, color);
}

export function hero(ctx, x, y, face, time, moving, firing, scale = 1) {
  ctx.save();
  ctx.translate(Math.round(x + 9), Math.round(y + 30));
  ctx.scale(face * scale, scale);
  const r = (a, b, w, h, color) => pixel(ctx, a, b, w, h, color);
  const foot = moving ? Math.round(Math.sin(time * 19) * 3) : 0;
  // White ears, swept hair, cyan hoodie, tail and a brass star blaster.
  r(-15, -14, 7, 10, "#354e78");
  r(-18, -10, 8, 6, "#d7e9f5");
  r(-20, -11, 5, 4, "#fcf4f0");
  r(-13, -30, 25, 20, "#354e78");
  r(-11, -34, 6, 10, "#354e78");
  r(5, -34, 6, 10, "#354e78");
  r(-10, -33, 4, 8, "#fff3e8");
  r(6, -33, 4, 8, "#fff3e8");
  r(-9, -31, 2, 5, "#e9aebe");
  r(7, -31, 2, 5, "#e9aebe");
  r(-11, -28, 22, 17, "#ecf3ff");
  r(-13, -21, 4, 15, "#c4d4f2");
  r(9, -21, 4, 15, "#c4d4f2");
  r(-8, -24, 16, 12, "#ffdccb");
  r(-10, -28, 20, 5, "#ffffff");
  r(-9, -24, 5, 4, "#ffffff");
  r(1, -25, 3, 5, "#ffffff");
  r(-6, -21, 3, 4, "#277ea1");
  r(4, -21, 3, 4, "#277ea1");
  r(-6, -21, 1, 2, "#fff");
  r(4, -21, 1, 2, "#fff");
  r(-8, -16, 3, 2, "#efacb3");
  r(6, -16, 3, 2, "#efacb3");
  r(0, -15, 2, 1, "#ad697e");
  r(-8, -12, 16, 11, "#246180");
  r(-7, -12, 14, 8, "#62cfd0");
  r(-3, -12, 6, 4, "#c9fff0");
  r(-1, -9, 2, 5, "#328999");
  r(-9, -10, 4, 7, "#79e4d9");
  r(6, -10, 5, 6, "#79e4d9");
  r(9, -7, 4, 3, "#ffdccb");
  r(-6, -2, 5, 4 + foot, "#c9def1");
  r(2, -2, 5, 4 - foot, "#c9def1");
  r(-7, 1 + foot, 7, 3, "#285875");
  r(1, 1 - foot, 8, 3, "#285875");
  r(-6, 1 + foot, 5, 1, "#87ebdf");
  r(2, 1 - foot, 5, 1, "#87ebdf");
  r(11, -10, 12, 6, "#334867");
  r(12, -10, 9, 3, "#f3c76d");
  r(14, -7, 6, 2, "#b98554");
  if (firing) star(ctx, 26, -8, 4, "#fff4ab");
  ctx.restore();
}

function cloud(ctx, x, y, color, size = 1) {
  pixel(ctx, x, y + 7 * size, 48 * size, 9 * size, color);
  pixel(ctx, x + 8 * size, y, 19 * size, 18 * size, color);
  pixel(ctx, x + 27 * size, y + 4 * size, 13 * size, 13 * size, color);
}

export function background(ctx, level, camera, time, width) {
  const r = (x, y, w, h, c) => pixel(ctx, x, y, w, h, c);
  const gradient = ctx.createLinearGradient(0, 0, 0, 310);
  gradient.addColorStop(0, level.sky[0]);
  gradient.addColorStop(1, level.sky[1]);
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, 360);
  const night = level.theme === "night",
    factory = level.theme === "factory";
  for (let i = 0; i < 55; i++) {
    const x =
      (((noise(i + 14) * (width + 40) - camera * 0.045) % (width + 40)) + width + 40) %
      (width + 40);
    const y = noise(i + 59) * 195;
    r(x, y, i % 9 === 0 ? 2 : 1, 1, night ? "#e8d8ffaa" : "#fff5d144");
  }
  const sunX = width * 0.79 - camera * 0.015;
  r(sunX - 28, 50, 57, 38, night ? "#eee5fc" : "#ffd49d");
  r(sunX - 20, 42, 41, 55, night ? "#eee5fc" : "#ffd49d");
  if (night) {
    r(sunX - 9, 38, 45, 44, level.sky[0]);
    r(sunX - 2, 77, 34, 8, level.sky[0]);
  }
  for (let i = -1; i < 5; i++)
    cloud(
      ctx,
      i * 195 - camera * 0.07 + Math.sin(time * 0.06 + i) * 8,
      45 + noise(i + 20) * 83,
      night ? "#74639345" : factory ? "#ffd7aa55" : "#c9fff099",
      0.7 + noise(i + 30)
    );
  for (let layer = 0; layer < 3; layer++) {
    const stride = factory ? 64 : 12,
      parallax = 0.1 + layer * 0.1;
    const left = Math.floor((camera * parallax) / stride) - 1;
    for (let i = left; i < left + width / stride + 3; i++) {
      const x = i * stride - camera * parallax;
      const h = factory
        ? 45 + noise(i + layer * 43) * (75 + layer * 12)
        : 45 + Math.sin(i * 0.16 + layer) * 34 + Math.sin(i * 0.044 + 2) * 42;
      const bottom = 263 + layer * 17;
      const colors = night
        ? ["#34365e", "#46406c", "#514777"]
        : factory
          ? ["#714c69", "#895167", "#6a445d"]
          : ["#41899a", "#387c88", "#2c6776"];
      r(x, bottom - h, stride + 1, h + 90, colors[layer]);
      if (factory) {
        r(x + 7, bottom - h - 12, 9, 14, colors[layer]);
        for (let wy = bottom - h + 15; wy < bottom; wy += 18)
          for (let wx = 10; wx < stride - 8; wx += 17)
            if (noise(i + wx + wy) > 0.45) r(x + wx, wy, 5, 7, "#ffca8266");
      }
    }
  }
  if (!factory) {
    for (let i = Math.floor((camera * 0.4) / 120) - 1; i < (camera * 0.4 + width) / 120 + 1; i++) {
      const x = i * 120 - camera * 0.4,
        h = 50 + noise(i + 50) * 75;
      const color = night ? "#3c315d" : "#254e61";
      r(x + 21, 292 - h, 8, h, color);
      for (let branch = 0; branch < 4; branch++)
        r(x + 21 - branch * 7, 280 - h + branch * 17, 9 + branch * 14, 24, color);
      if (night) {
        r(x + 18, 277 - h, 16, 15, "#a394af");
        r(x + 22, 279 - h, 8, 10, "#ffdda3");
      }
    }
  }
  for (let i = 0; i < 18; i++) {
    const x = (noise(i + 89) * width + time * (i % 2 ? 3 : -4) + width * 100) % width;
    const y = 170 + noise(i + 133) * 155 + Math.sin(time + i) * 8;
    r(x, y, 2, 2, night ? "#c8c1ed77" : factory ? "#ffc28c99" : "#daf5ab99");
  }
}

function drawEnemy(ctx, e, player, time) {
  const r = (x, y, w, h, c) => pixel(ctx, e.x + x, e.y + y, w, h, c);
  if (e.kind === "slime") {
    const bob = Math.sin(time * 7 + e.id) > 0 ? 1 : 0;
    r(1, 9 + bob, 23, 14 - bob, "#213f57");
    r(4, 3 + bob, 17, 17, "#7bdcae");
    r(1, 10 + bob, 23, 9, "#56ad98");
    r(5, 6 + bob, 12, 3, "#c4f5bd");
    r(7, 11, 3, 4, "#243855");
    r(16, 11, 3, 4, "#243855");
  } else if (e.kind === "drone") {
    r(-6, 1, 36, 2, "#c0eaf0");
    r(8, -4, 6, 8, "#32425e");
    r(2, 5, 20, 15, "#293650");
    r(4, 7, 16, 10, "#bbaaeb");
    r(7, 10, 10, 5, "#f69baf");
    r(4, 19, 4, 4, "#ffdaab");
    r(17, 19, 4, 4, "#ffdaab");
  } else {
    r(-2, 17, 29, 6, "#344059");
    r(2, 3, 21, 16, "#b07c74");
    r(4, 5, 17, 3, "#f5c297");
    r(player.x < e.x ? -7 : 17, 9, 14, 6, "#35435b");
    r(9, 9, 6, 6, "#ffd498");
  }
}

function drawBoss(ctx, boss, level) {
  const b = boss;
  if (b.hp <= 0) return;
  const r = (x, y, w, h, color) => pixel(ctx, b.x + x, b.y + y, w, h, b.flash ? "#fff8dd" : color);
  const warning = b.phase === "charge";
  if (level.theme === "coast") {
    r(2, 22, 62, 41, "#233f53");
    r(7, 12, 51, 44, "#73c78e");
    r(0, 34, 64, 20, "#45977a");
    r(3, 5, 60, 17, "#386f66");
    r(12, -3, 9, 21, "#ffd580");
    r(28, -9, 11, 27, "#ffe9a4");
    r(47, -3, 9, 21, "#ffd580");
    r(12, 28, 12, warning ? 3 : 9, "#223751");
    r(43, 28, 12, warning ? 3 : 9, "#223751");
    r(27, 41, 12, 6, "#214759");
    r(6, 56, 19, 8, "#273c51");
    r(44, 56, 19, 8, "#273c51");
  } else if (level.theme === "factory") {
    r(10, 8, 48, 43, "#33334c");
    r(13, 10, 42, 37, "#b97065");
    r(16, 13, 36, 7, "#f5b786");
    r(23, 23, 23, 19, warning ? "#fff4a8" : "#f68a66");
    r(29, 27, 12, 10, "#ffd394");
    r(-8, 17, 18, 31, "#654455");
    r(56, 17, 18, 31, "#654455");
    r(-6, 18, 14, 7, "#db976e");
    r(58, 18, 14, 7, "#db976e");
    r(13, 49, 14, 15, "#342e48");
    r(43, 49, 14, 15, "#342e48");
    r(8, 59, 22, 5, "#c59b96");
    r(40, 59, 22, 5, "#c59b96");
    r(22, -4, 6, 13, "#e4a77f");
    r(44, -4, 6, 13, "#e4a77f");
  } else {
    r(7, 5, 51, 46, "#242749");
    r(11, 8, 43, 41, "#8f82c8");
    r(17, 13, 31, 31, "#3e386a");
    r(6, -9, 9, 26, "#cdc5ef");
    r(51, -9, 9, 26, "#cdc5ef");
    r(0, -14, 9, 11, "#cdc5ef");
    r(57, -14, 9, 11, "#cdc5ef");
    r(21, 21, 10, 4, "#faafcb");
    r(37, 21, 10, 4, "#faafcb");
    star(ctx, b.x + 33, b.y + 36, warning ? 9 : 6, warning ? "#fff4b8" : "#9af3e1");
    r(-9, 27, 14, 24, "#756ca8");
    r(62, 27, 14, 24, "#756ca8");
    r(17, 51, 11, 12, "#b0a5d7");
    r(40, 51, 11, 12, "#b0a5d7");
  }
  if (warning) star(ctx, b.x + 32, b.y - 24, 5, "#fff0ad");
}

export function drawWorld(ctx, game, time, width, particles = []) {
  const { level, player: p } = game,
    camera = Math.round(game.camera);
  ctx.imageSmoothingEnabled = false;
  background(ctx, level, camera, time, width);
  ctx.save();
  ctx.translate(-camera, 0);
  const r = (x, y, w, h, c) => pixel(ctx, x, y, w, h, c);
  for (const solid of level.solids) {
    if (solid.x > camera + width + 30 || solid.x + solid.w < camera - 30) continue;
    r(solid.x, solid.y, solid.w, solid.h, level.land[2]);
    r(solid.x, solid.y, solid.w, 5, level.land[0]);
    r(solid.x, solid.y + 5, solid.w, solid.floor ? 14 : 7, level.land[1]);
    const left = Math.max(solid.x, Math.floor(camera / 16) * 16);
    for (let x = left; x < Math.min(solid.x + solid.w, camera + width + 16); x += 16) {
      r(x + 2, solid.y + 6, 6, 3, level.land[0]);
      if (solid.floor) {
        r(x + 4, solid.y + 24, 8, 4, level.land[1]);
        r(x + 11, solid.y + 45, 4, 7, level.land[1]);
        if (level.theme === "coast" && noise(x) > 0.4) {
          r(x + 2, solid.y - 6, 2, 7, "#9bda92");
          r(x, solid.y - 3, 6, 2, "#9bda92");
        }
      } else {
        r(x + 5, solid.y + 12, 3, 5, level.land[2]);
      }
    }
  }
  // Trail signs, a persistent checkpoint lantern and the exit beacon.
  r(185, 265, 3, 35, "#446778");
  r(174, 258, 32, 14, level.land[2]);
  r(181, 264, 16, 2, "#ffe2a1");
  r(194, 262, 3, 6, "#ffe2a1");
  const flag = level.checkpoint;
  r(flag, 243, 4, 57, "#35516a");
  r(flag - 8, 237, 20, 22, "#35516a");
  r(flag - 5, 240, 14, 15, game.checkpoint ? "#caffb4" : "#dfab72");
  star(ctx, flag + 2, 247, 4, game.checkpoint ? "#ffffff" : "#fff1bd");
  for (const coin of level.coins)
    if (!coin.taken) {
      const size = 2 + Math.abs(Math.cos(time * 3 + coin.x)) * 4;
      r(coin.x + 5 - size, coin.y, size * 2, 12, "#b88264");
      r(coin.x + 6 - size, coin.y + 1, size * 2 - 2, 8, "#ffe3a0");
      r(coin.x + 4, coin.y + 3, 2, 5, "#ffffff");
    }
  for (const heal of level.heals)
    if (!heal.taken) {
      const y = heal.y + Math.sin(time * 3) * 2;
      r(heal.x + 2, y, 11, 14, "#efffe4");
      r(heal.x + 5, y + 3, 5, 8, "#71c6a0");
      r(heal.x + 3, y + 6, 9, 3, "#71c6a0");
    }
  if (game.boss.active && game.boss.hp > 0) {
    for (let y = 155; y < 300; y += 12) r(level.arena, y, 4, 7, "#fcb59c99");
  }
  const exit = level.width - 92;
  r(exit, 215, 6, 85, "#323d5a");
  r(exit - 14, 218, 33, 34, "#323d5a");
  r(exit - 10, 222, 25, 25, game.boss.hp <= 0 ? "#a3f5e0" : "#777695");
  if (game.boss.hp <= 0) star(ctx, exit + 3, 233, 9 + Math.sin(time * 4) * 2, "#ffffd2");
  for (const enemy of level.enemies)
    if (enemy.hp > 0 && enemy.x > camera - 60 && enemy.x < camera + width + 60)
      drawEnemy(ctx, enemy, p, time);
  drawBoss(ctx, game.boss, level);
  for (const bullet of game.bullets) {
    r(bullet.x - Math.sign(bullet.vx) * 8, bullet.y + 1, 16, 3, "#9be8e566");
    r(bullet.x, bullet.y, bullet.w, bullet.h, "#fff1ad");
  }
  for (const threat of game.threats) {
    if (threat.kind === "wave") {
      r(threat.x, threat.y + 6, 24, 9, "#f4b67a");
      r(threat.x + 7, threat.y, 10, 9, "#ffe9a8");
    } else star(ctx, threat.x + 5, threat.y + 5, 5, threat.kind === "star" ? "#f5acd1" : "#ffbda0");
  }
  for (const warning of game.warnings) {
    ctx.globalAlpha = 0.3 + Math.abs(Math.sin(time * 12)) * 0.4;
    r(warning.x - 4, 30, 17, 270, "#e990ad");
    ctx.globalAlpha = 1;
    star(ctx, warning.x + 5, 285, 8, "#fff0bc");
  }
  if (p.invincible <= 0 || Math.floor(time * 15) % 2 === 0)
    hero(ctx, p.x, p.y, p.face, time, Math.abs(p.vx) > 15 && p.grounded, p.shot > 0.13);
  for (const particle of particles) {
    ctx.globalAlpha = Math.max(0, particle.life / particle.max);
    r(particle.x, particle.y, particle.size, particle.size, particle.color);
  }
  ctx.globalAlpha = 1;
  ctx.restore();
}
