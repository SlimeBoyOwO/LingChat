import { invoke } from "@tauri-apps/api/core";
import type { AffectionVector, NegativeVector } from "@/stores/modules/game/state";

/** 角色情感状态：总好感度 + 好感六维（flatten）+ 负面六维（对应 Rust AffectionState） */
export interface AffectionState extends AffectionVector {
  /** 总好感度 = 好感六维平均（后端派生字段，随六维自动同步） */
  total: number;
  negative: NegativeVector;
}

/** affection:changed 事件负载（snake_case，对应 Rust AffectionChangedPayload） */
export interface AffectionChangedPayload {
  role_id: number;
  /** 本次实际发生变化的好感维度增量（键名为维度序列化键） */
  deltas: Record<string, number>;
  /** 本次实际发生变化的负面维度增量 */
  negative_deltas: Record<string, number>;
  /** 调整后的完整好感六维数值 */
  values: AffectionVector;
  /** 调整后的完整负面六维数值 */
  negative: NegativeVector;
  /** 好感六维平均 */
  average: number;
  /** 上帝 Agent 给出的调整理由 */
  reason: string;
}

/**
 * 全量查询所有已加载角色的当前情感状态（role_id 字符串键 → 好感六维 + 负面六维）。
 * 好感度变更由后端主动广播 affection:changed，本命令只用于初始化/兜底刷新。
 */
export const getAffection = async (): Promise<Record<string, AffectionState>> => {
  return invoke<Record<string, AffectionState>>("get_affection");
};
