from dataclasses import dataclass, field
from datetime import datetime
from typing import Optional, List, Dict, Any
from ling_chat.core.ai_service.type import Player, GameRole, ScriptStatus
from ling_chat.core.ai_service.game_system.role_manager import GameRoleManager

from ling_chat.game_database.models import GameLine, LineBase

@dataclass
class GameStatus:
    """
    存储所有运行时共享的游戏状态。
    """
    player: Player = field(default_factory=Player)

    # 记录台词列表，用于记忆构建和历史记忆
    line_list: list[GameLine] = field(default_factory=list[GameLine])

    # 使用 RoleManager 管理所有角色
    role_manager: GameRoleManager = field(default_factory=GameRoleManager)
    # 记录当前对话角色，此角色将作为LLM传输入的对象，使用本角色的记忆
    current_character: Optional[GameRole] = None
    # 在场角色列表：只有在场的角色才能感知到台词
    present_roles: set[GameRole] = field(default_factory=set)
    # 游戏主角，也就是导入的游戏角色，剧本模式冒险的主角
    main_role: Optional[GameRole] = None

    # 背景信息
    background: str = field(default_factory=str)
    # BGM信息
    background_music: str = field(default_factory=str)
    # 背景特效
    background_effect: str = field(default_factory=str)
    # 全局变量信息
    global_variables: Dict[str, Any] = field(default_factory=dict)
    # 记录已经玩过的剧本
    completed_scripts: set[str] = field(default_factory=set)
    # 最后一次对话时间记录
    last_dialog_time: Optional[datetime] = None

    # 剧本模式中记录的额外信息
    script_status: Optional[ScriptStatus] = None

    # 当前激活的存档ID（用于 MemoryBank 持久化/载入/自动压缩）
    active_save_id: Optional[int] = None

    def add_line(self, line: LineBase):
        # 转换为GameLine
        game_line = GameLine(
            **line.model_dump(),
            perceived_role_ids=[role.role_id for role in self.present_roles if role.role_id is not None]  # 添加GameLine特有的属性
        )

        # TODO: 根据性能优化台词的更新频率，目前每条台词都更新
        self.line_list.append(game_line)
        self.refresh_memories()
    
    def refresh_memories(self):
        # 自动压缩只写入运行时缓存，不触发 DB
        self.role_manager.sync_memories(self.line_list)

    # ============ 全局变量便捷方法 ============

    def set_variable(self, key: str, value: Any):
        """设置全局变量（用于冒险剧情中的条件判断、分支记录等）"""
        self.global_variables[key] = value

    def get_variable(self, key: str, default: Any = None) -> Any:
        """获取全局变量"""
        return self.global_variables.get(key, default)

    def get_chat_message_count(self) -> int:
        """获取当前对话中非系统消息的数量（用于羁绊冒险解锁条件检测）"""
        from ling_chat.game_database.models import LineAttribute
        return len([l for l in self.line_list if l.attribute != LineAttribute.SYSTEM])
