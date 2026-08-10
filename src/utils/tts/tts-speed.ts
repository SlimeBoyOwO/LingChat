/** Normalize the user-facing SBV2 duration multiplier. Larger values sound slower. */
export function speedToLengthScale(lengthScale: number): number {
  return Number.isFinite(lengthScale) && lengthScale > 0 ? lengthScale : 1
}
