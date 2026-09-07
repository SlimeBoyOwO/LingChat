// U.N. Owen piano reference by DMBN: higher melody, separate inner harmony and bass.
import score from "../../assets/minigames/twilight/un-owen-score.json";
import { frequency } from "./harmony.js";

const segments = [];
for (const [tick, tempo] of score.tempos) {
  const previous = segments.at(-1);
  segments.push({
    tick,
    seconds: previous ? previous.seconds + ((tick - previous.tick) / score.ppq) * previous.beat : 0,
    beat: tempo / 1000000,
  });
}
function segmentAt(value, field) {
  let low = 0,
    high = segments.length - 1;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    if (segments[mid][field] <= value) low = mid;
    else high = mid - 1;
  }
  return segments[low];
}
function secondsAt(tick) {
  const segment = segmentAt(tick, "tick");
  return segment.seconds + ((tick - segment.tick) / score.ppq) * segment.beat;
}
export const beat = segments[0].beat;
export const bpm = Math.round(60 / beat);
export const bpmLabel = `${bpm} · 变速`;
const countIn = 4 * beat;
export const duration = countIn + secondsAt(score.endTick) + 1.8;
export function beatAt(time) {
  return segmentAt(time - countIn, "seconds").beat;
}
export function bpmAt(time) {
  return Math.round(60 / beatAt(time));
}
export function beatPosition(time) {
  if (time < countIn) return time / beat;
  const segment = segmentAt(time - countIn, "seconds");
  return 4 + segment.tick / score.ppq + (time - countIn - segment.seconds) / segment.beat;
}
const sections = [
  { tick: 0, name: "SCARLET INTRO", energy: 0.35 },
  { tick: 38400, name: "U.N. OWEN", energy: 0.65 },
  { tick: 69120, name: "CRYSTAL WINGS", energy: 1 },
  { tick: 138240, name: "MIDNIGHT", energy: 0.65 },
  { tick: 176640, name: "SCARLET STORM", energy: 1 },
  { tick: 240000, name: "FINAL SPELL", energy: 1 },
  { tick: 301440, name: "RITARDANDO", energy: 0.25 },
];
export function sectionAt(time) {
  return (
    [...sections].reverse().find((section) => time >= countIn + secondsAt(section.tick)) ??
    sections[0]
  );
}

export function renderPcm(sampleRate = 22050) {
  const data = new Float32Array(Math.ceil(duration * sampleRate));
  // Each part has a stable pitch and a soft timbre; no layered octaves or pitch detuning.
  const tables = new Map(),
    tableSize = 2048;
  function tone(at, length, midi, velocity, gain, voice = "lead") {
    const f = frequency(midi),
      key = `${voice}:${midi}`,
      harmonics = voice === "lead" ? [1, 0.18, 0.04] : [1, 0.1, 0.02];
    if (!tables.has(key)) {
      const table = new Float32Array(tableSize + 1);
      for (let i = 0; i <= tableSize; i++) {
        for (const [index, level] of harmonics.entries()) {
          const harmonic = index + 1;
          if (f * harmonic < sampleRate * 0.45)
            table[i] += Math.sin((i / tableSize) * Math.PI * 2 * harmonic) * level;
        }
      }
      tables.set(key, table);
    }
    const table = tables.get(key),
      start = Math.round(at * sampleRate),
      release = voice === "bass" ? 0.055 : 0.09,
      size = Math.ceil((length + release) * sampleRate),
      step = (f / sampleRate) * tableSize,
      decayStep = Math.exp(-(voice === "bass" ? 3 : 0.8 + f / 3000) / sampleRate),
      amplitude = gain * (velocity / 127) ** 1.4;
    let phase = 0,
      decay = 1;
    for (let i = 0; i < size && start + i < data.length; i++) {
      const t = i / sampleRate,
        index = Math.floor(phase),
        fraction = phase - index,
        wave = table[index] + (table[index + 1] - table[index]) * fraction,
        envelope =
          Math.min(1, t / (voice === "chord" ? 0.025 : 0.008)) *
          Math.max(0, 1 - Math.max(0, t - length) / release);
      data[start + i] += wave * envelope * decay * amplitude;
      phase = (phase + step) % tableSize;
      decay *= decayStep;
    }
  }
  for (let i = 0; i < 4; i++) tone(i * beat, 0.04, i === 3 ? 88 : 81, 90, 0.1);
  for (const [tick, ticks, midi, velocity] of score.melody) {
    const at = secondsAt(tick);
    tone(countIn + at, secondsAt(tick + ticks) - at, midi, velocity, 0.24);
  }
  for (const [voice, notes, gain] of [
    ["chord", score.harmony, 0.055],
    ["bass", score.bass, 0.085],
  ]) {
    for (const [tick, ticks, midi, velocity] of notes) {
      const at = secondsAt(tick);
      tone(countIn + at, secondsAt(tick + ticks) - at, midi, velocity, gain, voice);
    }
  }
  let peak = 0;
  for (let i = 0; i < data.length; i++) {
    data[i] *= Math.min(1, (duration - i / sampleRate) / 0.5);
    peak = Math.max(peak, Math.abs(data[i]));
  }
  const scale = 0.82 / Math.max(0.2, peak);
  for (let i = 0; i < data.length; i++) data[i] *= scale;
  return data;
}

export function makeChart() {
  const bars = new Set();
  for (const [index, [start, numerator, denominator]] of score.meters.entries()) {
    const end = score.meters[index + 1]?.[0] ?? score.endTick;
    for (let tick = start; tick < end; tick += (score.ppq * numerator * 4) / denominator)
      bars.add(tick);
  }
  const accents = new Set(score.bass.filter(([tick]) => bars.has(tick)).map(([tick]) => tick));
  const notes = [],
    lastEnd = [-10, -10, -10, -10];
  let previousPitch = 69,
    previousLane = 1;
  for (const [tick, length, pitch] of score.melody) {
    const at = countIn + secondsAt(tick),
      chord = accents.has(tick);
    const direction = Math.sign(pitch - previousPitch),
      preferred = (previousLane + (direction || 2) + 4) % 4;
    for (let hand = 0; hand < (chord ? 2 : 1); hand++) {
      const lane = [0, 1, 2, 3]
        .map((i) => (preferred + hand * 2 + i) % 4)
        .find((candidate) => at - lastEnd[candidate] >= 0.17);
      if (lane === undefined) continue;
      const note = { at, lane };
      if (hand === 0 && length >= score.ppq * 1.5) {
        note.end = countIn + secondsAt(tick + Math.min(length, score.ppq * 4));
      }
      lastEnd[lane] = note.end ?? at;
      notes.push(note);
      if (hand === 0) previousLane = lane;
    }
    previousPitch = pitch;
  }
  return notes.sort((a, b) => a.at - b.at || a.lane - b.lane);
}
