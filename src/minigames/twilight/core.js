const WINDOWS = { perfect: 0.055, good: 0.1, hit: 0.15, release: 0.1 };
const WEIGHT = { perfect: 1, good: 0.65, ok: 0.3, miss: 0 };
class Judge {
  constructor(notes, { inputGrace = 0, recordTiming = true } = {}) {
    this.notes = notes.map((note, id) => ({ ...note, id, state: "pending" }));
    this.held = new Set();
    this.combo = 0;
    this.maxCombo = 0;
    this.points = 0;
    this.resolved = 0;
    this.counts = { perfect: 0, good: 0, ok: 0, miss: 0 };
    this.events = [];
    this.inputGrace = inputGrace;
    this.recordTiming = recordTiming;
    this.timingSamples = 0;
    this.timingSum = 0;
    this.timingSquares = 0;
  }
  finish(note, grade, time, error = null) {
    if (note.state === "done") return;
    note.state = "done";
    note.grade = grade;
    this.counts[grade]++;
    this.resolved++;
    this.points += WEIGHT[grade];
    this.combo = grade === "miss" ? 0 : this.combo + 1;
    this.maxCombo = Math.max(this.maxCombo, this.combo);
    this.events.push({ grade, lane: note.lane, time, combo: this.combo, error });
  }
  update(time) {
    // Allow recently queued input to arrive before automatic misses/tails are resolved.
    // Press/release still use the original event time and unchanged hit windows.
    const automaticTime = time - this.inputGrace;
    for (const note of this.notes) {
      if (note.state === "pending" && automaticTime - (note.resumeAt ?? note.at) > WINDOWS.hit)
        this.finish(note, "miss", time);
      if (note.state === "holding" && automaticTime >= note.end && this.held.has(note.lane)) {
        this.finish(note, note.headGrade, note.end);
      }
    }
  }
  press(lane, time) {
    if (this.held.has(lane)) return null;
    this.update(time);
    this.held.add(lane);
    const note = this.notes.find(
      (n) =>
        n.lane === lane &&
        n.state === "pending" &&
        Math.abs(time - (n.resumeAt ?? n.at)) <= WINDOWS.hit
    );
    if (!note) return null;
    const error = time - note.at;
    const resumed = note.resumeAt != null;
    const grade = resumed
      ? note.headGrade
      : Math.abs(error) <= WINDOWS.perfect
        ? "perfect"
        : Math.abs(error) <= WINDOWS.good
          ? "good"
          : "ok";
    if (!resumed && this.recordTiming) {
      this.timingSamples++;
      this.timingSum += error * 1000;
      this.timingSquares += (error * 1000) ** 2;
    }
    if (note.end != null) {
      note.state = "holding";
      note.headGrade = grade;
      this.events.push({
        grade: "hold",
        lane,
        time,
        combo: this.combo,
        error: resumed ? null : error,
      });
    } else this.finish(note, grade, time, error);
    return grade;
  }
  release(lane, time) {
    this.update(time);
    this.held.delete(lane);
    const note = this.notes.find((n) => n.lane === lane && n.state === "holding");
    if (note) {
      const early = note.end - time;
      const grade =
        early > WINDOWS.release
          ? "miss"
          : early > WINDOWS.perfect && note.headGrade === "perfect"
            ? "good"
            : note.headGrade;
      this.finish(note, grade, time);
    }
  }
  pause() {
    // Keep the chart and hold grade intact while physical input is released.
    this.held.clear();
  }
  resume(time) {
    // A separate regrab time keeps the original chart head immutable across pauses.
    for (const note of this.notes)
      if (note.state === "holding" || (note.state === "pending" && note.resumeAt != null)) {
        if (time >= note.end) {
          this.finish(note, note.headGrade, note.end);
          continue;
        }
        note.state = "pending";
        note.resumeAt = time;
      }
  }
  result(status = "completed") {
    const meanErrorMs = this.timingSamples ? this.timingSum / this.timingSamples : null;
    return {
      status,
      accuracy: this.notes.length ? this.points / this.notes.length : 0,
      score: Math.round((this.points / Math.max(1, this.notes.length)) * 1000000),
      maxCombo: this.maxCombo,
      ...this.counts,
      totalNotes: this.notes.length,
      timingSamples: this.timingSamples,
      meanErrorMs,
      timingDeviationMs:
        meanErrorMs == null
          ? null
          : Math.sqrt(Math.max(0, this.timingSquares / this.timingSamples - meanErrorMs ** 2)),
    };
  }
}
export { Judge, WINDOWS };
