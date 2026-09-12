// Original A-natural-minor arrangements. Pitches are MIDI numbers, not root-relative offsets.
// Chord tones carry the downbeats; diatonic passing tones connect them on eighth notes.
export const progression = [
  { name: "Am", bass: 45, notes: [57, 60, 64] },
  { name: "F", bass: 41, notes: [57, 60, 65] },
  { name: "C", bass: 48, notes: [55, 60, 64] },
  { name: "G", bass: 43, notes: [55, 59, 62] },
];

export const electroPhrases = [
  [69, 71, 72, 74, 76, 74, 72, 71, 69, 72, 76, 79, 76, 74, 72, 71],
  [69, 67, 65, 67, 69, 71, 72, 71, 69, 67, 65, 64, 65, 67, 69, 71],
  [67, 69, 72, 74, 76, 74, 72, 71, 67, 69, 72, 74, 76, 79, 76, 74],
  [67, 69, 71, 72, 74, 72, 71, 69, 67, 69, 71, 72, 74, 72, 71, 67],
];

export const studyPhrases = [
  [76, 72, 69, 72, 76, 79, 76, 72],
  [72, 69, 65, 69, 72, 76, 72, 69],
  [76, 72, 67, 72, 76, 79, 76, 72],
  [74, 71, 67, 71, 74, 79, 74, 71],
];

export const frequency = (midi) => 440 * 2 ** ((midi - 69) / 12);
