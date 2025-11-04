from dataclasses import dataclass, field
from typing import Any

@dataclass
class Script:
    name: str
    description: str
    intro_charpter: str
    settings: dict

@dataclass
class Character:
    character_id: str
    settings: dict
    resource_path: str
    prompt: dict
    memory: list[dict]

@dataclass
class Charpter:
    charpter_id: str
    events: list[dict]
    ends: dict

@dataclass
class Player:
    user_name: str = ""
    user_subtitle: str = ""

def create_default_player():
    return Player()

@dataclass
class GameContext:
    """
    存储所有运行时共享的游戏状态。
    """
    player: Player = field(default_factory=create_default_player)
    dialogue: list[dict] = field(default_factory=list[dict])

    # 角色列表 - 局部信息，只把记忆信息传输给需要的小比崽子
    # key 是 character_id, value 是 Character 对象
    characters: dict[str, Character] = field(default_factory=dict)

    # 背景信息
    background: str = field(default_factory=str)
    
    # 故事变量 - 玩家可以通过事件来修改和查询
    variables: dict[str, Any] = field(default_factory=dict)
    
    def get_character(self, character_id: str) -> Character | None:
        """提供一个便捷的方法来安全地获取角色。"""
        return self.characters.get(character_id)