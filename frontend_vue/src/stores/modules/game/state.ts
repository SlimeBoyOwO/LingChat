export interface DialogMessage {
  type: "message" | "reply";
  content: string;
  emotion?: string;
  audioFile?: string;
  isFinal?: boolean;
  motionText?: string;
  originalTag?: string;
  timestamp?: number;
}

export interface GameState {
  currentScene: string;
  avatar: {
    character_id: number;
    character_name: string;
    character_subtitle: string;
    user_name: string;
    user_subtitle: string;
    think_message: string;
    emotion: string;
    originEmotion: string;
  };
  currentLine: string;
  currentStatus: "input" | "thinking" | "responding";
  dialogHistory: DialogMessage[];
}

export const state: GameState = {
  currentScene: "none",
  avatar: {
    character_id: 0,
    emotion: "正常",
    character_name: "钦灵",
    character_subtitle: "Slime Studio",
    user_name: "Lovely You",
    user_subtitle: "Bibilibi",
    think_message: "灵灵正在思考中",
    originEmotion: "",
  },
  currentLine: "",
  currentStatus: "input",
  dialogHistory: [],
};
