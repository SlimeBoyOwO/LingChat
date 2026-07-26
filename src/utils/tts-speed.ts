/** Convert the user-facing speed multiplier to SBV2's duration multiplier. */
export function speedToLengthScale(speed: number): number {
  const normalized = Number.isFinite(speed) && speed > 0 ? speed : 1
  return 1 / normalized
}
