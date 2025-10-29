export * from "./api";
export * from "./ui";
export * from "./websocket";
export * from "./script";
export type {
  ChatMessage,
  Conversation,
  ChatManagerConfig,
  EmotionConfig,
  EmotionData,
} from "./chat";

// 通用类型定义
export interface EventCallback<T = any> {
  (data: T): void;
}

export interface EventListeners<T = any> {
  [event: string]: EventCallback<T>[];
}

export type Nullable<T> = T | null;
export type Optional<T> = T | undefined;
