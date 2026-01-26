from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any

@dataclass
class GameRole:
    """
    游戏角色数据模型
    """
    role_id: Optional[int] = None
    # script_role_id removed in favor of unified Role mapping
    # script_role_id: Optional[str] = None
    memory: List[Dict[str, Any]] = field(default_factory=list)
    
    display_name: Optional[str] = None
    settings: dict = field(default_factory=dict)
    resource_path: Optional[str] = None
    prompt: Optional[str] = None
    memory_bank: dict = field(default_factory=dict)
    
    def __hash__(self):
        # Hash based on role_id if present, otherwise id of object
        return hash(self.role_id) if self.role_id is not None else id(self)
    
    def __eq__(self, other):
        if not isinstance(other, GameRole):
            return False
        if self.role_id is None or other.role_id is None:
            return self is other
        return self.role_id == other.role_id

@dataclass
class Player:
    user_name: str = ""
    user_subtitle: str = ""
    user_prompt: str = "" # 用于设定玩家的信息，如性格、喜好等

@dataclass
class ScriptStatus:
    folder_key: str

    name: str
    description: str
    intro_charpter: str
    settings: dict

    # 正在进行剧本模式的 client
    running_client_id: Optional[str] = None

    # 记录剧本进度
    current_charpter_key: str = field(default_factory=str)
    current_event_process: int = field(default_factory=int)

    # 剧本包含的变量
    vars: dict = field(default_factory=dict)
