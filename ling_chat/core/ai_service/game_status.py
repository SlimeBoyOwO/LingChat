from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any
from ling_chat.core.ai_service.type import Player, GameRole
from ling_chat.core.ai_service.role_manager import RoleManager

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
    role_manager: RoleManager = field(default_factory=RoleManager)
    # 记录当前对话角色，此角色将作为LLM传输入的对象，使用本角色的记忆
    current_character: GameRole = field(default_factory=GameRole)
    
    # [新增] 在场角色列表：只有在场的角色才能感知到台词
    # 使用 Set 避免重复
    present_roles: set[GameRole] = field(default_factory=set)

    # 背景信息
    background: str = field(default_factory=str)
    # BGM信息
    background_music: str = field(default_factory=str)
    # 背景特效
    background_effect: str = field(default_factory=str)

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
        self.role_manager.refresh_memories_from_lines(self.line_list)