from typing import Optional, Dict, List, Any
from datetime import datetime
from sqlmodel import Field, SQLModel, Relationship
from sqlalchemy import JSON, Column

# ==========================================
# 模型定义 (Entities)
# ==========================================

class Role(SQLModel, table=True):
    """角色表：存储主要角色的元数据"""
    __tablename__ = "role"
    
    id: Optional[int] = Field(default=None, primary_key=True)
    name: str = Field(index=True)
    resource_folder: str  # 资源路径

class UserInfo(SQLModel, table=True):
    """用户表"""
    __tablename__ = "user_info"
    
    id: Optional[int] = Field(default=None, primary_key=True)
    username: str = Field(unique=True, index=True)
    password: str
    # 外键：最后使用的角色
    last_character_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
    # 外键：最后使用的存档
    last_save_id: Optional[int] = Field(default=None, foreign_key="save.id", nullable=True)

class RunningScript(SQLModel, table=True):
    """运行剧本表：记录某个存档下的剧本运行状态"""
    __tablename__ = "running_script"
    
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
    
    # 角色逻辑 (也可以放在这里，或者根据需要拆分)
    role_id: Optional[int] = Field(default=None, nullable=True)
    script_role_id: Optional[str] = None
    display_name: Optional[str] = None

# 2. 定义数据库模型 (Table)
class Line(LineBase, table=True):
    __tablename__ = "line"
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # 在 DB 模型中，save_id 是必须的，且也是外键
    save_id: int = Field(foreign_key="save.id") 
    parent_line_id: Optional[int] = Field(default=None, foreign_key="line.id")

class Save(SQLModel, table=True):
    """存档表"""
    __tablename__ = "save"
    
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
    __tablename__ = "memory_bank"
    
    id: Optional[int] = Field(default=None, primary_key=True)
    
    # 记忆信息：JSON 存储
    info: Dict[str, Any] = Field(default={}, sa_column=Column(JSON))
    
    save_id: int = Field(foreign_key="save.id")
    role_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
    script_role_id: Optional[str] = Field(default=None, nullable=True)
