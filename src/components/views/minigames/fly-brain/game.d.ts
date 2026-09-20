export interface FlyBrainController {
  destroy(): void;
}

export function mountFlyBrain(
  root: ShadowRoot,
  options: {
    signal: AbortSignal;
    onExit(): void;
  },
): Promise<FlyBrainController>;
