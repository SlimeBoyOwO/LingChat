from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional

@dataclass
class GameState:
    """存储游戏世界中的所有变量，如玩家姓名、NPC好感度等。"""
    variables: Dict[str, Any] = field(default_factory=dict)

    def get(self, key: str, default: Any = None) -> Any:
        return self.variables.get(key, default)

    def set(self, key: str, value: Any):
        self.variables[key] = value

@dataclass
class ProgressPointer:
    """指向当前剧情单元和事件的指针。"""
    current_unit_id: str
    last_completed_event_index: int = -1

@dataclass
class GameProgress:
    """存储游戏的元数据和进度。"""
    save_name: str
    story_pack_id: str
    last_saved_timestamp: str
    pointer: ProgressPointer
    runtime_state: str = "ExecutingEvents"
    context: Dict[str, Any] = field(default_factory=dict)

@dataclass
class Character:
    """代表一个AI角色。"""
    role_id: str
    name: str
    prompt: str

@dataclass
class Player:
    """代表玩家角色。"""
    name: str
    prompt: Optional[str] = ""

@dataclass
class StoryEvent:
    """所有事件类的基类。"""
    raw_data: Dict[str, Any]

@dataclass
class NarrationEvent(StoryEvent):
    content: str
    mode: str

@dataclass
class DialogueEvent(StoryEvent):
    character_id: str
    content: str
    mode: str 

@dataclass
class StoryUnit:
    """代表一个剧情单元文件 (.yaml) 的内容。"""
    unit_id: str
    events: List[Dict[str, Any]]
    end_condition: Optional[Dict[str, Any]]

@dataclass
class StoryPack:
    """代表一个完整的剧本包。"""
    pack_id: str
    start_unit_id: str
    character_roles: List[str]
    dm_role_id: Optional[str] = None

@dataclass
class GameSession:
    """一个总的容器，包含一次游戏会话的所有数据。"""
    save_path: str
    game_state: GameState
    game_progress: GameProgress
    dialogue_history: List[Dict[str, Any]]
    story_pack_config: StoryPack
    story_units: Dict[str, StoryUnit]
    characters: Dict[str, Character]
    player: Player