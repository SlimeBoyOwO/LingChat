// Original electro / melodic bass track. Audio, chart and lighting share this timeline.
import { progression, electroPhrases, frequency } from "./harmony.js";
export const bpm = 168;
export const beat = 60 / bpm;
export const duration = (4 + 56 * 4) * beat + 1.8;
export const sections = [
  { bar: 0, name: "IGNITION", energy: 0.35 },
  { bar: 8, name: "BUILD UP", energy: 0.65 },
  { bar: 16, name: "DROP 01", energy: 1 },
  { bar: 32, name: "AFTERGLOW", energy: 0.3 },
  { bar: 40, name: "FINAL DROP", energy: 1 },
];
export function sectionAt(time) {
  const bar = (time / beat - 4) / 4;
  return [...sections].reverse().find((section) => bar >= section.bar) ?? sections[0];
}

export function makeScore() {
  const events = [];
  const tone = (at, length, midi, gain, voice = "lead") =>
    events.push({ at, length, midi, gain, voice });
  const drum = (at, kind, gain = 1) => events.push({ at, kind, gain });
  for (let i = 0; i < 4; i++) tone(i * beat, 0.08, i === 3 ? 88 : 81, 0.14, "bell");
  for (let bar = 0; bar < 56; bar++) {
    const chordIndex = Math.floor(bar / 2) % 4,
      chord = progression[chordIndex],
      root = chord.bass;
    const section = sectionAt((4 + bar * 4) * beat + 0.001),
      drop = section.energy === 1,
      rest = bar >= 32 && bar < 40;
    for (let b = 0; b < 4; b++) {
      const at = (4 + bar * 4 + b) * beat;
      if (!rest || b === 0) drum(at, "kick", rest ? 0.6 : 1);
      if (!rest && b % 2) drum(at, "snare");
      for (let eighth = 0; eighth < 2; eighth++) {
        const off = at + (eighth * beat) / 2;
        drum(off, "hat", rest ? 0.35 : 0.75);
        tone(off + beat / 4, beat * 0.3, root + (eighth ? 12 : 0), rest ? 0.1 : 0.24, "bass");
        const note = electroPhrases[chordIndex][(bar % 2) * 8 + b * 2 + eighth];
        tone(
          off,
          beat * (rest ? 1.2 : 0.43),
          note + (drop ? 12 : 0),
          rest ? 0.08 : drop ? 0.17 : 0.11,
          rest ? "bell" : "lead"
        );
        if (drop && b >= 2)
          tone(off + beat / 4, beat * 0.21, chord.notes[b % 3] + 24, 0.045, "bell");
      }
      if (b % 2 === 0 || drop)
        for (const midi of chord.notes)
          tone(
            at + (drop ? beat / 2 : 0),
            beat * (rest ? 1.8 : drop ? 0.4 : 0.85),
            midi,
            drop ? 0.08 : 0.05,
            "pad"
          );
      if ((bar === 15 || bar === 39) && b >= 2)
        for (let roll = 0; roll < 4; roll++) drum(at + (roll * beat) / 4, "snare", 0.3 + b * 0.1);
    }
  }
  const ending = (4 + 56 * 4) * beat;
  tone(ending, 1.2, 45, 0.17, "bass");
  for (const midi of progression[0].notes) tone(ending, 1.4, midi, 0.055, "pad");
  tone(ending, 1.1, 81, 0.11, "bell");
  return events;
}

