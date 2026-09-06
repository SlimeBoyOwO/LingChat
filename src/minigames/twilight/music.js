// Original, deterministic study: a 32-bar A-minor chiptune at 112 BPM.
import { progression, studyPhrases, frequency } from "./harmony.js";
const bpm = 112,
  beat = 60 / bpm,
  duration = 132 * beat + 1.2;
export function makeScore() {
  const events = [];
  const tone = (at, length, midi, gain, voice = "pulse") =>
    events.push({ at, length, midi, gain, voice });
  const drum = (at, kind, gain = 1) => events.push({ at, kind, gain });
  for (let i = 0; i < 4; i++) tone(i * beat, 0.07, i === 3 ? 88 : 81, 0.13);
  for (let bar = 0; bar < 32; bar++) {
    const chordIndex = Math.floor(bar / 2) % 4,
      chord = progression[chordIndex];
    for (let b = 0; b < 4; b++) {
      const at = (4 + bar * 4 + b) * beat;
      drum(at, b % 2 ? "hat" : "kick");
      drum(at + beat / 2, "hat");
      tone(at, beat * 0.75, chord.bass, 0.18, "bass");
      tone(at, beat * 0.42, chord.notes[b % 3], 0.095);
      tone(at + beat / 2, beat * 0.42, chord.notes[(b + 1) % 3], 0.075);
      const m = studyPhrases[chordIndex][(bar % 2) * 4 + b];
      tone(at, beat * (b === 3 ? 0.9 : 0.65), m, 0.12);
      if (bar >= 8 && bar % 4 === 3 && b === 2)
        tone(at + beat / 2, beat * 0.4, studyPhrases[chordIndex][7], 0.075);
    }
  }
  const ending = 132 * beat;
  tone(ending, 0.9, 45, 0.14, "bass");
  for (const midi of progression[0].notes) tone(ending, 0.9, midi, 0.055);
  tone(ending, 1, 69, 0.11);
  return events;
}

function renderPcm(sampleRate = 22050) {
  const data = new Float32Array(Math.ceil(duration * sampleRate));
  function tone(at, length, midi, gain, type = "pulse") {
    const start = Math.round(at * sampleRate),
      size = Math.ceil(length * sampleRate),
      f = frequency(midi);
    for (let i = 0; i < size && start + i < data.length; i++) {
      const t = i / sampleRate,
        phase = t * f;
      const env =
        Math.min(1, t / 0.008) *
        Math.min(1, (length - t) / 0.045) *
        Math.exp(-t * (type === "bass" ? 3 : 4));
      const wave =
        type === "pulse"
          ? Math.sin(phase * Math.PI * 2) + 0.22 * Math.sin(phase * Math.PI * 6)
          : Math.sin(phase * Math.PI * 2);
      data[start + i] += wave * env * gain;
    }
  }
  function drum(at, kind) {
    const start = Math.round(at * sampleRate),
      length = kind === "kick" ? 0.16 : 0.08;
    let seed = Math.round(at * 10000) + 17;
    for (let i = 0; i < length * sampleRate && start + i < data.length; i++) {
      const t = i / sampleRate;
      seed = (seed * 1664525 + 1013904223) >>> 0;
      const v =
        kind === "kick"
          ? Math.sin(2 * Math.PI * (65 * t + 35 * 0.02 * (1 - Math.exp(-t / 0.02))))
          : (seed / 4294967296) * 2 - 1;
      data[start + i] +=
        v * Math.exp(-t * (kind === "kick" ? 26 : 60)) * (kind === "kick" ? 0.28 : 0.055);
    }
  }
  for (const event of makeScore()) {
    if (event.kind) drum(event.at, event.kind, event.gain);
    else tone(event.at, event.length, event.midi, event.gain, event.voice);
  }
  let peak = 0;
  for (const v of data) peak = Math.max(peak, Math.abs(v));
  for (let i = 0; i < data.length; i++) data[i] = data[i] / Math.max(peak / 0.78, 1);
  return data;
}
function makeChart() {
  const notes = [],
    pattern = [0, 1, 2, 3, 2, 1, 3, 0, 1, 2, 0, 3, 1, 3, 2, 0];
  for (let i = 0; i < 128; i++) {
    const note = { at: (4 + i) * beat, lane: pattern[i % 16] };
    if (i % 16 === 10) note.end = note.at + beat * 1.5;
    notes.push(note);
    if (i >= 32 && i % 16 === 6) notes.push({ at: note.at + beat / 2, lane: (note.lane + 2) % 4 });
    if (i >= 64 && i % 16 === 15) notes.push({ at: note.at, lane: (note.lane + 2) % 4 });
  }
  return notes.sort((a, b) => a.at - b.at || a.lane - b.lane);
}
export { bpm, beat, duration, makeChart, renderPcm };
