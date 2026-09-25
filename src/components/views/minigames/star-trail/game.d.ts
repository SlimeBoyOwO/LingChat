export interface StarTrailController {
  destroy(): void;
}
export function mountStarTrail(
  root: ShadowRoot,
  options: { signal: AbortSignal; onExit(): void }
): StarTrailController;
