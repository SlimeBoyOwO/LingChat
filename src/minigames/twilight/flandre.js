// Bundled piano MIDI arrangement. Keep source pitches, velocities and tempo changes.
import score from "../../assets/minigames/twilight/flandre-score.json";
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
  { bar: 0, name: "SCARLET INTRO", energy: 0.35 },
  { bar: 8, name: "CRYSTAL WINGS", energy: 0.65 },
  { bar: 40, name: "SCARLET STORM", energy: 1 },
  { bar: 72, name: "MIDNIGHT", energy: 0.65 },
  { bar: 112, name: "FINAL SPELL", energy: 1 },
  { bar: 168, name: "RITARDANDO", energy: 0.25 },
];
export function sectionAt(time) {
  const bar = (beatPosition(time) - 4) / 4;
  return [...sections].reverse().find((section) => bar >= section.bar) ?? sections[0];
}

export function renderPcm(sampleRate = 22050) {
  const data = new Float32Array(Math.ceil(duration * sampleRate));
  // One band-limited wavetable per pitch: dense MIDI stays practical on mobile workers.
  const tables = new Map(),
    tableSize = 2048;
  function tone(at, length, midi, velocity, gain) {
    const f = frequency(midi);
    if (!tables.has(midi)) {
      const table = new Float32Array(tableSize + 1);
      for (let i = 0; i <= tableSize; i++) {
        for (const [harmonic, level] of [
          [1, 1],
          [2, 0.3],
          [3, 0.12],
          [4, 0.04],
        ]) {
          if (f * harmonic < sampleRate * 0.45)
            table[i] += Math.sin((i / tableSize) * Math.PI * 2 * harmonic) * level;
        }
      }
      tables.set(midi, table);
    }
    const table = tables.get(midi),
      start = Math.round(at * sampleRate),
      release = 0.18,
      size = Math.ceil((length + release) * sampleRate),
      step = (f / sampleRate) * tableSize,
      decayStep = Math.exp(-(1.3 + f / 2200) / sampleRate),
      amplitude = gain * (velocity / 127) ** 1.4;
    let phase = 0,
      decay = 1;
    for (let i = 0; i < size && start + i < data.length; i++) {
      const t = i / sampleRate,
        index = Math.floor(phase),
        fraction = phase - index,
        wave = table[index] + (table[index + 1] - table[index]) * fraction,
        envelope = Math.min(1, t / 0.004) * Math.max(0, 1 - Math.max(0, t - length) / release);
      data[start + i] += wave * envelope * decay * amplitude;
      phase = (phase + step) % tableSize;
      decay *= decayStep;
    }
  }
  for (let i = 0; i < 4; i++) tone(i * beat, 0.04, i === 3 ? 88 : 81, 90, 0.1);
  for (const track of score.tracks) {
    const gain = track.name.startsWith("Main Melody")
      ? 0.13
      : track.name.startsWith("Base")
        ? 0.065
        : track.name.startsWith("Chord")
          ? 0.055
          : 0.035;
    for (const [tick, ticks, midi, velocity] of track.notes) {
      const at = secondsAt(tick);
      tone(countIn + at, secondsAt(tick + ticks) - at, midi, velocity, gain);
    }
  }
  let peak = 0;
  for (let i = 0; i < data.length; i++) {
    data[i] = Math.tanh(data[i] * 1.35) * Math.min(1, (duration - i / sampleRate) / 0.5);
    peak = Math.max(peak, Math.abs(data[i]));
  }
  const scale = 0.86 / Math.max(0.86, peak);
  for (let i = 0; i < data.length; i++) data[i] *= scale;
  return data;
}

export function makeChart() {
  const groups = new Map();
  for (const [part, track] of score.tracks.entries()) {
    if (!track.name.startsWith("Main Melody")) continue;
    for (const [tick, length, pitch, velocity] of track.notes) {
      // Merge voices landing on the same sixteenth, retaining the primary voice's exact onset.
      const grid = Math.round(tick / (score.ppq / 4));
      if (!groups.has(grid)) groups.set(grid, []);
      groups.get(grid).push({ tick, length, pitch, velocity, part });
    }
  }
  const notes = [],
    lastEnd = [-10, -10, -10, -10];
  let previousPitch = 69,
    previousLane = 1;
  for (const [grid, voices] of [...groups].sort((a, b) => a[0] - b[0])) {
    voices.sort((a, b) => a.part - b.part || b.pitch - a.pitch);
    const lead = voices[0],
      at = countIn + secondsAt(lead.tick),
      chord = grid % 4 === 0 && new Set(voices.map((v) => v.pitch)).size >= 3;
    const direction = Math.sign(lead.pitch - previousPitch),
      preferred = (previousLane + (direction || 2) + 4) % 4;
    for (let hand = 0; hand < (chord ? 2 : 1); hand++) {
      const lane = [0, 1, 2, 3]
        .map((i) => (preferred + hand * 2 + i) % 4)
        .find((candidate) => at - lastEnd[candidate] >= 0.17);
      if (lane === undefined) continue;
      const note = { at, lane };
      if (hand === 0 && lead.length >= score.ppq * 1.5) {
        note.end = countIn + secondsAt(lead.tick + Math.min(lead.length, score.ppq * 2));
      }
      lastEnd[lane] = note.end ?? at;
      notes.push(note);
      if (hand === 0) previousLane = lane;
    }
    previousPitch = lead.pitch;
  }
  return notes.sort((a, b) => a.at - b.at || a.lane - b.lane);
}
