import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { Judge } from "../src/minigames/twilight/core.js";
import { SONGS } from "../src/minigames/twilight/songs.js";
import { usesMobileControls } from "../src/minigames/touch-controls.js";

const song = SONGS.find((song) => song.id === "neon-overdrive"),
  notes = song.makeChart();
assert.equal(SONGS[0].noteCount, 138, "original chart remains available");
assert(notes.length > 400 && song.bpm > SONGS[0].bpm);
assert.deepEqual(notes, song.makeChart(), "deterministic chart");
const lanes = Array.from({ length: 4 }, () => []),
  chords = new Map();
for (const note of notes) {
  assert(Number.isFinite(note.at) && note.at >= song.beat * 4 && note.at < song.duration);
  assert(Number.isInteger(note.lane) && note.lane >= 0 && note.lane < 4);
  if (note.end) assert(note.end > note.at && note.end < song.duration);
  lanes[note.lane].push(note);
  chords.set(note.at, (chords.get(note.at) || 0) + 1);
}
for (const lane of lanes)
  for (let i = 1; i < lane.length; i++)
    assert(
      lane[i].at - (lane[i - 1].end ?? lane[i - 1].at) >= song.beat * 0.62 - 0.00001,
      "hold clearance and playable same-lane spacing"
    );
assert(Math.max(...chords.values()) <= 2, "no more than two simultaneous notes");
assert([...chords.values()].filter((count) => count === 2).length > 30);
assert(notes.filter((note) => note.end).length >= 8);
const judge = new Judge(notes);
const actions = notes
  .flatMap((note) => [
    { t: note.at, lane: note.lane, down: true },
    { t: note.end ?? note.at + 0.02, lane: note.lane, down: false },
  ])
  .sort((a, b) => a.t - b.t || Number(a.down) - Number(b.down));
for (const action of actions)
  action.down ? judge.press(action.lane, action.t) : judge.release(action.lane, action.t);
judge.update(song.duration);
assert.equal(judge.result().perfect, notes.length);
assert.equal(judge.result().accuracy, 1);
assert.equal(judge.maxCombo, notes.length);
const missed = new Judge(notes);
missed.update(song.duration);
assert.equal(missed.result().miss, notes.length);

const started = performance.now(),
  pcm = song.renderPcm(),
  renderMs = performance.now() - started;
assert.equal(pcm.length, Math.ceil(song.duration * 22050));
let peak = 0,
  sum = 0;
for (const sample of pcm) {
  assert(Number.isFinite(sample));
  peak = Math.max(peak, Math.abs(sample));
  sum += sample * sample;
}
assert(peak > 0.2 && peak <= 0.861, "PCM headroom");
assert(Math.sqrt(sum / pcm.length) > 0.025, "audible mix");
for (const section of song.sections) {
  const start = Math.round((4 + section.bar * 4) * song.beat * 22050);
  let energy = 0;
  for (const sample of pcm.subarray(start, start + 22050)) energy += sample * sample;
  assert(energy / 22050 > 0.0001, `non-silent ${section.name}`);
}
for (const [nav, expected] of [
  [{ userAgent: "Windows NT 10.0", platform: "Win32", maxTouchPoints: 10 }, false],
  [{ userAgent: "Macintosh; Intel Mac OS X", platform: "MacIntel", maxTouchPoints: 0 }, false],
  [{ userAgent: "Android 14", platform: "Linux arm", maxTouchPoints: 5 }, true],
  [{ userAgent: "iPhone", platform: "iPhone", maxTouchPoints: 5 }, true],
  [{ userAgent: "Macintosh; Intel Mac OS X", platform: "MacIntel", maxTouchPoints: 5 }, true],
])
  assert.equal(usesMobileControls(nav), expected);

const report = {
  title: song.title,
  bpm: song.bpm,
  duration: song.duration,
  notes: notes.length,
  holds: notes.filter((note) => note.end).length,
  chords: [...chords.values()].filter((count) => count === 2).length,
  peak,
  rms: Math.sqrt(sum / pcm.length),
  renderMs,
};
if (process.env.TEST_OUTPUT) {
  fs.mkdirSync(process.env.TEST_OUTPUT, { recursive: true });
  fs.writeFileSync(
    path.join(process.env.TEST_OUTPUT, "chart-audio.json"),
    JSON.stringify(report, null, 2)
  );
  const wav = Buffer.alloc(44 + pcm.length * 2);
  wav.write("RIFF");
  wav.writeUInt32LE(wav.length - 8, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(22050, 24);
  wav.writeUInt32LE(44100, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36);
  wav.writeUInt32LE(pcm.length * 2, 40);
  pcm.forEach((sample, i) => wav.writeInt16LE(Math.round(sample * 32767), 44 + i * 2));
  fs.writeFileSync(path.join(process.env.TEST_OUTPUT, "neon-overdrive.wav"), wav);
}
console.log(
  "PASS: dense chart full combo, all-miss run, holds/chords, PCM, sections and mobile/desktop detection",
  report
);
