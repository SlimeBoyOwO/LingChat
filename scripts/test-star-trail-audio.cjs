const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require("playwright");
const out = process.env.TEST_OUTPUT || path.resolve(".test-output/star-trail-audio");
fs.mkdirSync(out, { recursive: true });
function wav(samples, sampleRate) {
  const result = Buffer.alloc(44 + samples.length * 2);
  result.write("RIFF");
  result.writeUInt32LE(result.length - 8, 4);
  result.write("WAVEfmt ", 8);
  result.writeUInt32LE(16, 16);
  result.writeUInt16LE(1, 20);
  result.writeUInt16LE(1, 22);
  result.writeUInt32LE(sampleRate, 24);
  result.writeUInt32LE(sampleRate * 2, 28);
  result.writeUInt16LE(2, 32);
  result.writeUInt16LE(16, 34);
  result.write("data", 36);
  result.writeUInt32LE(samples.length * 2, 40);
  samples.forEach((sample, index) =>
    result.writeInt16LE(Math.round(Math.max(-1, Math.min(1, sample)) * 32767), 44 + index * 2)
  );
  return result;
}
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH,
  });
  try {
    const page = await browser.newPage();
    await page.goto(
      `${process.env.TEST_ORIGIN || "http://127.0.0.1:1438"}/scripts/star-trail-smoke.html`
    );
    const results = [];
    for (const [level, boss] of [
      [0, false],
      [1, false],
      [2, false],
      [0, true],
    ]) {
      const result = await page.evaluate(
        async ({ level, boss }) => {
          const { TrailAudio } = await import("/src/minigames/star-trail/audio.js");
          const { bgmTheme, scheduleBgmStep } = await import("/src/minigames/star-trail/bgm.js");
          const theme = bgmTheme(level, boss),
            step = 60 / theme.bpm / 4,
            sampleRate = 22050;
          const context = new OfflineAudioContext(
            1,
            Math.ceil((128 * step + 1) * sampleRate),
            sampleRate
          );
          // Reuse the real synthesis graph; only bypass the live playback gate for offline rendering.
          Object.defineProperty(context, "state", { get: () => "running" });
          context.resume = () => Promise.resolve();
          const Original = window.AudioContext;
          window.AudioContext = function () {
            return context;
          };
          const audio = new TrailAudio();
          audio.playing = true;
          try {
            await audio.start(level, boss, audio.generation);
          } finally {
            window.AudioContext = Original;
          }
          audio.setVolume(1);
          for (let tick = 0; tick < 128; tick++)
            scheduleBgmStep(audio, tick, 0.05 + tick * step, level, boss);
          const buffer = await context.startRendering(),
            samples = Array.from(buffer.getChannelData(0));
          return {
            level,
            boss,
            bpm: theme.bpm,
            sampleRate,
            samples,
            peak: samples.reduce((max, value) => Math.max(max, Math.abs(value)), 0),
            rms: Math.sqrt(samples.reduce((sum, value) => sum + value * value, 0) / samples.length),
            liveVoices: audio.voices.size,
          };
        },
        { level, boss }
      );
      assert(result.samples.every(Number.isFinite), "finite PCM");
      assert(result.peak > 0.1 && result.peak < 0.99, `headroom at maximum volume: ${result.peak}`);
      assert(result.rms > 0.015, `audible arrangement: ${result.rms}`);
      assert.equal(result.liveVoices, 0, "oscillators and noise voices are disposed");
      const { samples, ...metrics } = result;
      fs.writeFileSync(
        path.join(out, `kawaii-${boss ? "boss" : `level-${level + 1}`}.wav`),
        wav(samples, result.sampleRate)
      );
      results.push(metrics);
    }
    fs.writeFileSync(path.join(out, "audio-result.json"), JSON.stringify(results, null, 2));
    console.log(
      "PASS: four original kawaii bass arrangements, full-volume PCM headroom, finite audible output and voice cleanup",
      results
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
