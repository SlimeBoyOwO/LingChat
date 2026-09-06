const WINDOWS = { perfect: 0.055, good: 0.1, hit: 0.15, release: 0.1 };
const WEIGHT = { perfect: 1, good: 0.65, ok: 0.3, miss: 0 };
class Judge {
  constructor(notes) {
    this.notes = notes.map((note, id) => ({ ...note, id, state: "pending" }));
    this.held = new Set();
    this.combo = 0;
    this.maxCombo = 0;
    this.points = 0;
    this.resolved = 0;
    this.counts = { perfect: 0, good: 0, ok: 0, miss: 0 };
    this.events = [];
  }
  finish(note, grade, time) {
    if (note.state === "done") return;
    note.state = "done";
    note.grade = grade;
    this.counts[grade]++;
    this.resolved++;
    this.points += WEIGHT[grade];
    this.combo = grade === "miss" ? 0 : this.combo + 1;
    this.maxCombo = Math.max(this.maxCombo, this.combo);
    this.events.push({ grade, lane: note.lane, time, combo: this.combo });
  }
  update(time) {
    for (const note of this.notes) {
      if (note.state === "pending" && time - note.at > WINDOWS.hit) this.finish(note, "miss", time);
      if (note.state === "holding" && time >= note.end && this.held.has(note.lane)) {
        this.finish(note, note.headGrade, time);
      }
    }
  }
  press(lane, time) {
    if (this.held.has(lane)) return null;
    this.update(time);
    this.held.add(lane);
    const note = this.notes.find(
      (n) => n.lane === lane && n.state === "pending" && Math.abs(time - n.at) <= WINDOWS.hit
    );
    if (!note) return null;
    const error = Math.abs(time - note.at);
    const grade = note.resumeHold
      ? note.headGrade
      : error <= WINDOWS.perfect
        ? "perfect"
        : error <= WINDOWS.good
          ? "good"
          : "ok";
    if (note.end != null) {
      note.state = "holding";
      note.headGrade = grade;
      this.events.push({ grade: "hold", lane, time, combo: this.combo });
    } else this.finish(note, grade, time);
    return grade;
  }
  release(lane, time) {
    this.update(time);
    this.held.delete(lane);
    const note = this.notes.find((n) => n.lane === lane && n.state === "holding");
    if (note) this.finish(note, time >= note.end - WINDOWS.release ? note.headGrade : "miss", time);
  }
  pause() {
    // Return unfinished holds to pending. The player can pick them up after resuming.
    this.held.clear();
  }
  resume(time) {
    // A pause never awards or loses a hold. Move its remaining head to the resume point.
    for (const note of this.notes)
      if (note.state === "holding") {
        note.state = "pending";
        note.at = time;
        note.resumeHold = true;
      }
  }
  result(status = "completed") {
    return {
      status,
      accuracy: this.notes.length ? this.points / this.notes.length : 0,
      score: Math.round((this.points / Math.max(1, this.notes.length)) * 1000000),
      maxCombo: this.maxCombo,
      ...this.counts,
      totalNotes: this.notes.length,
    };
  }
}
export { Judge, WINDOWS };
