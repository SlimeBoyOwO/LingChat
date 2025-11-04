export interface DialogMessage {
  type: "message" | "reply";
  character: string;
  content: string;
  emotion?: string;
  audioFile?: string;
  isFinal?: boolean;
  motionText?: string;
  originalTag?: string;
  timestamp?: number;
}

export interface ScriptInfo {
  script_name: string;
  script_characters: Map<string, ScriptCharacter>; // 用角色名作为key
}

export interface ScriptCharacter {
  character_id?: string | number;
  character_name: string;
  character_subtitle: string;
  think_message: string;
  emotion: string;
  originEmotion: string;
  scale: number;
  offset_y: number;
  offset_x: number;
  bubble_top: number;
  bubble_left: number;
  show: boolean;
}

export interface GameState {
  currentScene: string;
  script: ScriptInfo;
  character: string;
  avatar: ScriptCharacter & {
    user_name: string;
    user_subtitle: string;
  };
  currentLine: string;
  currentStatus: "input" | "thinking" | "responding" | "presenting";
  dialogHistory: DialogMessage[];
}

export const state: GameState = {
  currentScene: "none",
  script: {
    script_name: "none",
    script_characters: new Map(),
  },
  character: "default",
  avatar: {
    character_id: 0,
    emotion: "正常",
    character_name: "钦灵",
    character_subtitle: "Slime Studio",
    user_name: "Lovely You",
    user_subtitle: "Bibilibi",
    think_message: "灵灵正在思考中",
    originEmotion: "",
    scale: 1,
    offset_y: 0,
    offset_x: 0,
    bubble_top: 5,
    bubble_left: 20,
    show: true,
  },
  currentLine: "",
  currentStatus: "input",
  dialogHistory: [],
};
