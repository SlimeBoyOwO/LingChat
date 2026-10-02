// 聊天相关类型定义

export interface ChatMessage {
  id: string;
  content: string;
  timestamp: number;
  sender: "user" | "assistant";
  type?: "text" | "image" | "file";
  metadata?: Record<string, any>;
}

export interface Conversation {
  id: string;
  title: string;
  messages: ChatMessage[];
  createdAt: number;
  updatedAt: number;
}

export interface ChatManagerConfig {
  connection: any; // WebSocket connection
  historyManager: any; // History manager instance
}

export interface EmotionConfig {
  emotions: Record<string, EmotionData>;
  defaultEmotion: string;
}

export interface EmotionData {
  name: string;
  icon: string;
  color: string;
  description?: string;
}
