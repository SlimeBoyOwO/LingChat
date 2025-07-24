// UI相关类型定义

export interface DOMElements {
  canvas?: HTMLCanvasElement;
  chatContainer?: HTMLElement;
  messageInput?: HTMLInputElement;
  sendButton?: HTMLButtonElement;
  menuButton?: HTMLButtonElement;
  historyPanel?: HTMLElement;
}

export interface UIState {
  isMenuOpen: boolean;
  isHistoryOpen: boolean;
  currentTheme: "light" | "dark";
  isLoading: boolean;
}

export interface StarFieldConfig {
  starCount: number;
  speed: number;
  color: string;
  size: number;
}

export interface MenuItem {
  id: string;
  label: string;
  icon?: string;
  action: () => void;
  disabled?: boolean;
}

export interface SaveData {
  conversations: any[];
  settings: Record<string, any>;
  timestamp: number;
}
