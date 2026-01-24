from dataclasses import dataclass, field
from typing import Optional, List, Dict, Any

@dataclass
class GameRole:
    """
    游戏角色数据模型
    """
    role_id: Optional[int] = None
    script_role_id: Optional[str] = None
    memory: List[Dict[str, Any]] = field(default_factory=list)
    
    display_name: Optional[str] = None
    settings: dict = field(default_factory=dict)
    resource_path: Optional[str] = None
    prompt: Optional[str] = None
    memory_bank: dict = field(default_factory=dict)

@dataclass
class Player:
    user_name: str = ""
    user_subtitle: str = ""
