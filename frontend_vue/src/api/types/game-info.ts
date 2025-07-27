// types/ai-engine.ts
export interface AIEngineResponse {
  status: "success" | "pending" | "failed";
  data?: any;
  error?: string;
  requestId: string;
}
