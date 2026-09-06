import { bindTouchControls, usesMobileControls } from "../touch-controls.js";
import { SONGS } from "./songs.js";
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
  let W = 960,
    H = 540,
    LINE = 449,
    TRACK = { x: 548, w: 348, top: 77, bottom: 500 };
  let portrait = false,
    touchLayout = false,
    touchControls;
  let music = SONGS[0];
  const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
  let beatEffects = true,
    renderWorker = null,
    cancelRender = null,
    demoActions = [],
    demoIndex = 0;
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
    cancelRender?.();
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
    feedbackUntil = 0;
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
    beatEffects = saved.beatEffects !== false;
    music = SONGS.find((song) => song.id === saved.songId) ?? SONGS[0];
  } catch (_) {}
  function saveSettings() {
    try {
      localStorage.setItem(
        "twilight-cadence-settings",
        JSON.stringify({ volume, offset, approach, horror, beatEffects, songId: music.id })
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
    touchControls?.clear();
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
    if (!audio) {
      const Context = window.AudioContext || window.webkitAudioContext;
      audio = new Context({ latencyHint: "interactive" });
      on(audio, "statechange", () => {
        if (audio.state === "interrupted" || audio.state === "suspended") pauseGame();
      });
    }
    await audio.resume();
    if (destroyed) return;
    if (!gain) {
      gain = audio.createGain();
      gain.connect(audio.destination);
    }
    gain.gain.value = volume;
    if (!buffer) {
      const pcm = await new Promise((resolve, reject) => {
        const worker = new Worker(new URL("./music-worker.js", import.meta.url), {
          type: "module",
        });
        renderWorker = worker;
        const finish = (error, pcm) => {
          worker.terminate();
          if (renderWorker === worker) {
            renderWorker = null;
            cancelRender = null;
          }
          if (error) reject(error);
          else resolve(pcm);
        };
        cancelRender = () => finish(new DOMException("Game closed", "AbortError"));
        worker.onmessage = ({ data }) =>
          finish(data.error ? new Error(data.error) : null, data.pcm);
        worker.onerror = () => finish(new Error("曲目合成失败，请重试"));
        worker.postMessage({ songId: music.id, sampleRate: 22050 });
      });
      if (destroyed) return;
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
  function resetDemoActions(notes) {
    demoActions = notes
      .filter((note) => note.state !== "done")
      .flatMap((note) => [
        { at: note.at, lane: note.lane, down: true },
        { at: note.end ?? note.at + 0.02, lane: note.lane, down: false },
      ])
      .sort((a, b) => a.at - b.at || Number(a.down) - Number(b.down));
    demoIndex = 0;
  }
  async function startGame(watchOnly = false) {
    const generation = ++songGeneration;
    state = "preparing";
    $("start").disabled = true;
    $("start").textContent = "准备节拍…";
    $("demo").disabled = true;
    $("song-prev").disabled = true;
    $("song-next").disabled = true;
    $("footer-status").textContent = "正在准备节拍…";
    try {
      await readyAudio();
      if (destroyed || generation !== songGeneration) return;
      demo = watchOnly;
      runHorror = horror;
      seek = 0;
      lastResult = null;
      const chart = music.makeChart();
      judge = new Judge(chart);
      resetDemoActions(chart);
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
      $("start").textContent = "开始演奏";
      $("demo").disabled = false;
      $("song-prev").disabled = false;
      $("song-next").disabled = false;
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
    lastResult.songId = music.id;
    lastResult.songTitle = music.title;
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
    $("result-title").textContent = runHorror
      ? "最后一拍，没有返回"
      : music.neon
        ? "霓虹熄灭，节拍仍在"
        : "最后一盏灯，为你亮着";
    $("result-song").textContent = `${music.title} · ${music.difficulty} · ${music.noteCount} 音符`;
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
  touchControls = bindTouchControls(root, {
    selector: "[data-lane]",
    enabled: () => state === "playing" && !demo,
    press,
    release,
    signal: lifetime.signal,
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
    $("settings-close").focus({ preventScroll: true });
    root.querySelector(".settings-panel").scrollTop = 0;
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
    $("beat-effects").checked = beatEffects;
    scene.dataset.effects = String(beatEffects && !reducedMotion.matches);
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
  $("beat-effects").onchange = (event) => {
    beatEffects = event.target.checked;
    if (!beatEffects) {
      effects.length = 0;
      particles.length = 0;
    }
    controls();
    saveSettings();
  };
  on(reducedMotion, "change", controls);
  function selectSong(direction = 0) {
    if (!["loading", "idle"].includes(state)) return;
    const next = SONGS[(SONGS.indexOf(music) + direction + SONGS.length) % SONGS.length];
    if (next !== music) {
      music = next;
      buffer = null;
    }
    scene.dataset.song = music.id;
    $("song-title").textContent = music.title;
    const description = `${music.difficulty} · ${music.bpm} BPM · ${Math.round(music.duration)} 秒 · ${music.noteCount} 音符`;
    $("song-details").textContent = description;
    $("track-summary").textContent = `${music.style} · ${music.noteCount} 音符`;
    $("footer-status").textContent = `${music.title} · ${description}`;
    colors.splice(
      0,
      4,
      ...(music.neon
        ? ["#67e9ff", "#adacff", "#ff88ca", "#ffe29b"]
        : ["#f2acb5", "#f7d39a", "#9fd0d4", "#c6b5ea"])
    );
    root
      .querySelectorAll("[data-lane]")
      .forEach((button, lane) => button.style.setProperty("--lane-color", colors[lane]));
    if (direction) saveSettings();
  }
  $("song-prev").onclick = () => selectSong(-1);
  $("song-next").onclick = () => selectSong(1);
  selectSong();
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
      state === "playing" && beatEffects && !reducedMotion.matches
        ? -Math.pow(Math.max(0, Math.sin((t / music.beat) * Math.PI)), 5) * 7
        : beatEffects && !reducedMotion.matches
          ? Math.sin(now / 440) * 2
          : 0;
    const idle = ["idle", "loading", "preparing"].includes(state);
    const x = portrait ? (idle ? W * 0.56 : W - 147) : idle ? 392 : 248,
      y = portrait ? (idle ? H * 0.61 : 105) : 282;
    ctx.save();
    if (portrait || touchLayout) {
      const size = portrait && !idle ? 0.62 : touchLayout && !portrait ? 0.78 : 1;
      const targetY = portrait ? y : H - 210;
      ctx.translate(x, targetY);
      ctx.scale(size, size);
      ctx.translate(-x, -y);
    }
    ctx.translate(0, bounce);
    ctx.fillStyle = "#2d16385c";
    ctx.beginPath();
    ctx.ellipse(x + 74, y + 179, 49, 7, 0, 0, Math.PI * 2);
    ctx.fill();
    if (corrupt > 0.35) {
      ctx.globalAlpha = 0.35;
      ctx.filter = "sepia(1) saturate(7) hue-rotate(310deg)";
      ctx.drawImage(img, x + Math.sin(now / 180) * 10, y - 2, 156, 187);
      ctx.filter = "none";
      ctx.globalAlpha = 1;
    }
    ctx.drawImage(img, x, y, 156, 187);
    if (state === "playing" && !portrait) {
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
    ctx.restore();
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
      if (!touchLayout) text("DFJK"[lane], x + lw / 2, LINE + 35, 16, colors[lane], "center");
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
        if (beatEffects && !reducedMotion.matches)
          for (let i = 0; i < 9; i++)
            particles.push({
              x: TRACK.x + ((event.lane + 0.5) * TRACK.w) / 4,
              y: LINE,
              dx: Math.cos(i * 2.4) * (1 + (i % 3)),
              dy: -1 - (i % 4),
              life: 1,
              color: colors[event.lane],
            });
        if (music.neon && beatEffects && !reducedMotion.matches) {
          effects.push({
            lane: event.lane,
            life: 1,
            combo:
              event.combo > 0 && event.combo % 50 === 0 && event.grade !== "hold" ? event.combo : 0,
          });
        }
      }
    }
    if (particles.length > 240) particles.splice(0, particles.length - 240);
    if (effects.length > 32) effects.splice(0, effects.length - 32);
  }
  function drawNeon(t, idle) {
    if (!music.neon) return;
    const section = music.sectionAt(t),
      moving = beatEffects && !reducedMotion.matches;
    const pulse =
      moving && !idle ? Math.pow((Math.cos((t / music.beat) * Math.PI * 2) + 1) / 2, 3) : 0.1;
    const energy = idle ? 0.25 : section.energy;
    ctx.save();
    ctx.globalAlpha = 0.12 + pulse * energy * 0.14;
    ctx.strokeStyle = "#59d7ff";
    ctx.lineWidth = 2;
    const horizon = H * 0.78;
    for (let i = -4; i <= 4; i++) {
      ctx.beginPath();
      ctx.moveTo(W * 0.5, horizon);
      ctx.lineTo(W * 0.5 + i * W * 0.22, H);
      ctx.stroke();
    }
    for (let i = 0; i < 5; i++) {
      const progress = (i + (moving ? (Math.max(0, t) / music.beat) % 1 : 0)) / 5;
      const y = horizon + progress * progress * (H - horizon);
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(W, y);
      ctx.stroke();
    }
    const beam = ctx.createLinearGradient(0, H, 0, H * 0.15);
    beam.addColorStop(0, "#8d61ff");
    beam.addColorStop(1, "#72e5ff00");
    ctx.fillStyle = beam;
    for (let side = 0; side < 2; side++) {
      const origin = side ? W : 0,
        swing = moving ? Math.sin(t * 0.9 + side * 2) * W * 0.08 : 0;
      ctx.beginPath();
      ctx.moveTo(origin, H);
      ctx.lineTo(W * (side ? 0.85 : 0.15) + swing, 0);
      ctx.lineTo(W * (side ? 0.94 : 0.06) + swing, 0);
      ctx.fill();
    }
    ctx.globalAlpha = 0.3 + energy * 0.25;
    for (let i = 0; i < 16; i++) {
      const value = moving ? (Math.sin(t * 7 + i * 1.8) + 1) / 2 : 0.25;
      const height = 5 + value * energy * 25;
      rect(12 + (i * (W - 24)) / 16, H - height - 4, (W - 24) / 16 - 5, height, colors[i % 4]);
    }
    ctx.restore();
  }
  function drawHitEffects(dt) {
    for (let i = effects.length - 1; i >= 0; i--) {
      const effect = effects[i];
      if (state === "playing") effect.life -= dt * 0.035;
      if (effect.life <= 0) {
        effects.splice(i, 1);
        continue;
      }
      if (!beatEffects || reducedMotion.matches) continue;
      ctx.save();
      ctx.globalAlpha = effect.life * 0.75;
      ctx.strokeStyle = colors[effect.lane];
      ctx.lineWidth = 2;
      const x = TRACK.x + ((effect.lane + 0.5) * TRACK.w) / 4;
      ctx.beginPath();
      ctx.ellipse(
        x,
        LINE,
        (1 - effect.life) * 44 + 6,
        (1 - effect.life) * 18 + 4,
        0,
        0,
        Math.PI * 2
      );
      ctx.stroke();
      if (effect.combo)
        text(
          `${effect.combo} CHAIN`,
          portrait ? W / 2 : TRACK.x / 2,
          portrait ? TRACK.top - 12 : 190,
          21,
          "#82edff",
          "center",
          "bold"
        );
      ctx.restore();
    }
  }
  function frame(now) {
    const dt = Math.min(2, (now - previousFrame) / 16.667);
    previousFrame = now;
    if (state === "countdown" && now >= countdownUntil) {
      judge.resume(resumeAt - offset / 1000);
      if (demo) resetDemoActions(judge.notes);
      playFrom(resumeAt);
    }
    const t = songTime(),
      jt = t - offset / 1000;
    if (state === "playing") {
      if (demo)
        while (demoIndex < demoActions.length && demoActions[demoIndex].at <= jt) {
          const action = demoActions[demoIndex++];
          if (action.down) judge.press(action.lane, action.at);
          else judge.release(action.lane, action.at);
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
    if (corrupt && beatEffects && !reducedMotion.matches) {
      if (Math.sin(now / 640) > 0.86)
        for (let i = 0; i < 5; i++) rect(0, 100 + i * 75, W, 2 + i, "#ef839124");
    }
    for (const petal of petals) {
      if (state !== "paused" && state !== "countdown" && beatEffects && !reducedMotion.matches) {
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
    drawNeon(t, idle);
    drawCharacter(now, Math.max(0, t), corrupt);
    if (!idle) drawTracks(t, now, false);
    drawHitEffects(dt);
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
      const hudWidth = portrait ? W - 50 : TRACK.x - 33;
      rect(25, 20, hudWidth, 2, "#eecbc132");
      rect(25, 20, hudWidth * Math.min(1, t / music.duration), 2, "#f1c997");
      text(music.title, 29, 49, 13);
      text(`${music.bpm} BPM  /  ` + (demo ? "AUTO PLAY" : music.difficulty), 29, 68, 9, "#e0b9c3");
      text(String(result.score).padStart(7, "0"), 29, 108, 27, "#fae2ba");
      const liveAccuracy = judge.resolved ? judge.points / judge.resolved : 1;
      text((liveAccuracy * 100).toFixed(1) + "%", 30, 129, 11, "#e4c3c5");
      if (music.neon) text(music.sectionAt(t).name, 30, 151, 10, "#83eaff");
      if (judge.combo > 1) {
        text(
          judge.combo,
          TRACK.x + TRACK.w / 2,
          TRACK.top + (LINE - TRACK.top) * 0.34,
          39,
          "#fbe4c2",
          "center"
        );
        text(
          "COMBO",
          TRACK.x + TRACK.w / 2,
          TRACK.top + (LINE - TRACK.top) * 0.34 + 22,
          9,
          "#ebc3bd",
          "center"
        );
      }
      if (feedback && now < feedbackUntil) {
        const labels = { perfect: "PERFECT", good: "GOOD", ok: "OK", miss: "MISS", hold: "HOLD" };
        text(
          labels[feedback.grade],
          TRACK.x + TRACK.w / 2,
          LINE - 64,
          17,
          feedback.grade === "miss" ? "#e48b98" : "#ffe3aa",
          "center"
        );
      }
      const time = Math.max(0, music.duration - t);
      text(
        `${Math.floor(time / 60)}:${String(Math.floor(time % 60)).padStart(2, "0")}`,
        portrait ? W - 28 : TRACK.x - 33,
        portrait ? 136 : 47,
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
      const cx = TRACK.x + TRACK.w / 2,
        cy = TRACK.top + (LINE - TRACK.top) * 0.5;
      rect(cx - 82, cy - 45, 165, 86, "#241a33c9");
      text(count, cx, cy + 13, 46, "#ffe6b7", "center");
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
  let previousOrientation;
  const mobileControls = usesMobileControls();
  const syncPlayfield = () => {
    const bounds = canvas.getBoundingClientRect();
    if (!bounds.width || !bounds.height) return;
    const nextOrientation = bounds.width < bounds.height;
    if (previousOrientation !== undefined && previousOrientation !== nextOrientation) pauseGame();
    previousOrientation = nextOrientation;
    portrait = bounds.width / bounds.height < 1.15;
    touchLayout = mobileControls;
    scene.dataset.layout = portrait ? "portrait" : "landscape";
    scene.dataset.touch = String(touchLayout);
    W = portrait ? 480 : 960;
    H = portrait || touchLayout ? Math.round((W * bounds.height) / bounds.width) : 540;
    LINE = H - (portrait ? 100 : touchLayout ? Math.max(100, (56 * W) / bounds.width + 22) : 91);
    TRACK = portrait
      ? { x: 24, w: W - 48, top: Math.min(240, H * 0.29), bottom: H - 12 }
      : touchLayout
        ? { x: 420, w: 516, top: 77, bottom: H - 12 }
        : { x: 548, w: 348, top: 77, bottom: 500 };
    canvas.width = W;
    canvas.height = H;
    const scale = Math.min(bounds.width / W, bounds.height / H);
    const pads = $("touch-keys");
    pads.style.width = `${W * scale}px`;
    pads.style.height = `${H * scale}px`;
    root.querySelectorAll("[data-lane]").forEach((button, lane) => {
      button.style.left = `${((TRACK.x + (lane * TRACK.w) / 4) / W) * 100}%`;
      button.style.width = `${(TRACK.w / 4 / W) * 100}%`;
      button.style.top = `${((LINE + (portrait || touchLayout ? 10 : -30)) / H) * 100}%`;
      button.style.height = `${((portrait || touchLayout ? H - LINE - 22 : 95) / H) * 100}%`;
    });
    $("play-hint").textContent = touchLayout
      ? "点按下方四键 · 长条按住直到尾端 · 支持多指同按"
      : "D / F / J / K · 点按或长按";
  };
  resizeObserver = new ResizeObserver(syncPlayfield);
  resizeObserver.observe(canvas);
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
  on(window, "pagehide", pauseGame);
  return {
    destroy,
    snapshot: () => ({
      state,
      demo,
      songId: music.id,
      songTitle: music.title,
      noteCount: music.noteCount,
      section: music.neon ? music.sectionAt(songTime()).name : undefined,
      effectCount: effects.length,
      renderingAudio: !!renderWorker,
      time: songTime(),
      result: lastResult ?? judge?.result(),
      held: judge ? [...judge.held] : [],
      audioState: audio?.state,
    }),
  };
}
