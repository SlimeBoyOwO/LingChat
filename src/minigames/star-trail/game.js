import { Adventure, STEP } from "./core.js";
import { TrailAudio } from "./audio.js";
import { ARMOR_TIERS } from "./levels.js";
import { drawWorld, background, hero } from "./render.js";

export function mountStarTrail(root, options) {
  const $ = (id) => root.querySelector(`#trail-${id}`);
  const canvas = $("canvas"),
    ctx = canvas.getContext("2d"),
    scene = root.querySelector(".star-trail");
  const game = new Adventure(),
    audio = new TrailAudio(),
    lifetime = new AbortController();
  const keys = new Set(),
    pointers = new Map(),
    particles = [];
  const keyActions = {
    ArrowLeft: "left",
    KeyA: "left",
    ArrowRight: "right",
    KeyD: "right",
    ArrowUp: "jump",
    KeyW: "jump",
    KeyZ: "jump",
    Space: "fire",
  };
  let destroyed = false,
    frameId = 0,
    previous = performance.now(),
    accumulator = 0,
    lastMode = "",
    lastHUD = "",
    width = 640,
    announcement = 0;
  const show = (id, visible) => {
    $(id).hidden = !visible;
  };
  const on = (target, name, handler) =>
    target.addEventListener(name, handler, { signal: lifetime.signal });
  const clearInput = () => {
    keys.clear();
    pointers.clear();
    root.querySelectorAll("[data-action]").forEach((button) => button.classList.remove("active"));
  };
  const input = () => {
    const active = [...keys].map((key) => keyActions[key]).concat([...pointers.values()]);
    return Object.fromEntries(active.map((action) => [action, true]));
  };
  const notify = (message, seconds = 2.4) => {
    $("announcement").textContent = message;
    $("status").textContent = message;
    announcement = seconds;
    show("announcement", true);
  };
  let shopReturnToGear = false;
  function renderShop(message = "") {
    $("wallet").textContent = `星晶 ${game.wallet}`;
    $("shop-items").innerHTML = game
      .shopItems()
      .map(
        (item) =>
          `<button class="shop-item" data-buy="${item.id}" ${item.available ? "" : "disabled"}><span><strong>${item.name}</strong><small>${item.detail}</small></span><span class="shop-price">${item.cost} ✦<small>${item.reason || "购买"}</small></span></button>`
      )
      .join("");
    $("shop-message").textContent = message;
  }
  function renderGear() {
    const tier = ARMOR_TIERS[game.armorLevel];
    $("gear-name").textContent = tier.name;
    $("gear-value").textContent =
      `当前护甲 ${game.player.armor} / ${tier.capacity}　星晶 ${game.wallet}`;
    $("gear-bar").innerHTML = Array.from(
      { length: tier.capacity },
      (_, i) => `<span class="armor-cell${i < game.player.armor ? " full" : ""}"></span>`
    ).join("");
    $("gear-tiers").innerHTML = ARMOR_TIERS.map(
      (entry, i) =>
        `<div class="${i === game.armorLevel ? "equipped" : ""}"><strong>${entry.name}</strong><span>${entry.capacity} 点防护 · ${i === game.armorLevel ? "已装备" : i < game.armorLevel ? "已解锁" : `${entry.cost} 星晶`}</span></div>`
    ).join("");
  }
  function openShop(fromGear = false) {
    if (game.mode === "playing") pause();
    if (game.mode !== "paused") return;
    shopReturnToGear = fromGear;
    show("overlay", false);
    show("gear-screen", false);
    show("shop-screen", true);
    renderShop();
    $("shop-close").focus();
  }
  function closeShop() {
    show("shop-screen", false);
    if (shopReturnToGear) openGear();
    else {
      show("overlay", true);
      $("primary").focus();
    }
    shopReturnToGear = false;
  }
  function openGear() {
    if (game.mode === "playing") pause();
    if (game.mode !== "paused") return;
    show("overlay", false);
    show("shop-screen", false);
    show("gear-screen", true);
    renderGear();
    $("gear-close").focus();
  }
  function closeGear() {
    show("gear-screen", false);
    show("overlay", true);
    $("primary").focus();
  }
  function start() {
    clearInput();
    game.start();
    particles.length = 0;
    audio.play(0, false);
    notify(`01  ${game.level.name}`);
  }
  function resume() {
    clearInput();
    root.activeElement?.blur();
    game.resume();
    audio.play(game.levelIndex, game.boss.active && game.boss.hp > 0);
  }
  function pause() {
    if (game.mode !== "playing") return;
    clearInput();
    game.pause();
    audio.pause();
  }
  function retry() {
    clearInput();
    game.retry();
    particles.length = 0;
    audio.play(game.levelIndex, false);
    notify(game.checkpoint ? "从星灯检查点重新出发" : "重新出发");
  }
  function toTitle() {
    clearInput();
    game.mode = "title";
    audio.pause();
    announcement = 0;
    show("announcement", false);
    show("help-screen", false);
  }
  function exit() {
    destroy();
    options.onExit();
  }
  function updateUI() {
    const mode = game.mode;
    if (lastMode !== mode) {
      scene.dataset.mode = mode;
      lastMode = mode;
      show("title", mode === "title");
      show("hud", mode !== "title");
      show("touch", mode === "playing");
      show(
        "overlay",
        ["paused", "dead", "cleared", "won"].includes(mode) &&
          $("shop-screen").hidden &&
          $("gear-screen").hidden
      );
      if (mode !== "paused") {
        show("shop-screen", false);
        show("gear-screen", false);
      }
      if (mode !== "playing") clearInput();
      if (["dead", "cleared", "won"].includes(mode)) audio.pause();
      if (mode === "paused") {
        $("overlay-title").textContent = "暂停";
        $("overlay-copy").textContent = "星灯会在这里等你。";
        $("primary").textContent = "继续远征";
      } else if (mode === "dead") {
        $("overlay-title").textContent = "星火未熄";
        $("overlay-copy").textContent = game.checkpoint
          ? "星灯已经记住你的旅途，从检查点再来一次。"
          : "再试一次，留意脚下的缺口与敌人的攻击提示。";
        $("primary").textContent = "重新出发";
      } else if (mode === "cleared" || mode === "won") {
        $("overlay-title").textContent =
          mode === "won" ? "群星，再次亮起" : `${game.level.name} · 已点亮`;
        $("overlay-copy").textContent =
          `星晶 ${game.crystals}　 得分 ${String(game.score).padStart(6, "0")}　 重试 ${game.deaths} 次`;
        $("primary").textContent = mode === "won" ? "再次远征" : "前往下一关";
      }
      show("retry", mode === "paused");
      show("shop-open", mode === "paused");
      show("gear-open", mode === "paused");
      if (
        ["paused", "dead", "cleared", "won"].includes(mode) &&
        $("shop-screen").hidden &&
        $("gear-screen").hidden
      )
        $("primary").focus();
    }
    const p = game.player,
      tier = ARMOR_TIERS[game.armorLevel];
    const hud = `${game.levelIndex}/${p.hp}/${game.score}/${game.wallet}/${game.boss.hp}/${p.armor}/${game.armorLevel}/${p.shield}/${Math.ceil(p.rapid)}/${Math.ceil(p.magnet)}`;
    if (lastHUD !== hud) {
      lastHUD = hud;
      $("stage").textContent = `0${game.levelIndex + 1} / 03　${game.level.name}`;
      $("hearts").innerHTML = Array.from(
        { length: 5 },
        (_, i) => `<span class="heart${i < game.player.hp ? "" : " empty"}"></span>`
      ).join("");
      $("hearts").setAttribute("aria-label", `生命 ${game.player.hp} / 5`);
      $("score").textContent =
        `✦ ${String(game.wallet).padStart(2, "0")}　${String(game.score).padStart(6, "0")}`;
      $("armor-label").textContent = `护甲 ${p.armor}/${tier.capacity}`;
      $("armor-bar").innerHTML = Array.from(
        { length: tier.capacity },
        (_, i) => `<span class="armor-cell${i < p.armor ? " full" : ""}"></span>`
      ).join("");
      $("armor-bar").setAttribute("aria-valuemax", String(tier.capacity));
      $("armor-bar").setAttribute("aria-valuenow", String(p.armor));
      const buffs = [
        p.shield > 0 ? `护盾 ×${p.shield}` : "",
        p.rapid > 0 ? `连射 ${Math.ceil(p.rapid)}s` : "",
        p.magnet > 0 ? `磁力 ${Math.ceil(p.magnet)}s` : "",
      ]
        .filter(Boolean)
        .join(" · ");
      $("buffs").textContent = buffs;
      show("buffs", !!buffs);
      $("boss-name").textContent =
        game.level.bossName + (game.boss.hp <= game.boss.maxHP / 2 ? " · 狂暴" : "");
      $("boss-health").style.transform = `scaleX(${Math.max(0, game.boss.hp / game.boss.maxHP)})`;
    }
    show("boss", game.boss.active && game.boss.hp > 0 && mode === "playing");
    const hint = mode === "playing" ? game.interactionHint() : "";
    if ($("interaction").textContent !== hint) $("interaction").textContent = hint;
    show("interaction", !!hint && !game.nearShop());
    show("world-shop", mode === "playing" && game.nearShop());
  }
  function effects(dt) {
    for (const event of game.takeEvents()) {
      if (event.type === "boss") {
        audio.setTheme(game.levelIndex, true);
        notify(`${game.level.bossName}　出现了`, 2.2);
      }
      if (event.type === "boss-down") {
        audio.setTheme(game.levelIndex, false);
        notify("守卫已击败 · 前往右侧终点星灯", 4);
      }
      if (event.type === "checkpoint") notify("检查点已点亮 · 生命已恢复");
      if (event.type === "heal")
        notify(event.value > 0 ? `生命恢复 +${event.value}` : "满血奖励 +50 分", 1.6);
      if (event.type === "shield") notify(`护盾已就绪 · 可抵挡 ${game.player.shield} 次攻击`, 1.8);
      if (event.type === "rapid") notify("连射核心 · 强化射击 12 秒", 1.8);
      if (event.type === "magnet") notify("磁力星 · 吸引星晶 10 秒", 1.8);
      if (event.type === "crate") notify("补给箱已打开 · 靠近拾取道具", 1.4);
      audio.effect(event.type);
      if (
        [
          "coin",
          "hit",
          "burst",
          "boss-down",
          "heal",
          "checkpoint",
          "slam",
          "crate",
          "spring",
          "shield",
          "rapid",
          "magnet",
          "armor-hit",
          "shield-break",
        ].includes(event.type)
      ) {
        const count = event.type === "boss-down" ? 44 : event.type === "burst" ? 15 : 7;
        for (let i = 0; i < count; i++) {
          const angle = i * 2.399 + game.time;
          particles.push({
            x: event.x + 9,
            y: event.y + 7,
            vx: Math.cos(angle) * (20 + i * 3),
            vy: -30 + Math.sin(angle) * (25 + i * 2),
            life: 0.5 + (i % 3) * 0.15,
            max: 0.5 + (i % 3) * 0.15,
            size: (i % 3) + 1,
            color: ["#ffedb4", "#a6f2d6", "#ecc8f3"][i % 3],
          });
        }
      }
    }
    if (game.mode === "playing")
      for (const particle of particles) {
        particle.life -= dt;
        particle.x += particle.vx * dt;
        particle.y += particle.vy * dt;
        particle.vy += dt * 100;
      }
    for (let i = particles.length - 1; i >= 0; i--)
      if (particles[i].life <= 0) particles.splice(i, 1);
    if (particles.length > 240) particles.splice(0, particles.length - 240);
  }
  function frame(now) {
    if (destroyed) return;
    const dt = Math.min((now - previous) / 1000, 0.08);
    previous = now;
    if (game.mode === "playing") {
      accumulator += dt;
      const controls = input();
      while (accumulator >= STEP) {
        game.step(STEP, controls);
        accumulator -= STEP;
      }
      announcement -= dt;
      if (announcement <= 0) show("announcement", false);
      const min = game.boss.active && game.boss.hp > 0 ? game.level.arena : 0;
      const max = Math.max(0, game.level.width - width);
      const target = Math.max(Math.min(min, max), Math.min(max, game.player.x - width * 0.36));
      game.camera += (target - game.camera) * Math.min(1, dt * 8);
    } else accumulator = 0;
    effects(dt);
    audio.tick();
    updateUI();
    if (game.mode === "title") {
      background(ctx, game.level, 80, now / 1000, width);
      ctx.fillStyle = game.level.land[2];
      ctx.fillRect(0, 310, width, 50);
      ctx.fillStyle = game.level.land[0];
      ctx.fillRect(0, 310, width, 4);
      ctx.fillStyle = game.level.land[1];
      ctx.fillRect(0, 314, width, 9);
      hero(ctx, width * 0.72, 272, -1, now / 1000, false, false, 2);
    } else drawWorld(ctx, game, game.time, width, particles);
    frameId = requestAnimationFrame(frame);
  }
  const resize = () => {
    const bounds = root.host.getBoundingClientRect();
    // Match the client aspect ratio so generated pixels stay square in every window.
    width = Math.max(120, Math.round((360 * bounds.width) / Math.max(1, bounds.height)));
    canvas.width = width;
    canvas.height = 360;
    ctx.imageSmoothingEnabled = false;
  };
  const observer = new ResizeObserver(resize);
  observer.observe(root.host);
  resize();
  on(window, "keydown", (event) => {
    if (event.code === "Escape") {
      event.preventDefault();
      if (!$("shop-screen").hidden) closeShop();
      else if (!$("gear-screen").hidden) closeGear();
      else if (!$("help-screen").hidden) {
        show("help-screen", false);
        $("help").focus();
      } else if (game.mode === "paused") resume();
      else pause();
    } else if (event.code === "KeyE" && game.mode === "playing" && game.nearShop()) {
      event.preventDefault();
      openShop();
    } else if (
      keyActions[event.code] &&
      game.mode === "playing" &&
      event.composedPath()[0]?.tagName !== "INPUT"
    ) {
      event.preventDefault();
      keys.add(event.code);
    }
  });
  on(window, "keyup", (event) => keys.delete(event.code));
  on(window, "blur", pause);
  on(document, "visibilitychange", () => {
    if (document.hidden) pause();
  });
  root.querySelectorAll("[data-action]").forEach((button) => {
    on(button, "pointerdown", (event) => {
      if (game.mode !== "playing") return;
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      pointers.set(event.pointerId, button.dataset.action);
      button.classList.add("active");
    });
    const release = (event) => {
      pointers.delete(event.pointerId);
      if (![...pointers.values()].includes(button.dataset.action))
        button.classList.remove("active");
    };
    on(button, "pointerup", release);
    on(button, "pointercancel", release);
    on(button, "lostpointercapture", release);
  });
  on($("start"), "click", () => {
    $("start").blur();
    start();
  });
  on($("pause"), "click", pause);
  on($("shop-open"), "click", () => openShop());
  on($("world-shop"), "click", () => openShop());
  on($("gear-open"), "click", openGear);
  on($("armor-open"), "click", openGear);
  on($("shop-close"), "click", closeShop);
  on($("gear-close"), "click", closeGear);
  on($("gear-shop"), "click", () => openShop(true));
  on($("shop-items"), "click", (event) => {
    const button = event.target.closest("[data-buy]");
    if (!button || button.disabled) return;
    const id = button.dataset.buy,
      result = game.buy(id);
    renderShop(result.message);
    renderGear();
    const next = $("shop-items").querySelector(`[data-buy="${id}"]`);
    (next && !next.disabled ? next : $("shop-close")).focus();
  });
  on($("exit"), "click", exit);
  on($("overlay-exit"), "click", exit);
  on($("help"), "click", () => {
    show("help-screen", true);
    $("help-close").focus();
  });
  on($("help-close"), "click", () => {
    show("help-screen", false);
    $("help").focus();
  });
  on($("primary"), "click", () => {
    $("primary").blur();
    if (game.mode === "paused") resume();
    else if (game.mode === "dead") retry();
    else if (game.mode === "cleared") {
      clearInput();
      game.next();
      particles.length = 0;
      audio.play(game.levelIndex, false);
      notify(`0${game.levelIndex + 1}  ${game.level.name}`);
    } else if (game.mode === "won") start();
  });
  on($("retry"), "click", () => {
    $("retry").blur();
    retry();
  });
  on($("title-return"), "click", toTitle);
  on($("volume"), "input", () => {
    const volume = Number($("volume").value);
    audio.setVolume(volume / 100);
    $("volume-value").textContent = `${volume}%`;
  });
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    cancelAnimationFrame(frameId);
    lifetime.abort();
    observer.disconnect();
    clearInput();
    audio.destroy();
    game.mode = "destroyed";
  }
  options.signal.addEventListener("abort", destroy, { once: true, signal: lifetime.signal });
  if (options.signal.aborted) destroy();
  else frameId = requestAnimationFrame(frame);
  return {
    destroy,
    snapshot: () => ({
      mode: game.mode,
      level: game.levelIndex,
      player: { ...game.player },
      bossHP: game.boss.hp,
      score: game.score,
      crystals: game.crystals,
      wallet: game.wallet,
      armorLevel: game.armorLevel,
      interaction: game.interactionHint(),
      audioState: audio.context?.state,
      time: game.time,
    }),
  };
}
