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
  snapshot(): {
    state: string;
    demo?: boolean;
    songId?: string;
    songTitle?: string;
    noteCount?: number;
    section?: string;
    effectCount?: number;
    renderingAudio?: boolean;
    time?: number;
    result?: RhythmResult;
    held?: number[];
    audioState?: AudioContextState;
  };
}

export function mountRhythm(
  root: ShadowRoot,
  options: {
    signal: AbortSignal;
    onExit(): void;
    onResult?(result: RhythmResult): void;
  }
): Promise<RhythmController>;
