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
