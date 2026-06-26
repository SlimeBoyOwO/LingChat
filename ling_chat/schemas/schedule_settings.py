from typing import Any, Dict, List, Optional

from pydantic import BaseModel


class ScheduleItem(BaseModel):
    name: str
    time: str
    content: str


class ScheduleGroup(BaseModel):
    title: str
    description: str
    items: List[ScheduleItem]


class TodoItem(BaseModel):
    id: int
    text: str
    priority: int
    completed: bool
    deadline: Optional[str] = None


class TodoGroup(BaseModel):
    title: str
    description: Optional[str] = None
    todos: List[TodoItem]


class ImportantDay(BaseModel):
    id: str
    date: str
    title: str
    desc: Optional[str] = ""
    cycle: Optional[str] = ""


class MemoryNote(BaseModel):
    id: str
    content: str
    tags: Optional[List[str]] = None
    source: Optional[str] = ""
    createdAt: Optional[str] = ""


class UserScheduleSettings(BaseModel):
    scheduleGroups: Optional[Dict[str, ScheduleGroup]] = None
    todoGroups: Optional[Dict[str, TodoGroup]] = None
    importantDays: Optional[List[ImportantDay]] = None
    memoryNotes: Optional[List[MemoryNote]] = None


class UpdatedPlanItem(BaseModel):
    step: str
    status: str = "pending"  # pending | in_progress | completed | cancelled
    note: Optional[str] = None


class UpdatedPlan(BaseModel):
    title: Optional[str] = None
    items: Optional[List[UpdatedPlanItem]] = None
    source: Optional[str] = None
    updatedAt: Optional[str] = None


class ScheduleDataPayload(BaseModel):
    scheduleGroups: Optional[Dict[str, Any]] = (
        None  # 使用 Any 避免严格校验导致转换麻烦，或者定义严格的 Dict[str, ScheduleGroup]
    )
    todoGroups: Optional[Dict[str, Any]] = None
    importantDays: Optional[List[ImportantDay]] = None
    memoryNotes: Optional[List[MemoryNote]] = None
    updatedPlan: Optional[UpdatedPlan] = None