export function renderPcm(sampleRate = 22050) {
  const data = new Float32Array(Math.ceil(duration * sampleRate));
  const tau = Math.PI * 2;
  function tone(at, length, midi, gain, voice = "lead") {
    const start = Math.round(at * sampleRate),
      size = Math.ceil(length * sampleRate),
      f = frequency(midi);
    for (let i = 0; i < size && start + i < data.length; i++) {
      const t = i / sampleRate,
        phase = tau * f * t;
      const attack = voice === "pad" ? 0.045 : 0.006;
      const env =
        Math.min(1, t / attack) *
        Math.min(1, (length - t) / 0.035) *
        Math.exp(-t * (voice === "pad" ? 0.7 : 4));
      const songBeat = (at + t) / beat;
      const pump = voice === "bass" ? 1 : 0.3 + 0.7 * Math.min(1, (songBeat % 1) / 0.42);
      const wave =
        voice === "bass"
          ? Math.sin(phase) + 0.18 * Math.sin(phase * 2)
          : voice === "bell"
            ? Math.sin(phase) + 0.25 * Math.sin(phase * 3) * Math.exp(-t * 18)
            : Math.sin(phase) * 0.7 +
              (Math.sin(phase * 0.9985) + Math.sin(phase * 1.0015)) * 0.1 +
              Math.sin(phase * 2) * 0.14 +
              Math.sin(phase * 3) * 0.06;
      data[start + i] += wave * env * gain * pump;
    }
  }
  function drum(at, kind, gain = 1) {
    const start = Math.round(at * sampleRate),
      length = kind === "kick" ? 0.22 : kind === "snare" ? 0.14 : 0.045;
    let seed = Math.round(at * 10000) + 137,
      previous = 0;
    for (let i = 0; i < length * sampleRate && start + i < data.length; i++) {
      const t = i / sampleRate;
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const noise = seed / 2147483648 - 1,
        high = (noise - previous) * 0.5;
      previous = noise;
      const wave =
        kind === "kick"
          ? Math.sin(tau * (48 * t + 115 * 0.025 * (1 - Math.exp(-t / 0.025))))
          : kind === "snare"
            ? high * 0.85 + Math.sin(tau * 180 * t) * Math.exp(-t * 60) * 0.4
            : high;
      data[start + i] +=
        wave *
        Math.min(1, t / 0.002) *
        Math.exp(-t * (kind === "kick" ? 20 : kind === "snare" ? 29 : 85)) *
        (kind === "kick" ? 0.48 : kind === "snare" ? 0.3 : 0.1) *
        gain;
    }
  }
  for (const event of makeScore()) {
    if (event.kind) drum(event.at, event.kind, event.gain);
    else tone(event.at, event.length, event.midi, event.gain, event.voice);
  }
  let peak = 0;
  for (let i = 0; i < data.length; i++) {
    const tail = Math.min(1, (duration - i / sampleRate) / 0.7);
    data[i] = Math.tanh(data[i] * 1.15) * tail;
    peak = Math.max(peak, Math.abs(data[i]));
  }
  const scale = 0.86 / Math.max(0.86, peak);
  for (let i = 0; i < data.length; i++) data[i] *= scale;
  return data;
}

export function makeChart() {
  const notes = [],
    lastEnd = [-10, -10, -10, -10];
  const add = (tick, preferred, hold = 0) => {
    const at = (4 + tick / 4) * beat;
    // Reserve lanes until after a hold release and avoid impossible fast same-lane repeats.
    const lane = [0, 1, 2, 3]
      .map((i) => (preferred + i) % 4)
      .find((l) => at - lastEnd[l] >= beat * 0.62);
    if (lane === undefined) throw new Error(`No playable lane at tick ${tick}`);
    const note = { at, lane };
    if (hold) note.end = at + (hold * beat) / 4;
    lastEnd[lane] = note.end ?? at;
    notes.push(note);
  };
  const wave = [0, 2, 1, 3, 2, 0, 3, 1];
  for (let bar = 0; bar < 56; bar++) {
    const drop = (bar >= 16 && bar < 32) || bar >= 40,
      rest = bar >= 32 && bar < 40;
    const ticks = rest
      ? [0, 4, 8, 12]
      : drop
        ? [0, 2, 4, 6, 8, 9, 10, 11, 12, 13, 14, 15]
        : [0, 2, 4, 6, 8, 10, 12, 14];
    for (const [i, tick] of ticks.entries()) {
      const preferred = (wave[i % 8] + Math.floor(bar / 2)) % 4;
      const hold = rest && tick === 0 ? 10 : !drop && bar % 4 === 3 && tick === 8 ? 4 : 0;
      add(bar * 16 + tick, preferred, hold);
      if (drop && (tick === 0 || tick === 4)) add(bar * 16 + tick, (preferred + 2) % 4);
    }
  }
  return notes.sort((a, b) => a.at - b.at || a.lane - b.lane);
}
