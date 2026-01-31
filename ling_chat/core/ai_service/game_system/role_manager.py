from typing import Optional, List, Dict, Set

from ling_chat.game_database.models import GameLine
from ling_chat.core.ai_service.game_system.memory_builder import MemoryBuilder
from ling_chat.core.ai_service.game_system.persistent_memory_system import PersistentMemorySystem
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.core.ai_service.type import GameRole, GameMemoryBank
from ling_chat.core.ai_service.exceptions import RoleNotFoundError
from ling_chat.core.logger import logger
from ling_chat.game_database.managers.memory_manager import MemoryManager

class GameRoleManager:
    """
    角色运行时管理器：维护当前活跃角色的内存状态。
    """
    def __init__(self):
        # 简单直接：这就是一个存活角色的缓存
        self.loaded_roles: Dict[int, GameRole] = {}
        # role_id -> PersistentMemorySystem
        self._memory_bank_systems: Dict[int, PersistentMemorySystem] = {}

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
            slice_start_idx = 0
            system_addendum = ""
            short_term_prefix = ""
            try:
                mb = self._get_memory_bank_system(role)
                # 只有用户显式开启永久记忆时才触发自动压缩
                if mb.is_enabled():
                    mb.check_and_trigger_auto_update(source_lines)
                    slice_start_idx = mb.get_slice_start_index()
                    system_addendum = mb.get_system_memory_text()
                    short_term_prefix = mb.get_short_term_user_text()
            except Exception as e:
                logger.error(f"MemoryBank: role_id={rid} 初始化/更新失败: {e}", exc_info=True)

            # 构建记忆（裁剪历史窗口，避免上下文无限膨胀）
            sliced_lines = source_lines[slice_start_idx:] if slice_start_idx > 0 else source_lines
            builder = MemoryBuilder(target_role_id=rid)
            built = builder.build(sliced_lines)
            role.memory = self._merge_memory_bank_into_context(built, system_addendum, short_term_prefix)

        # 【重要改动】去掉了自动删除 "stale" 角色的逻辑。
        # 角色加载后通常应该保留直到场景结束，频繁删除重建会浪费算力。
        # 如果真的需要清理内存，应该提供一个显式的 .clear_cache() 方法。

    def _get_memory_bank_system(self, role: GameRole) -> PersistentMemorySystem:
        if role.role_id is None:
            raise ValueError("role.role_id 为空，无法初始化 PersistentMemorySystem")
        rid = role.role_id
        if rid not in self._memory_bank_systems:
            self._memory_bank_systems[rid] = PersistentMemorySystem(role)
        return self._memory_bank_systems[rid]

    def _merge_memory_bank_into_context(self, memory: List[Dict], system_addendum: str, short_term_prefix: str) -> List[Dict]:
        """
        将 MemoryBank 合并进“唯一的 system 消息”，避免出现多条 system。
        - system_addendum：长期记忆/用户画像/约定（合并到 system.content 末尾）
        - short_term_prefix：近期回顾（更适合作为 user.content 的前缀）
        """
        out = list(memory)

        if system_addendum.strip():
            # 合并到第一条 system
            if out and out[0].get("role") == "system":
                content = out[0].get("content", "") or ""
                if system_addendum not in content:
                    out[0]["content"] = f"{content}{system_addendum}"
            else:
                out = [{"role": "system", "content": system_addendum}] + out

        if short_term_prefix.strip():
            # 尽量合并进第一条 user
            for i in range(len(out)):
                if out[i].get("role") == "user":
                    content = out[i].get("content", "") or ""
                    if not content.startswith(short_term_prefix):
                        out[i]["content"] = f"{short_term_prefix}{content}"
                    break
            else:
                # 没有 user 消息时，插入一条 user
                out.append({"role": "user", "content": short_term_prefix.strip()})

        # 移除连续重复的 system
        cleaned: List[Dict] = []
        for msg in out:
            if cleaned and cleaned[-1].get("role") == "system" and msg.get("role") == "system":
                cleaned[-1]["content"] = (cleaned[-1].get("content", "") or "") + "\n" + (msg.get("content", "") or "")
            else:
                cleaned.append(msg)

        return cleaned

    # DB 交互
    def load_memory_banks_from_db(self, save_id: int, role_ids: Optional[List[int]] = None) -> None:
        """
        载入某个存档下的 MemoryBank 到运行时缓存（GameRole.memory_bank）。
        仅应在“载入存档”时调用。
        """
        try:
            memories = MemoryManager.get_memories(save_id=save_id, role_id=None)
            by_role: Dict[int, Dict] = {}
            for m in memories:
                if m.role_id is None:
                    continue
                # 取最新
                if m.role_id not in by_role or (m.id or 0) > (by_role[m.role_id].get("_id", 0)):
                    by_role[m.role_id] = {"_id": m.id or 0, "info": m.info or {}}

            target_ids = set(role_ids) if role_ids else set(by_role.keys())
            for rid in target_ids:
                role = self.get_role(rid)
                info = by_role.get(rid, {}).get("info")
                if info:
                    role.memory_bank = GameMemoryBank.model_validate(info)
                else:
                    role.memory_bank = GameMemoryBank()  # 重置为默认
        except Exception as e:
            logger.error(f"load_memory_banks_from_db 失败: {e}", exc_info=True)

    def persist_memory_banks_to_db(self, save_id: int, role_ids: Optional[List[int]] = None) -> None:
        """
        将运行时缓存（GameRole.memory_bank）写入 DB。
        仅应在“创建/保存存档”时调用，避免玩家不存档时污染数据库。
        """
        try:
            target_ids = role_ids if role_ids else list(self.loaded_roles.keys())
            for rid in target_ids:
                role = self.loaded_roles.get(rid)
                if not role or role.role_id is None:
                    continue
                MemoryManager.upsert_memory(
                    save_id=save_id,
                    role_id=role.role_id,
                    info=role.memory_bank.model_dump(),
                    memory_id=None,
                )
        except Exception as e:
            logger.error(f"persist_memory_banks_to_db 失败: {e}", exc_info=True)

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