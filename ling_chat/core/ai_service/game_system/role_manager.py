from typing import Optional, List, Dict, Set

from ling_chat.game_database.models import GameLine
from ling_chat.core.ai_service.game_system.memory_builder import MemoryBuilder
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.core.ai_service.type import GameRole
from ling_chat.core.ai_service.exceptions import RoleNotFoundError
from ling_chat.core.logger import logger

class GameRoleManager:
    """
    角色运行时管理器：维护当前活跃角色的内存状态。
    """
    def __init__(self):
        # 简单直接：这就是一个存活角色的缓存
        self.loaded_roles: Dict[int, GameRole] = {}

    def get_role(self, role_id: int) -> GameRole:
        """获取角色，如果不存在则在内存中初始化一个空的"""
        if role_id not in self.loaded_roles:
            self.loaded_roles[role_id] = GameRole(role_id=role_id)
        return self.loaded_roles[role_id]

    def get_role_by_script_keys(self, script_key: str, script_role_key: str) -> GameRole:
        """
        通过 script_key 获取运行时角色。
        """
        role = RoleManager.get_role_by_script_keys(script_key, script_role_key)
        if not role or not role.id:
            error_msg = f"数据库中未找到角色：script_key={script_key}, script_role_key={script_role_key}，说明本角色所属剧本未初始化"
            logger.error(error_msg)
            raise RoleNotFoundError(error_msg)
            
        return self.get_role(role.id)

    def sync_memories(self, lines: List[GameLine], recent_n: Optional[int] = None):
        """
        根据台词同步角色的状态和记忆。
        """
        # 1. 确定数据源
        source_lines = lines[-recent_n:] if recent_n else lines

        # 2. 收集涉及到的 Role ID
        involved_role_ids: Set[int] = set()
        for line in source_lines:
            if line.sender_role_id:
                involved_role_ids.add(line.sender_role_id)
            # 假设 perceived_role_ids 是整数列表
            involved_role_ids.update(line.perceived_role_ids)

        # 3. 仅更新活跃角色的记忆
        for rid in involved_role_ids:
            role = self.get_role(rid)
            
            # 同步名字 (复用逻辑，提取为独立动作)
            self._sync_display_name(role, source_lines)
            
            # 构建记忆
            builder = MemoryBuilder(target_role_id=rid)
            role.memory = builder.build(source_lines)

        # 【重要改动】去掉了自动删除 "stale" 角色的逻辑。
        # 角色加载后通常应该保留直到场景结束，频繁删除重建会浪费算力。
        # 如果真的需要清理内存，应该提供一个显式的 .clear_cache() 方法。

    def _sync_display_name(self, role: GameRole, lines: List[GameLine]):
        """辅助方法：从最近的台词中更新显示名称"""
        # 倒序查找该角色说的最后一句话
        for line in reversed(lines):
            if line.sender_role_id == role.role_id and line.display_name:
                role.display_name = line.display_name
                break

    def _db_ensure_role_exists(self, script_key: str, script_role_key: str) -> int:
        """
        DB 辅助方法：确保角色在数据库存在 (保持原有逻辑，但名字更清晰)
        """
        role = RoleManager.get_role_by_script_keys(script_key, script_role_key)
        if role is None:
            # 如果没有角色可以创建，但一般来讲应该是报错因为角色应该被初始化才对
            raise ValueError(f"角色 {script_key} 不存在")
        else:
            if role.id is None:
                raise ValueError(f"角色 {script_key} 的 ID 为空")
            return role.id