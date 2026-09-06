import assert from "node:assert/strict";
import { Judge } from "../src/minigames/twilight/core.js";
import * as music from "../src/minigames/twilight/music.js";
let j = new Judge([
  { at: 1, lane: 0 },
  { at: 2, lane: 1 },
  { at: 3, lane: 2 },
  { at: 4, lane: 3 },
]);
assert.equal(j.press(0, 1.02), "perfect");
j.release(0, 1.02);
assert.equal(j.press(1, 2.08), "good");
j.release(1, 2.08);
assert.equal(j.press(2, 3.12), "ok");
j.release(2, 3.12);
j.update(4.151);
assert.deepEqual(j.counts, { perfect: 1, good: 1, ok: 1, miss: 1 });
assert.equal(j.maxCombo, 3);
assert.equal(j.combo, 0);
assert.equal(j.result().score, 487500);
j = new Judge([{ at: 1, end: 2, lane: 0 }]);
j.press(0, 1);
assert.equal(j.press(0, 1.01), null);
j.release(0, 1.5);
assert.equal(j.result().miss, 1);
j = new Judge([{ at: 1, end: 2, lane: 0 }]);
j.press(0, 1);
j.update(2.01);
j.release(0, 2.01);
assert.equal(j.result().perfect, 1);
assert.equal(j.resolved, 1);
j = new Judge([{ at: 1, end: 3, lane: 0 }]);
j.press(0, 1);
j.pause();
j.resume(2);
j.press(0, 2);
j.release(0, 3);
assert.equal(j.result().perfect, 1);
assert.equal(j.resolved, 1);
const notes = music.makeChart();
assert(notes.length > 120);
assert(notes.every((n) => n.lane >= 0 && n.lane < 4 && n.at >= music.beat * 4));
for (const [i, n] of notes.entries()) {
  if (n.end)
    assert(
      !notes.some(
        (other, k) => k !== i && other.lane === n.lane && other.at > n.at && other.at < n.end
      )
    );
}
j = new Judge(notes);
const actions = notes
  .flatMap((n) => [
    { t: n.at, lane: n.lane, down: true },
    { t: n.end ?? n.at + 0.02, lane: n.lane, down: false },
  ])
  .sort((a, b) => a.t - b.t);
for (const action of actions)
  action.down ? j.press(action.lane, action.t) : j.release(action.lane, action.t);
j.update(music.duration);
assert.equal(j.result().accuracy, 1);
assert.equal(j.result().miss, 0);
assert.equal(j.maxCombo, notes.length);
j = new Judge(notes);
j.update(music.duration);
assert.equal(j.result().miss, notes.length);
console.log(
  `PASS: timing windows, early/complete holds, duplicate input, pause/resume, ${notes.length}-note perfect and missed runs.`
);
