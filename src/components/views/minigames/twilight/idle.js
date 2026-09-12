// Generated poses use a shared ground anchor; changes in hair/ear height never move the feet.
const idleFrames = [
  { bounds: [148, 25, 312, 427], footX: 303 },
  { bounds: [615, 20, 312, 430], footX: 768.5 },
  { bounds: [1080, 20, 315, 430], footX: 1235 },
  { bounds: [144, 530, 313, 428], footX: 300.5 },
  { bounds: [615, 529, 313, 429], footX: 768.5 },
  { bounds: [1077, 530, 311, 428], footX: 1230 },
];

// A slow breath with a short blink, then a quiet rest. Half lids also reopen the blink.
const sequence = [
  [0, 900],
  [1, 300],
  [2, 360],
  [1, 300],
  [0, 700],
  [3, 70],
  [4, 100],
  [3, 70],
  [5, 320],
  [0, 1000],
];
const cycle = sequence.reduce((sum, [, duration]) => sum + duration, 0);
export function idleFrameAt(elapsed) {
  let time = ((elapsed % cycle) + cycle) % cycle;
  for (const [index, duration] of sequence) {
    if (time < duration) return index;
    time -= duration;
  }
  return 0;
}

export function breathAt(elapsed) {
  const time = ((elapsed % 4400) + 4400) % 4400;
  // A shorter inhale, longer exhale and a quiet rest, independent of the blink.
  if (time < 1600) return (1 - Math.cos((time / 1600) * Math.PI)) / 2;
  if (time < 4000) return (1 + Math.cos(((time - 1600) / 2400) * Math.PI)) / 2;
  return 0;
}

export function drawIdle(ctx, atlas, index, x, y, breath = 0) {
  const {
    bounds: [sx, sy, sw, sh],
    footX,
  } = idleFrames[index];
  const width = Math.round(sw * 0.4),
    height = Math.round(sh * 0.4);
  const left = Math.round(x + 77 + (sx - footX) * 0.4),
    top = Math.round(y + 177) - height;
  const legs = 28,
    torso = 52,
    head = height - legs - torso;
  // Stretch only the hoodie; the head follows the shoulders and the feet stay grounded.
  // Integer pixel motion and shared slice boundaries avoid blur and gaps at the neck/hem.
  const lift = Math.round(breath * 2);
  const slice = (from, size, dest, destSize) =>
    ctx.drawImage(
      atlas,
      sx,
      sy + (from * sh) / height,
      sw,
      (size * sh) / height,
      left,
      top + dest,
      width,
      destSize
    );
  slice(0, head, -lift, head);
  slice(head, torso, head - lift, torso + lift);
  slice(head + torso, legs, head + torso, legs);
}
