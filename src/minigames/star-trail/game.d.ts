export interface StarTrailController {
  destroy(): void;
  snapshot(): {
    mode: string;
    level: number;
    player: { x: number; y: number; hp: number; grounded: boolean };
    bossHP: number;
    score: number;
    crystals: number;
    time: number;
    audioState?: AudioContextState;
  };
}
export function mountStarTrail(
  root: ShadowRoot,
  options: { signal: AbortSignal; onExit(): void }
): StarTrailController;
