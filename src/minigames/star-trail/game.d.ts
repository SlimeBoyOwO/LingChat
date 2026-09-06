export interface StarTrailController {
  destroy(): void;
  snapshot(): {
    mode: string;
    level: number;
    player: {
      x: number;
      y: number;
      hp: number;
      grounded: boolean;
      armor: number;
      shield: number;
      rapid: number;
      magnet: number;
    };
    bossHP: number;
    score: number;
    crystals: number;
    wallet: number;
    armorLevel: number;
    interaction: string;
    time: number;
    audioState?: AudioContextState;
  };
}
export function mountStarTrail(
  root: ShadowRoot,
  options: { signal: AbortSignal; onExit(): void }
): StarTrailController;
