import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { SONGS } from "../src/minigames/twilight/songs.js";
import { frequency, progression, electroPhrases } from "../src/minigames/twilight/harmony.js";

const key = new Set([0, 2, 4, 5, 7, 9, 11]); // A natural minor: A B C D E F G.
assert.equal(frequency(69), 440);
assert.equal(frequency(81), 880);
for (const [index, phrase] of electroPhrases.entries()) {
  const chord = new Set(progression[index].notes.map((note) => note % 12));
  phrase.forEach((note, eighth) => {
    assert(key.has(note % 12), `diatonic melody: ${note}`);
    if (eighth % 2 === 0) assert(chord.has(note % 12), "melody downbeats belong to the chord");
  });
}

const report = [];
for (const song of SONGS) {
  const pitched = song.makeScore().filter((event) => !event.kind);
  for (const event of pitched) {
    assert(Number.isInteger(event.midi) && event.midi >= 0 && event.midi <= 127);
    assert(key.has(event.midi % 12), `${song.title}: unexpected accidental ${event.midi}`);
    assert(event.at >= 0 && event.length > 0 && event.at + event.length < song.duration);
  }
  const ending = song.duration - (song.neon ? 1.8 : 1.2);
  assert.deepEqual(
    [
      ...new Set(
        pitched.filter((event) => event.at >= ending - 0.001).map((event) => event.midi % 12)
      ),
    ].sort((a, b) => a - b),
    [0, 4, 9],
    "final cadence resolves to A minor"
  );
  const pcm = song.renderPcm();
  let peak = 0,
    dc = 0,
    energy = 0;
  for (const sample of pcm) {
    assert(Number.isFinite(sample));
    peak = Math.max(peak, Math.abs(sample));
    dc += sample;
    energy += sample * sample;
  }
  assert(peak > 0.2 && peak <= 0.86, "audible output with headroom");
  assert(Math.abs(dc / pcm.length) < 0.001, "no DC offset");
  assert(
    pcm.subarray(-1102).every((sample) => sample === 0),
    "clean silent ending"
  );
  report.push({
    song: song.title,
    tones: pitched.length,
    peak,
    rms: Math.sqrt(energy / pcm.length),
  });
  if (process.env.TEST_OUTPUT) {
    fs.mkdirSync(process.env.TEST_OUTPUT, { recursive: true });
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
    fs.writeFileSync(path.join(process.env.TEST_OUTPUT, `${song.id}.wav`), wav);
  }
}
console.log(
  "PASS: A440 tuning, diatonic score, chord-tone downbeats, tonic cadence and PCM",
  report
);
