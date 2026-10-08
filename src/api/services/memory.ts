import { invoke } from "@tauri-apps/api/core";

/** 有长期记忆的角色摘要（浏览面板的角色下拉选项）。 */
export interface MemoryRoleSummary {
  role_id: number;
  /** 角色名；后端查不到角色行时为空串，前端回退成「ID N」。 */
  name: string;
  /** 该角色的记忆条数（按 memory_id 去重）。 */
  count: number;
}

/** 一条长期记忆（分块已聚合、标签已从正文剥离）。 */
export interface MemoryItem {
  memory_id: string;
  content: string;
  tags: string[];
  /** 创建时间（Unix 毫秒）。 */
  created_at: number;
}

/** 列出存在长期记忆的角色（按 role_id 升序）。 */
export function listMemoryRoles(): Promise<MemoryRoleSummary[]> {
  return invoke("memory_list_roles");
}

/** 列出某角色的全部记忆（最新在前）。 */
export function listMemories(roleId: number): Promise<MemoryItem[]> {
  return invoke("memory_list", { roleId });
}

/** 删除某角色的一条记忆，返回被删除的行数。 */
export function deleteMemory(roleId: number, memoryId: string): Promise<number> {
  return invoke("memory_delete", { roleId, memoryId });
}
