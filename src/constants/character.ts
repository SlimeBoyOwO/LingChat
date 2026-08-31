/**
 * 角色相关常量。
 *
 * 与后端 `RoleRepo::SYSTEM_PROTECTED_ROLE_IDS` ([src-tauri/src/db/managers/role_repo.rs](src-tauri/src/db/managers/role_repo.rs))
 * 保持同步——两边手动对齐，避免跨语言共享配置的复杂度。
 *
 * 这些 ID 是系统角色，禁止删除：
 * - 0: User 角色（玩家本体）
 * - 1: 默认 main 角色（启动兜底）
 * - 2: 预留系统角色位
 */
export const SYSTEM_PROTECTED_ROLE_IDS: readonly number[] = [0, 1, 2] as const

/** 检查给定的角色 ID 是否为系统保护角色（禁止删除）。 */
export const isSystemProtectedRole = (roleId: number): boolean =>
  SYSTEM_PROTECTED_ROLE_IDS.includes(roleId)
