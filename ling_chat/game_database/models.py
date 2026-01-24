from typing import Optional, Dict, List, Any
from datetime import datetime
from enum import Enum
from sqlmodel import Field, SQLModel, Relationship
from sqlalchemy import JSON, Column
from sqlalchemy import or_, select

# ==========================================
# 枚举定义
# ==========================================

class RoleType(str, Enum):
    MAIN = "main"       # 主要角色 (AI驱动，有完整记忆)
    NPC = "npc"         # 剧本角色 (通常也是AI驱动，或者是固定的，视需求而定)
    SYSTEM = "system"   # 旁白、系统提示等

# ==========================================
# 关联表 (Join Tables)
# ==========================================

class LinePerception(SQLModel, table=True):
    """感知表：记录某句台词被哪些角色感知到了"""
    __tablename__ = "line_perception" # type: ignore
    line_id: Optional[int] = Field(default=None, foreign_key="line.id", primary_key=True)
    role_id: Optional[int] = Field(default=None, foreign_key="role.id", primary_key=True)

# ==========================================
# 模型定义 (Entities)
# ==========================================

class Role(SQLModel, table=True):
    """角色表：统一存储所有出现的实体（主角、NPC、旁白）"""
    __tablename__ = "role" # type: ignore
    
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # 剧本中的标识符 (例如 "narrator", "hero_01")
    # 不强制唯一，因为不同剧本可能复用Key，需结合上下文处理
    script_key: Optional[str] = Field(default=None, index=True) 
    
    name: str = Field(index=True)
    role_type: RoleType = Field(default=RoleType.NPC) # 默认为NPC
    
    resource_folder: Optional[str] = None # 资源路径 (NPC也可以有头像)
    
    # 一个角色可以感知多句台词，这个字段一般是不会用到的，用了会导致几千个台词被查找
    # perceived_lines: List["Line"] = Relationship(back_populates="perceived_by", link_model=LinePerception)

class UserInfo(SQLModel, table=True):
    """用户表"""
    __tablename__ = "user_info" # type: ignore
    
    id: Optional[int] = Field(default=None, primary_key=True)
    username: str = Field(unique=True, index=True)
    password: str
    
    # 外键：最后使用的角色
    last_character_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
    # 外键：最后使用的存档
    last_save_id: Optional[int] = Field(default=None, foreign_key="save.id", nullable=True)

class RunningScript(SQLModel, table=True):
    """运行剧本表：记录某个存档下的剧本运行状态"""
    __tablename__ = "running_script" # type: ignore
    
    id: Optional[int] = Field(default=None, primary_key=True)
    script_folder: str = Field(index=True) # 剧本Key/文件夹名
    
    # 变量信息：使用 JSON 存储动态数据
    variable_info: Dict[str, Any] = Field(default={}, sa_column=Column(JSON))
    
    current_chapter: str
    event_sequence: int
    
    # 外键：归属的存档
    save_id: int = Field(foreign_key="save.id")

# 1. 定义基础字段 (Base)
class LineBase(SQLModel):
    id: Optional[int] = Field(default=None)

    content: str
    original_emotion: Optional[str] = None
    predicted_emotion: Optional[str] = None
    tts_content: Optional[str] = None
    action_content: Optional[str] = None
    audio_file: Optional[str] = None
    attribute: str
    
    # 缓存用的感知列表 (JSON存储)，用于 GameStatus 缓存和 SaveManager 持久化参考
    # perceived_role_ids: List[int] = Field(default_factory=list, sa_column=Column(JSON))
    
    # 【变更】统一说话人字段
    # 不再区分 role_id 和 script_role_id
    sender_role_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
    
    # 仍然保留 display_name，因为剧本里同一个角色可能显示不同的名字（例如 "神秘人" -> "王大锤"）
    display_name: Optional[str] = None

class GameLine(LineBase):
    """
    运行时对象：在内存中流转的剧本行。
    """
    # 显式定义 ID 列表，类型清晰
    perceived_role_ids: List[int] = []

# 2. 定义数据库模型 (Table)
class Line(LineBase, table=True):
    __tablename__ = "line" # type: ignore
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # 在 DB 模型中，save_id 是必须的，且也是外键
    save_id: int = Field(foreign_key="save.id") 
    parent_line_id: Optional[int] = Field(default=None, foreign_key="line.id")
    
    # 定义关系：这句台词被谁感知了
    perceived_by: List["Role"] = Relationship(link_model=LinePerception)

class Save(SQLModel, table=True):
    """存档表"""
    __tablename__ = "save" # type: ignore
    
    id: Optional[int] = Field(default=None, primary_key=True)
    title: str
    status: Dict[str, Any] = Field(default={}, sa_column=Column(JSON)) # 存档状态，记录背景图片，音频等信息
    
    create_date: datetime = Field(default_factory=datetime.now)
    update_date: datetime = Field(default_factory=datetime.now)
    
    user_id: int = Field(foreign_key="user_info.id")
    
    # 指针：指向当前激活的剧本状态
    running_script_id: Optional[int] = Field(default=None, foreign_key="running_script.id", nullable=True)
    
    # 指针：指向最后一条对话 (用于快速恢复上下文)
    # 注意：这里引用了 line 表的 id
    last_message_id: Optional[int] = Field(default=None, foreign_key="line.id", nullable=True)
    main_role_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)

class MemoryBank(SQLModel, table=True):
    """记忆仓库表"""
    __tablename__ = "memory_bank" # type: ignore
    
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # 记忆信息：JSON 存储
    info: Dict[str, Any] = Field(default={}, sa_column=Column(JSON))
    
    save_id: int = Field(foreign_key="save.id")
    
    # 统一使用 role_id
    role_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
