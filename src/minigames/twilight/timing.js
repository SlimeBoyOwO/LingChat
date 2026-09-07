/** Normalize legacy epoch timestamps onto the same clock as audio output timestamps. */
export function inputPerformanceTime(
  event,
  now = performance.now(),
  origin = performance.timeOrigin
) {
  let stamp = event?.timeStamp;
  if (stamp > 1e12) stamp -= origin;
  return Number.isFinite(stamp) && stamp > 0 && stamp <= now ? stamp : now;
}
