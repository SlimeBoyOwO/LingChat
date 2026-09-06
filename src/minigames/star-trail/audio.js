const frequency = (midi) => 440 * 2 ** ((midi - 69) / 12);
const melodies = [
  [76, 79, 83, 79, 81, 79, 76, 74, 72, 76, 79, 76, 74, 72, 71, 74],
  [69, 72, 76, 79, 76, 72, 74, 71, 67, 71, 74, 77, 76, 74, 72, 71],
  [81, 76, 79, 83, 81, 79, 76, 74, 72, 76, 79, 84, 83, 79, 76, 79],
];

/** A short look-ahead sequencer. Scheduled voices are stopped on pause and disposed on exit. */
export class TrailAudio {
  constructor() {
    this.context = null;
    this.voices = new Set();
    this.volume = 0.35;
    this.destroyed = false;
    this.playing = false;
    this.level = 0;
    this.boss = false;
    this.beat = 0;
    this.generation = 0;
  }
  async start(level, boss = false, generation) {
    if (this.destroyed) return;
    if (!this.context) {
      this.context = new AudioContext();
      this.gain = this.context.createGain();
      this.gain.gain.value = this.volume;
      this.gain.connect(this.context.destination);
    }
    try {
      await this.context.resume();
    } catch {
      return;
    }
    if (this.destroyed || !this.playing || generation !== this.generation) {
      if (!this.destroyed && !this.playing && this.context.state === "running")
        await this.context.suspend().catch(() => {});
      return;
    }
    this.setTheme(level, boss);
  }
  setTheme(level, boss) {
    this.level = level;
    this.boss = boss;
    this.stopVoices();
    this.beat = 0;
    this.next = (this.context?.currentTime || 0) + 0.06;
  }
  setVolume(volume) {
    this.volume = volume;
    if (this.gain && !this.destroyed)
      this.gain.gain.setTargetAtTime(volume, this.context.currentTime, 0.02);
  }
  tone(at, midi, duration, gain, shape = "square", slide = 0) {
    if (!this.context || this.destroyed || this.context.state !== "running") return;
    const oscillator = this.context.createOscillator(),
      envelope = this.context.createGain();
    oscillator.type = shape;
    oscillator.frequency.setValueAtTime(frequency(midi), at);
    if (slide)
      oscillator.frequency.exponentialRampToValueAtTime(frequency(midi + slide), at + duration);
    envelope.gain.setValueAtTime(0, at);
    envelope.gain.linearRampToValueAtTime(gain, at + 0.006);
    envelope.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    oscillator.connect(envelope);
    envelope.connect(this.gain);
    const voice = { oscillator, envelope };
    this.voices.add(voice);
    oscillator.onended = () => {
      oscillator.disconnect();
      envelope.disconnect();
      this.voices.delete(voice);
    };
    oscillator.start(at);
    oscillator.stop(at + duration + 0.01);
  }
  tick() {
    if (!this.playing || !this.context || this.context.state !== "running" || this.destroyed)
      return;
    const now = this.context.currentTime,
      step = 60 / (this.boss ? 152 : [118, 130, 112][this.level]) / 2;
    if (this.next < now) this.next = now + 0.03;
    while (this.next < now + 0.13) {
      const beat = this.beat++,
        chord = [0, -5, -3, -7][Math.floor(beat / 8) % 4];
      const note = this.boss
        ? [69, 72, 70, 76, 69, 77, 76, 72][beat % 8]
        : melodies[this.level][beat % 16];
      this.tone(this.next, note + chord, step * 0.7, 0.085, "square");
      if (beat % 2 === 0) this.tone(this.next, 45 + chord, step * 1.45, 0.18, "triangle");
      if (beat % 4 === 0) this.tone(this.next, 42, 0.12, 0.2, "sine", -22);
      else if (beat % 2 === 1) this.tone(this.next, 101, 0.025, 0.028, "square", -13);
      this.next += step;
    }
  }
  effect(type) {
    if (!this.context || !this.playing) return;
    const at = this.context.currentTime;
    if (type === "shoot") this.tone(at, 83, 0.065, 0.045, "square", -16);
    else if (type === "jump") this.tone(at, 61, 0.14, 0.08, "triangle", 17);
    else if (type === "spring") this.tone(at, 48, 0.3, 0.12, "triangle", 30);
    else if (type === "crate") this.tone(at, 43, 0.12, 0.13, "square", -18);
    else if (type === "armor-hit" || type === "shield-break") {
      this.tone(at, 88, 0.14, 0.1, "triangle", -19);
      this.tone(at, 69, 0.12, 0.07, "square");
    } else if (["shield", "rapid", "magnet"].includes(type))
      [72, 79, 84].forEach((note, i) => this.tone(at + i * 0.06, note, 0.17, 0.08, "triangle"));
    else if (type === "coin" || type === "heal") {
      this.tone(at, 88, 0.07, 0.12, "triangle");
      this.tone(at + 0.065, 95, 0.14, 0.09, "triangle");
    } else if (type === "hurt") this.tone(at, 46, 0.22, 0.14, "sawtooth", -12);
    else if (type === "burst" || type === "slam") this.tone(at, 39, 0.22, 0.18, "triangle", -24);
    else if (type === "hit") this.tone(at, 67, 0.035, 0.045, "square", -9);
    else if (type === "checkpoint" || type === "boss-down")
      [72, 76, 79, 84].forEach((note, i) => this.tone(at + i * 0.1, note, 0.22, 0.11, "triangle"));
  }
  stopVoices() {
    for (const voice of [...this.voices]) {
      try {
        voice.oscillator.stop();
      } catch {
        /* Already ended. */
      }
    }
  }
  pause() {
    this.playing = false;
    this.generation++;
    this.stopVoices();
    if (this.context?.state === "running") void this.context.suspend().catch(() => {});
  }
  play(level, boss) {
    this.playing = true;
    const generation = ++this.generation;
    void this.start(level, boss, generation).catch(() => {});
  }
  destroy() {
    this.destroyed = true;
    this.pause();
    if (this.context && this.context.state !== "closed") void this.context.close().catch(() => {});
  }
}
