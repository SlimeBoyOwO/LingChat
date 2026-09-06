// Original four-bar kawaii bass loops. Sixteenth-note timing keeps the bass and
// chord chops syncopated; no recordings or third-party melodies are used.
export const BGM_THEMES = [
  {
    bpm: 144,
    roots: [53, 55, 52, 57],
    thirds: [4, 4, 3, 3],
    lead: [12, 16, 19, 23, 19, 16, 14, 12],
  },
  {
    bpm: 150,
    roots: [50, 52, 49, 54],
    thirds: [4, 4, 3, 3],
    lead: [19, 16, 12, 23, 24, 19, 16, 14],
  },
  {
    bpm: 140,
    roots: [56, 58, 55, 60],
    thirds: [4, 4, 3, 3],
    lead: [23, 19, 16, 14, 12, 16, 19, 24],
  },
];
export function bgmTheme(level, boss) {
  const theme = BGM_THEMES[level];
  return boss ? { ...theme, bpm: 164, roots: [57, 53, 60, 55], thirds: [3, 4, 4, 4] } : theme;
}

export function scheduleBgmStep(audio, tick, at, level, boss) {
  const theme = bgmTheme(level, boss),
    step = 60 / theme.bpm / 4,
    bar = Math.floor(tick / 16) % 4,
    position = tick % 16,
    root = theme.roots[bar],
    third = theme.thirds[bar];
  const tone = (midi, duration, volume, shape = "triangle", extra = {}) =>
    audio.tone(at, midi, duration, volume, shape, 0, { music: true, ...extra });

  // A soft sub drop and a crisp attack leave room for the off-beat bass.
  if ([0, 8, ...(boss ? [6, 14] : [11])].includes(position)) {
    audio.duck(at, step * 2);
    audio.tone(at, 45, 0.19, 0.24, "sine", -22);
    audio.noise(at, 0.026, 0.045, 2600);
  }
  if (position === 4 || position === 12) {
    audio.noise(at, 0.12, 0.12, 1400);
    audio.noise(at + 0.012, 0.075, 0.055, 2400);
    audio.tone(at, 50, 0.065, 0.065, "triangle", -8);
  }
  if (position % 2 || (boss && position === 14))
    audio.noise(at, position === 15 ? 0.095 : 0.033, position % 4 === 3 ? 0.038 : 0.022, 6800);

  if ([2, 6, 9, 10, 14].includes(position)) {
    tone(root - 12, step * (position === 9 ? 0.65 : 1.4), 0.12, "sine");
    tone(root, step * 0.85, 0.055, "sawtooth", { cutoff: 650, attack: 0.012 });
  }
  if ([2, 6, 10, 13, 14].includes(position)) {
    const duration = step * (position === 13 ? 0.65 : 1.5);
    for (const interval of [0, third, 7, third === 4 ? 11 : 10]) {
      for (const detune of [-7, 7])
        tone(root + 12 + interval, duration, 0.018, "sawtooth", {
          detune,
          cutoff: 3200,
          attack: 0.018,
        });
    }
  }
  if (position % 2 === 0 && position !== 6) {
    const degree = theme.lead[position / 2];
    // Flatten the third over the minor bars to keep the candy lead in key.
    const note = root + degree - (third === 3 && [16, 23].includes(degree) ? 1 : 0);
    tone(note, step * 2.4, 0.072, "sine");
    tone(note + 12, step * 0.7, 0.025, "sine");
    tone(note, step * 0.8, 0.023, "triangle");
    audio.tone(at + step * 3, note, step, 0.016, "sine", 0, { music: true });
  }
  if (position === 15) {
    tone(root + (bar % 2 ? 26 : 23), step * 0.7, 0.042, "sine");
  }
}
