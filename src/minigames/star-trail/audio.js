import { bgmTheme, scheduleBgmStep } from "./bgm.js";
const frequency = (midi) => 440 * 2 ** ((midi - 69) / 12);

/** A short look-ahead sequencer. Scheduled voices are stopped on pause and disposed on exit. */
export class TrailAudio {
  constructor(onInterrupted = () => {}) {
    this.onInterrupted = onInterrupted;
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
      const Context = window.AudioContext || window.webkitAudioContext;
      this.context = new Context();
      this.context.onstatechange = () => {
        if (this.playing && ["interrupted", "suspended"].includes(this.context.state))
          this.onInterrupted();
      };
      this.gain = this.context.createGain();
      this.gain.gain.value = this.volume;
      this.compressor = this.context.createDynamicsCompressor();
      this.compressor.threshold.value = -12;
      this.compressor.knee.value = 12;
      this.compressor.ratio.value = 4;
      this.gain.connect(this.compressor);
      this.compressor.connect(this.context.destination);
      this.musicBus = this.context.createGain();
      this.musicBus.connect(this.gain);
      this.noiseBuffer = this.context.createBuffer(
        1,
        this.context.sampleRate / 2,
        this.context.sampleRate
      );
      const samples = this.noiseBuffer.getChannelData(0);
      let seed = 137;
      for (let i = 0; i < samples.length; i++) {
        seed = (Math.imul(seed, 1664525) + 1013904223) | 0;
        samples[i] = seed / 2147483648;
      }
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
  tone(at, midi, duration, gain, shape = "square", slide = 0, options = {}) {
    if (!this.context || this.destroyed || this.context.state !== "running") return;
    const oscillator = this.context.createOscillator(),
      envelope = this.context.createGain();
    oscillator.type = shape;
    oscillator.detune.value = options.detune || 0;
    oscillator.frequency.setValueAtTime(frequency(midi), at);
    if (slide)
      oscillator.frequency.exponentialRampToValueAtTime(frequency(midi + slide), at + duration);
    envelope.gain.setValueAtTime(0, at);
    envelope.gain.linearRampToValueAtTime(gain, at + (options.attack || 0.006));
    envelope.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    let filter;
    if (options.cutoff) {
      filter = this.context.createBiquadFilter();
      filter.type = "lowpass";
      filter.frequency.setValueAtTime(options.cutoff, at);
      filter.frequency.exponentialRampToValueAtTime(options.cutoff * 0.45, at + duration);
      oscillator.connect(filter);
      filter.connect(envelope);
    } else oscillator.connect(envelope);
    envelope.connect(options.music ? this.musicBus : this.gain);
    const voice = { oscillator, envelope, filter };
    this.voices.add(voice);
    oscillator.onended = () => {
      oscillator.disconnect();
      envelope.disconnect();
      filter?.disconnect();
      this.voices.delete(voice);
    };
    oscillator.start(at);
    oscillator.stop(at + duration + 0.01);
  }
  duck(at, duration) {
    this.musicBus.gain.setValueAtTime(0.3, at);
    this.musicBus.gain.linearRampToValueAtTime(1, at + duration);
  }
  noise(at, duration, volume, cutoff) {
    if (!this.context || this.context.state !== "running" || this.destroyed) return;
    const oscillator = this.context.createBufferSource(),
      envelope = this.context.createGain(),
      filter = this.context.createBiquadFilter();
    oscillator.buffer = this.noiseBuffer;
    filter.type = "highpass";
    filter.frequency.value = cutoff;
    envelope.gain.setValueAtTime(0, at);
    envelope.gain.linearRampToValueAtTime(volume, at + 0.002);
    envelope.gain.exponentialRampToValueAtTime(0.0001, at + duration);
    oscillator.connect(filter);
    filter.connect(envelope);
    envelope.connect(this.gain);
    const voice = { oscillator, envelope, filter };
    this.voices.add(voice);
    oscillator.onended = () => {
      oscillator.disconnect();
      envelope.disconnect();
      filter.disconnect();
      this.voices.delete(voice);
    };
    oscillator.start(at);
    oscillator.stop(at + duration + 0.01);
  }
  tick() {
    if (!this.playing || !this.context || this.context.state !== "running" || this.destroyed)
      return;
    const now = this.context.currentTime,
      step = 60 / bgmTheme(this.level, this.boss).bpm / 4;
    if (this.next < now) this.next = now + 0.03;
    while (this.next < now + 0.13) {
      scheduleBgmStep(this, this.beat++, this.next, this.level, this.boss);
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
    if (this.musicBus) {
      this.musicBus.gain.cancelScheduledValues(this.context.currentTime);
      this.musicBus.gain.value = 1;
    }
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
    if (this.context) this.context.onstatechange = null;
    this.pause();
    if (this.context && this.context.state !== "closed") void this.context.close().catch(() => {});
  }
}
