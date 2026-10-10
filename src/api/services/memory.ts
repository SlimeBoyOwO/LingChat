import { invoke } from "@tauri-apps/api/core";

/** 有记忆的角色摘要（浏览面板的角色下拉选项）。 */
export interface MemoryRoleSummary {
  role_id: number;
  /** 角色名；后端查不到角色行时为空串，前端回退成「ID N」。 */
  name: string;
  /** 可浏览条目数：长期记忆 + 旧版整合条目（共用表统一视图）。 */
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

// ── 旧版存档记忆一键整合（存储在每「剧本 × 角色」独立归档表中） ──

/** 一键导入旧版存档记忆的结果汇总。 */
export interface LegacyImportResult {
  /** 旧表中读到的（已按存档×角色归约后的）记忆行数。 */
  legacy_rows: number;
  /** 新建档案的角色数（每个角色 = 一个剧本 × 角色 组合）。 */
  imported_roles: number;
  /** 目录表里已存在、本次跳过未重复导入的角色数。 */
  skipped_existing: number;
  /** 实际写入的整合记忆条目数（按分段计）。 */
  memories_written: number;
  /** 内容为空、未建档案的角色数。 */
  skipped_empty: number;
  /** 无法归属到角色的行数（role_id 为空）。 */
  skipped_unassigned: number;
}

/** 档案目录摘要（设置页列表展示）。 */
export interface MemoryArchiveSummary {
  id: number;
  role_id: number;
  role_name: string;
  script_key: string | null;
  script_role_key: string | null;
  table_name: string;
  entry_count: number;
  created_at: number;
  updated_at: number;
}

/** 档案内的一条整合记忆。 */
export interface MemoryArchiveEntry {
  id: number;
  section: string;
  content: string;
  created_at: number;
}

/** 一键把旧版 `memory_bank` 的四段式记忆整合到「剧本 × 角色」独立归档表。 */
export function importLegacyMemories(): Promise<LegacyImportResult> {
  return invoke("memory_import_legacy");
}

/** 列出所有已导入的「剧本 × 角色」记忆档案（最新在前）。 */
export function listMemoryArchives(): Promise<MemoryArchiveSummary[]> {
  return invoke("memory_archive_list");
}

/** 查看某个档案里的全部整合记忆条目。 */
export function getMemoryArchive(profileId: number): Promise<MemoryArchiveEntry[]> {
  return invoke("memory_archive_get", { profileId });
}

/** 删除某个档案（连同数据表），返回是否真的删除了。 */
export function deleteMemoryArchive(profileId: number): Promise<boolean> {
  return invoke("memory_archive_delete", { profileId });
}

// ── 本地嵌入模型（设置页「嵌入模型」子页：从本地导入 / 删除，实时生效） ──

/** 本地嵌入模型单个部件的状态。 */
export interface EmbeddingLocalFile {
  /** 文件名（如 model_int4.onnx）。 */
  name: string;
  /** 是否必需（缺失即判定未安装）。 */
  required: boolean;
  /** 是否已存在。 */
  present: boolean;
  /** 文件字节数（不存在时 0）。 */
  size_bytes: number;
}

/** 本地嵌入模型状态快照。 */
export interface EmbeddingLocalStatus {
  installed: boolean;
  /** 模型存放目录。 */
  dir: string;
  model_name: string;
  /** 当前检索模式：hybrid（向量 + 词法）或 lexical（纯词法）。 */
  retrieval_mode: string;
  /** 是否配置了远程 embeddings 接口。 */
  remote_configured: boolean;
  files: EmbeddingLocalFile[];
}

/** 读取本地嵌入模型状态。 */
export function getEmbeddingLocalStatus(): Promise<EmbeddingLocalStatus> {
  return invoke("embedding_local_status");
}

/** 从本地文件夹或 zip / 7z 压缩包导入本地嵌入模型（导入后立即生效，无需重启）。 */
export function importEmbeddingLocal(sourcePath: string): Promise<EmbeddingLocalStatus> {
  return invoke("embedding_local_import", { sourcePath });
}

/** 删除已安装的本地嵌入模型（删除后立即回退词法检索）。 */
export function deleteEmbeddingLocal(): Promise<EmbeddingLocalStatus> {
  return invoke("embedding_local_delete");
}
