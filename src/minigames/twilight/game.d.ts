export interface RhythmResult {
  status: "completed" | "interrupted";
  demo: boolean;
  songId: string;
  songTitle: string;
  accuracy: number;
  score: number;
  maxCombo: number;
  perfect: number;
  good: number;
  ok: number;
  miss: number;
  totalNotes: number;
}

export interface RhythmController {
  destroy(): void;
}

export function mountRhythm(
  root: ShadowRoot,
  options: {
    signal: AbortSignal;
    onExit(): void;
    onResult?(result: RhythmResult): void;
  }
): Promise<RhythmController>;
