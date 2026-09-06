import * as music from "./music.js";
import { Judge } from "./core.js";
import backgroundUrl from "../../assets/minigames/twilight/shrine-dusk.png";
import pose0Url from "../../assets/minigames/twilight/qinling-0.png";
import pose1Url from "../../assets/minigames/twilight/qinling-1.png";
import pose2Url from "../../assets/minigames/twilight/qinling-2.png";
import pose3Url from "../../assets/minigames/twilight/qinling-3.png";

/** Mount the bundled game inside an isolated UI root; all resources belong to this mount. */
export async function mountRhythm(root, options) {
  "use strict";
  const $ = (id) => root.querySelector("#" + id);
  const canvas = $("game"),
    ctx = canvas.getContext("2d");
  const W = 960,
    H = 540,
    LINE = 449,
    TRACK = { x: 548, w: 348, top: 77, bottom: 500 };
  const colors = ["#f2acb5", "#f7d39a", "#9fd0d4", "#c6b5ea"];
  const keys = ["KeyD", "KeyF", "KeyJ", "KeyK"];
  const lifetime = new AbortController();
  let destroyed = false,
    animationFrame = 0;
  let resizeObserver = null;
  const scene = root.querySelector(".cadence-root");
  $("scene-background").src = backgroundUrl;
  const on = (target, name, callback) =>
    target.addEventListener(name, callback, { signal: lifetime.signal });
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    songGeneration++;
    cancelAnimationFrame(animationFrame);
    resizeObserver?.disconnect();
    lifetime.abort();
    stopSource();
    clearInputs();
    judge?.pause();
    state = "destroyed";
    if (audio && audio.state !== "closed") void audio.close();
  }
  let state = "loading",
    judge = null,
    audio = null,
    buffer = null,
    source = null,
    gain = null;
  let startWhen = 0,
    seek = 0,
    demo = false,
    lastResult = null,
    songGeneration = 0;
  let countdownUntil = 0,
    resumeAt = 0,
    previousFrame = performance.now();
  let volume = 0.55,
    offset = 0,
    approach = 1.8,
    horror = false;
  let poseIndex = 0,
    poseUntil = 0,
    feedback = null,
    feedbackUntil = 0,
    pointerLanes = new Map();
  let runHorror = false;
  const inputSources = Array.from({ length: 4 }, () => new Set());
  const particles = [],
    effects = [],
    laneFlash = [0, 0, 0, 0];
  const petals = Array.from({ length: 30 }, (_, i) => ({
    x: (i * 179) % W,
    y: (i * 79) % H,
    speed: 0.17 + (i % 5) * 0.065,
    phase: i,
  }));
  const images = {};
  try {
    const saved = JSON.parse(localStorage.getItem("twilight-cadence-settings") || "{}");
    volume = Math.max(0, Math.min(1, Number.isFinite(saved.volume) ? saved.volume : 0.55));
    offset = Math.max(-200, Math.min(200, Number.isFinite(saved.offset) ? saved.offset : 0));
    approach = Math.max(1.1, Math.min(2.6, Number.isFinite(saved.approach) ? saved.approach : 1.8));
    horror = saved.horror === true;
  } catch (_) {}
  function saveSettings() {
    try {
      localStorage.setItem(
        "twilight-cadence-settings",
        JSON.stringify({ volume, offset, approach, horror })
      );
    } catch (_) {}
  }
  function show(id, visible) {
    $(id).hidden = !visible;
  }
  function loadImage(name, path) {
    return new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => {
        images[name] = image;
        resolve();
      };
      image.onerror = () => reject(new Error("素材加载失败：" + path));
      image.src = path;
    });
  }
  function outputClock() {
    if (!audio) return 0;
    if (audio.getOutputTimestamp) {
      const stamp = audio.getOutputTimestamp();
      if (stamp.performanceTime > 0 && stamp.contextTime > 0)
        return stamp.contextTime + Math.max(0, performance.now() - stamp.performanceTime) / 1000;
    }
    return audio.currentTime;
  }
  function songTime() {
    if (state !== "playing") return seek;
    return Math.max(-0.2, outputClock() - startWhen + seek);
  }
  function judgedTime() {
    return songTime() - offset / 1000;
  }
  function clearInputs() {
    for (const set of inputSources) set.clear();
    pointerLanes.clear();
    root.querySelectorAll("[data-lane]").forEach((button) => button.classList.remove("active"));
  }
  function stopSource() {
    if (source) {
      source.onended = null;
      try {
        source.stop();
      } catch (_) {}
      source.disconnect();
      source = null;
    }
  }
  async function readyAudio() {
    audio ??= new AudioContext({ latencyHint: "interactive" });
    await audio.resume();
    if (!gain) {
      gain = audio.createGain();
      gain.connect(audio.destination);
    }
    gain.gain.value = volume;
    if (!buffer) {
      const pcm = music.renderPcm();
      buffer = audio.createBuffer(1, pcm.length, 22050);
      buffer.copyToChannel(pcm, 0);
    }
  }
  function playFrom(at) {
    stopSource();
    seek = at;
    startWhen = audio.currentTime + 0.12;
    source = audio.createBufferSource();
    source.buffer = buffer;
    source.connect(gain);
    source.start(startWhen, at);
    state = "playing";
    $("footer-status").textContent = demo
      ? "演示成绩不会记作玩家成绩"
      : "跟着音乐，按下 D / F / J / K";
  }
  async function startGame(watchOnly = false) {
    const generation = ++songGeneration;
    state = "preparing";
    $("start").disabled = true;
    $("demo").disabled = true;
    $("footer-status").textContent = "正在准备节拍…";
    try {
      await readyAudio();
      if (destroyed || generation !== songGeneration) return;
      demo = watchOnly;
      runHorror = horror;
      seek = 0;
      lastResult = null;
      judge = new Judge(music.makeChart());
      clearInputs();
      effects.length = 0;
      particles.length = 0;
      feedback = null;
      show("settings", false);
      $("settings-toggle").setAttribute("aria-expanded", "false");
      show("title-card", false);
      show("pause-screen", false);
      show("result-screen", false);
      show("live-tools", true);
      $("mode-label").textContent = demo ? "观赏演示 · 自动演奏" : "演奏中";
      $("footer-status").textContent = demo
        ? "演示成绩不会记作玩家成绩"
        : "跟着音乐，按下 D / F / J / K";
      $("horror").disabled = true;
      playFrom(0);
      if (document.hidden || !document.hasFocus()) pauseGame();
    } catch (error) {
      if (destroyed) return;
      state = "idle";
      show("title-card", true);
      $("footer-status").textContent = "音乐启动失败，请再次点击开始";
      console.error(error);
    } finally {
      $("start").disabled = false;
      $("demo").disabled = false;
    }
  }
  function pauseGame() {
    if (state !== "playing" && state !== "countdown") return;
    if (state === "playing") seek = Math.min(music.duration, Math.max(0, songTime()));
    stopSource();
    state = "paused";
    judge.pause();
    clearInputs();
    show("pause-screen", true);
    $("footer-status").textContent = "已暂停 · 时间与音符都停在原处";
  }
  async function resumeGame() {
    if (state !== "paused") return;
    try {
      await audio.resume();
    } catch (_) {
      return;
    }
    if (state !== "paused") return;
    show("pause-screen", false);
    state = "countdown";
    resumeAt = seek;
    countdownUntil = performance.now() + 3 * music.beat * 1000;
  }
  function backToTitle() {
    songGeneration++;
    stopSource();
    state = "idle";
    seek = 0;
    judge = null;
    clearInputs();
    effects.length = 0;
    show("settings", false);
    $("settings-toggle").setAttribute("aria-expanded", "false");
    show("title-card", true);
    show("pause-screen", false);
    show("result-screen", false);
    show("live-tools", false);
    $("horror").disabled = false;
    $("footer-status").textContent = "庭院已就绪";
  }
  function finishGame() {
    if (state !== "playing") return;
    judge.update(music.duration + 1);
    stopSource();
    state = "result";
    seek = music.duration;
    clearInputs();
    lastResult = judge.result(runHorror ? "interrupted" : "completed");
    lastResult.demo = demo;
    show("result-screen", true);
    show("live-tools", false);
    $("horror").disabled = false;
    const acc = lastResult.accuracy;
    $("result-grade").textContent = demo
      ? "DEMO"
      : acc >= 0.95
        ? "S"
        : acc >= 0.85
          ? "A"
          : acc >= 0.7
            ? "B"
            : "C";
    $("result-title").textContent = runHorror ? "最后一拍，没有返回" : "最后一盏灯，为你亮着";
    $("result-eyebrow").textContent = runHorror
      ? "SCENE_INDEX_MISSING"
      : demo
        ? "观赏演示 · 不记录玩家成绩"
        : "演奏完成";
    $("result-copy").textContent = demo
      ? "自动演示已结束，试着亲手接住下一次节拍。"
      : runHorror
        ? "剧情演出已中断。此前的真实判定保留，可以重试或返回庭院。"
        : acc >= 0.85
          ? "钦灵记住了你的节拍。再来一次，会更默契。"
          : "每盏灯都愿意再等一拍。调慢音符，或者再练一次。";
    $("result-accuracy").textContent = (acc * 100).toFixed(1) + "%";
    $("result-combo").textContent = lastResult.maxCombo;
    $("result-miss").textContent = lastResult.miss;
    $("footer-status").textContent = demo ? "演示结束 · 不计入玩家成绩" : "演奏结束";
    // Local event only. A future Tauri adapter can translate this result into script variables.
    options.onResult?.({ ...lastResult });
  }
  function hitsound(lane) {
    if (!audio || !gain) return;
    const osc = audio.createOscillator(),
      amp = audio.createGain();
    osc.type = "sine";
    osc.frequency.value = [880, 1046.5, 1174.7, 1318.5][lane];
    amp.gain.setValueAtTime(0.06, audio.currentTime);
    amp.gain.exponentialRampToValueAtTime(0.001, audio.currentTime + 0.065);
    osc.connect(amp);
    amp.connect(gain);
    osc.start();
    osc.stop(audio.currentTime + 0.07);
    osc.onended = () => {
      osc.disconnect();
      amp.disconnect();
    };
  }
  function press(lane, origin) {
    if (state !== "playing" || demo) return;
    const sources = inputSources[lane];
    if (sources.has(origin)) return;
    const wasHeld = sources.size > 0;
    sources.add(origin);
    root.querySelector(`[data-lane="${lane}"]`).classList.add("active");
    if (wasHeld) return;
    judge.press(lane, judgedTime());
    laneFlash[lane] = performance.now();
    hitsound(lane);
    poseIndex = lane < 2 ? 1 : 2;
    poseUntil = performance.now() + 230;
  }
  function release(lane, origin) {
    inputSources[lane].delete(origin);
    if (inputSources[lane].size) return;
    root.querySelector(`[data-lane="${lane}"]`).classList.remove("active");
    if (state === "playing" && !demo) judge.release(lane, judgedTime());
  }
  on(window, "keydown", (event) => {
    if (event.code === "Escape") {
      event.preventDefault();
      if (!$("settings").hidden) closeSettings();
      else if (state === "paused") resumeGame();
      else pauseGame();
      return;
    }
    const target = event.composedPath()[0];
    if (
      target instanceof HTMLInputElement ||
      (target instanceof HTMLButtonElement && event.code === "Space")
    )
      return;
    const lane = keys.indexOf(event.code);
    if (lane >= 0 && state === "playing") {
      event.preventDefault();
      if (!event.repeat) press(lane, event.code);
    }
  });
  on(window, "keyup", (event) => {
    const lane = keys.indexOf(event.code);
    if (lane >= 0) release(lane, event.code);
  });
  on(window, "blur", pauseGame);
  on(document, "visibilitychange", () => {
    if (document.hidden) pauseGame();
  });
  root.querySelectorAll("[data-lane]").forEach((button) => {
    const lane = Number(button.dataset.lane);
    on(button, "pointerdown", (event) => {
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      pointerLanes.set(event.pointerId, lane);
      press(lane, "pointer-" + event.pointerId);
    });
    for (const name of ["pointerup", "pointercancel", "lostpointercapture"])
      on(button, name, (event) => {
        if (pointerLanes.get(event.pointerId) === lane) {
          pointerLanes.delete(event.pointerId);
          release(lane, "pointer-" + event.pointerId);
        }
      });
  });
  $("start").onclick = () => startGame(false);
  $("demo").onclick = () => startGame(true);
  $("pause").onclick = pauseGame;
  $("resume").onclick = resumeGame;
  $("retry").onclick = () => startGame(false);
  $("restart-paused").onclick = () => startGame(demo);
  $("leave").onclick = backToTitle;
  $("result-leave").onclick = backToTitle;
  function openSettings() {
    if (state === "playing" || state === "countdown") pauseGame();
    show("settings", true);
    $("settings-toggle").setAttribute("aria-expanded", "true");
    $("settings-close").focus();
  }
  function closeSettings() {
    show("settings", false);
    $("settings-toggle").setAttribute("aria-expanded", "false");
    (state === "paused" ? $("resume") : $("settings-toggle")).focus();
  }
  $("settings-toggle").onclick = openSettings;
  $("pause-settings").onclick = openSettings;
  $("settings-close").onclick = closeSettings;
  function controls() {
    $("volume").value = Math.round(volume * 100);
    $("volume-value").textContent = Math.round(volume * 100) + "%";
    $("offset").value = offset;
    $("offset-value").textContent = (offset > 0 ? "+" : "") + offset + " ms";
    $("speed").value = approach;
    $("speed-value").textContent = approach.toFixed(1) + " s";
    $("horror").checked = horror;
  }
  $("volume").oninput = (e) => {
    volume = Number(e.target.value) / 100;
    if (gain) gain.gain.setTargetAtTime(volume, audio.currentTime, 0.04);
    controls();
    saveSettings();
  };
  $("offset").oninput = (e) => {
    offset = Number(e.target.value);
    controls();
    saveSettings();
  };
  $("speed").oninput = (e) => {
    approach = Number(e.target.value);
    controls();
    saveSettings();
  };
  $("horror").onchange = (e) => {
    horror = e.target.checked;
    saveSettings();
  };
  controls();
  function text(value, x, y, size = 12, color = "#f9e8d0", align = "left", weight = "normal") {
    ctx.font = `${weight} ${size}px "Cascadia Code","Microsoft YaHei",sans-serif`;
    ctx.textAlign = align;
    ctx.fillStyle = color;
    ctx.fillText(value, Math.round(x), Math.round(y));
  }
  function rect(x, y, w, h, color) {
    ctx.fillStyle = color;
    ctx.fillRect(Math.round(x), Math.round(y), Math.round(w), Math.round(h));
  }
  function drawCharacter(now, t, corrupt) {
    let frame = now < poseUntil ? poseIndex : 0;
    if (judge && judge.combo > 0 && judge.combo % 16 === 0 && now < feedbackUntil) frame = 3;
    const img = images["pose" + frame];
    if (!img) return;
    const bounce =
      state === "playing"
        ? -Math.pow(Math.max(0, Math.sin((t / music.beat) * Math.PI)), 5) * 7
        : Math.sin(now / 440) * 2;
    const x = state === "idle" || state === "loading" ? 392 : 248,
      y = 282 + bounce;
    ctx.fillStyle = "#2d16385c";
    ctx.beginPath();
    ctx.ellipse(x + 74, 461, 49, 7, 0, 0, Math.PI * 2);
    ctx.fill();
    if (corrupt > 0.35) {
      ctx.globalAlpha = 0.35;
      ctx.filter = "sepia(1) saturate(7) hue-rotate(310deg)";
      ctx.drawImage(img, x + Math.sin(now / 180) * 10, y - 2, 156, 187);
      ctx.filter = "none";
      ctx.globalAlpha = 1;
    }
    ctx.drawImage(img, x, y, 156, 187);
    if (state === "playing") {
      const phrase =
        corrupt > 0.55
          ? "你的下一拍，谁在替你按？"
          : judge.combo >= 16
            ? "嗯！就是这个节奏。"
            : "跟着灯光，慢慢来。";
      const width = corrupt > 0.55 ? 222 : 182;
      rect(x + 77 - width / 2, y - 33, width, 29, "#2d213bea");
      rect(x + 71, y - 4, 10, 5, "#2d213bea");
      text(phrase, x + 77, y - 14, 10, "#f4d4c5", "center");
    }
  }
  function drawTracks(t, now, idle) {
    const lw = TRACK.w / 4;
    ctx.save();
    const shade = ctx.createLinearGradient(0, TRACK.top, 0, TRACK.bottom);
    shade.addColorStop(0, "#20172b50");
    shade.addColorStop(0.55, "#20172bc7");
    shade.addColorStop(1, "#20172be8");
    ctx.fillStyle = shade;
    ctx.fillRect(TRACK.x, TRACK.top, TRACK.w, TRACK.bottom - TRACK.top);
    for (let lane = 0; lane < 4; lane++) {
      const x = TRACK.x + lane * lw;
      rect(x, TRACK.top, 1, TRACK.bottom - TRACK.top, "#cfb0c424");
      const strength = Math.max(0, 1 - (now - laneFlash[lane]) / 180);
      if (strength > 0) {
        ctx.globalAlpha = strength * 0.27;
        rect(x + 1, TRACK.top, lw - 2, LINE - TRACK.top, colors[lane]);
        ctx.globalAlpha = 1;
      }
      text("DFJK"[lane], x + lw / 2, 484, 16, colors[lane], "center");
      rect(x + 10, LINE - 2, lw - 20, 4, colors[lane]);
    }
    rect(TRACK.x + TRACK.w, TRACK.top, 1, TRACK.bottom - TRACK.top, "#cfb0c424");
    ctx.beginPath();
    ctx.rect(TRACK.x, TRACK.top, TRACK.w, LINE - TRACK.top + 26);
    ctx.clip();
    const notes = idle
      ? music
          .makeChart()
          .slice(0, 10)
          .map((n, i) => ({ ...n, at: 1 + i * 0.25, end: i === 6 ? 3.2 : undefined }))
      : (judge?.notes ?? []);
    const baseTime = idle ? 0.12 : t;
    for (const note of notes) {
      if (note.state === "done") continue;
      const y = LINE - ((note.at - baseTime) / approach) * (LINE - TRACK.top);
      if (y < TRACK.top - 12 || (y > LINE + 120 && note.end == null)) continue;
      const x = TRACK.x + note.lane * lw + 12,
        width = lw - 24;
      if (note.end != null) {
        const tail = LINE - ((note.end - baseTime) / approach) * (LINE - TRACK.top);
        const head = note.state === "holding" ? LINE : y;
        ctx.globalAlpha = note.state === "holding" ? 0.75 : 0.42;
        rect(x + 20, tail, width - 40, Math.max(0, head - tail), colors[note.lane]);
        ctx.globalAlpha = 1;
        rect(x + 17, tail, width - 34, 4, colors[note.lane]);
        if (note.state === "holding") {
          rect(x, LINE - 6, width, 12, "#fff2cb");
          continue;
        }
      }
      rect(x + 3, y + 3, width, 10, "#120c2380");
      rect(x, y - 5, width, 10, colors[note.lane]);
      rect(x + 3, y - 3, width - 6, 2, "#fff7e591");
    }
    ctx.restore();
    if (idle) text("D  /  F  /  J  /  K", TRACK.x + TRACK.w / 2, 60, 10, "#ead5d2", "center");
  }
  function feedbackEvents(now) {
    for (const event of judge?.events.splice(0) ?? []) {
      feedback = event;
      feedbackUntil = now + 530;
      if (event.grade !== "miss") {
        poseIndex = event.lane < 2 ? 1 : 2;
        poseUntil = now + 240;
        laneFlash[event.lane] = now;
        for (let i = 0; i < 9; i++)
          particles.push({
            x: TRACK.x + ((event.lane + 0.5) * TRACK.w) / 4,
            y: LINE,
            dx: Math.cos(i * 2.4) * (1 + (i % 3)),
            dy: -1 - (i % 4),
            life: 1,
            color: colors[event.lane],
          });
      }
    }
  }
  function frame(now) {
    const dt = Math.min(2, (now - previousFrame) / 16.667);
    previousFrame = now;
    if (state === "countdown" && now >= countdownUntil) {
      judge.resume(resumeAt - offset / 1000);
      playFrom(resumeAt);
    }
    const t = songTime(),
      jt = t - offset / 1000;
    if (state === "playing") {
      if (demo)
        for (const note of judge.notes) {
          if (note.state === "pending" && jt >= note.at) {
            judge.press(note.lane, note.at);
            if (note.end == null) judge.release(note.lane, note.at);
          } else if (note.state === "holding" && jt >= note.end) judge.release(note.lane, note.end);
        }
      judge.update(jt);
      feedbackEvents(now);
      if (t >= music.duration) finishGame();
    }
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, W, H);
    const idle = state === "idle" || state === "loading" || state === "preparing";
    if (scene.dataset.state !== state) scene.dataset.state = state;
    const corrupt =
      runHorror && !idle ? Math.max(0, Math.min(0.85, (t - music.beat * 68) / 28)) : 0;
    $("scene-corruption").style.opacity = String(corrupt * 0.55);
    if (corrupt) {
      if (Math.sin(now / 640) > 0.86)
        for (let i = 0; i < 5; i++) rect(0, 100 + i * 75, W, 2 + i, "#ef839124");
    }
    for (const petal of petals) {
      if (state !== "paused" && state !== "countdown") {
        petal.x += petal.speed * dt;
        petal.y += 0.1 * dt;
        if (petal.x > W) petal.x = -5;
        if (petal.y > H) petal.y = -5;
      }
      rect(
        petal.x,
        petal.y + Math.sin(now / 1200 + petal.phase) * 7,
        3,
        2,
        corrupt > 0.5 ? "#bd364c99" : "#f9bcb6aa"
      );
    }
    drawCharacter(now, Math.max(0, t), corrupt);
    if (!idle) drawTracks(t, now, false);
    for (let i = particles.length - 1; i >= 0; i--) {
      const p = particles[i];
      if (state === "playing") {
        p.x += p.dx * dt;
        p.y += p.dy * dt;
        p.dy += 0.1 * dt;
        p.life -= 0.035 * dt;
      }
      if (p.life <= 0) {
        particles.splice(i, 1);
        continue;
      }
      ctx.globalAlpha = p.life;
      rect(p.x, p.y, 3, 3, p.color);
      ctx.globalAlpha = 1;
    }
    if (!idle && judge) {
      const result = judge.result();
      rect(25, 20, 515, 2, "#eecbc132");
      rect(25, 20, 515 * Math.min(1, t / music.duration), 2, "#f1c997");
      text("灯下回声", 29, 49, 13);
      text("112 BPM  /  " + (demo ? "AUTO PLAY" : "4 KEYS"), 29, 68, 9, "#e0b9c3");
      text(String(result.score).padStart(7, "0"), 29, 108, 27, "#fae2ba");
      const liveAccuracy = judge.resolved ? judge.points / judge.resolved : 1;
      text((liveAccuracy * 100).toFixed(1) + "%", 30, 129, 11, "#e4c3c5");
      if (judge.combo > 1) {
        text(judge.combo, TRACK.x + TRACK.w / 2, 213, 39, "#fbe4c2", "center");
        text("COMBO", TRACK.x + TRACK.w / 2, 235, 9, "#ebc3bd", "center");
      }
      if (feedback && now < feedbackUntil) {
        const labels = { perfect: "PERFECT", good: "GOOD", ok: "OK", miss: "MISS", hold: "HOLD" };
        text(
          labels[feedback.grade],
          TRACK.x + TRACK.w / 2,
          379,
          17,
          feedback.grade === "miss" ? "#e48b98" : "#ffe3aa",
          "center"
        );
      }
      const time = Math.max(0, music.duration - t);
      text(
        `${Math.floor(time / 60)}:${String(Math.floor(time % 60)).padStart(2, "0")}`,
        514,
        47,
        11,
        "#edd0c9",
        "right"
      );
    }
    if (state === "countdown" || (state === "playing" && t < music.beat * 4)) {
      const count =
        state === "countdown"
          ? Math.ceil((countdownUntil - now) / (music.beat * 1000))
          : Math.max(1, 4 - Math.floor(Math.max(0, t) / music.beat));
      rect(641, 225, 165, 86, "#241a33c9");
      text(count, 724, 283, 46, "#ffe6b7", "center");
    }
    if (!destroyed) animationFrame = requestAnimationFrame(frame);
  }
  options.signal.addEventListener("abort", destroy, { once: true, signal: lifetime.signal });
  const exitToGames = () => {
    destroy();
    options.onExit();
  };
  $("back-to-games").onclick = exitToGames;
  $("exit-paused").onclick = exitToGames;
  const syncPlayfield = () => {
    const bounds = canvas.getBoundingClientRect();
    const scale = Math.min(bounds.width / W, bounds.height / H);
    $("touch-keys").style.width = `${W * scale}px`;
    $("touch-keys").style.height = `${H * scale}px`;
  };
  resizeObserver = new ResizeObserver(syncPlayfield);
  resizeObserver.observe(root.host);
  syncPlayfield();
  if (options.signal.aborted) {
    destroy();
    return { destroy, snapshot: () => ({ state }) };
  }
  animationFrame = requestAnimationFrame(frame);
  try {
    await Promise.all([
      loadImage("bg", backgroundUrl),
      ...[pose0Url, pose1Url, pose2Url, pose3Url].map((url, i) => loadImage("pose" + i, url)),
    ]);
    if (!destroyed) {
      state = "idle";
      show("loading", false);
      show("title-card", true);
    }
  } catch (error) {
    if (!destroyed) {
      $("loading").textContent = "素材加载失败，请返回小游戏后重试。";
      $("footer-status").textContent = "素材未就绪";
      console.error(error);
    }
  }
  on(window, "pagehide", destroy);
  return {
    destroy,
    snapshot: () => ({
      state,
      demo,
      time: songTime(),
      result: lastResult ?? judge?.result(),
      held: judge ? [...judge.held] : [],
      audioState: audio?.state,
    }),
  };
}
