// A single extracted melody with independently arranged triads and bass.
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
  let previousVoicing = [50, 57, 62];
  for (const [tick, ticks, root, quality] of score.harmony) {
    const intervals = [
      [0, 4, 7],
      [0, 3, 7],
      [0, 3, 6],
    ][quality];
    const voicings = intervals
      .reduce(
        (list, interval) => {
          const low = 48 + ((root + interval) % 12),
            choices = low + 12 <= 64 ? [low, low + 12] : [low];
          return list.flatMap((notes) => choices.map((pitch) => [...notes, pitch]));
        },
        [[]]
      )
      .map((notes) => notes.sort((a, b) => a - b));
    const motion = (notes) =>
      notes.reduce((sum, pitch, i) => sum + Math.abs(pitch - previousVoicing[i]), 0);
    voicings.sort((a, b) => motion(a) - motion(b));
    previousVoicing = voicings[0];
    for (let pulse = tick; pulse < tick + ticks; pulse += score.ppq * 2) {
      const at = secondsAt(pulse),
        end = Math.min(tick + ticks, pulse + score.ppq * 1.6);
      for (const midi of previousVoicing)
        tone(countIn + at, secondsAt(end) - at, midi, 80, 0.055, "chord");
      for (let b = 0; b < 2 && pulse + b * score.ppq < tick + ticks; b++) {
        const bassTick = pulse + b * score.ppq,
          bassAt = secondsAt(bassTick),
          bassEnd = Math.min(tick + ticks, bassTick + score.ppq * 0.6);
        tone(
          countIn + bassAt,
          secondsAt(bassEnd) - bassAt,
          36 + root + (b ? intervals[2] : 0),
          85,
          0.085,
          "bass"
        );
      }
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
  const accents = new Set(
    score.harmony.filter(([tick]) => tick % (score.ppq * 4) === 0).map(([tick]) => tick)
  );
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
