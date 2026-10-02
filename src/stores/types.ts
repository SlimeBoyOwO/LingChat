export interface GameSettings {
  volume: number;
  textSpeed: "slow" | "normal" | "fast";
  autoPlay: boolean;
}

export interface SceneData {
  sceneId: string;
  timestamp: number;
}

export interface CharacterState {
  position: "left" | "right" | "center";
  emotion: string;
}
