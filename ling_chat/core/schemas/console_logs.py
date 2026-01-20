"""
前端控制台日志数据模型
"""
from datetime import datetime
from enum import Enum
from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field, validator


class ConsoleLogLevel(str, Enum):
    """前端控制台日志级别"""
    LOG = "log"          # console.log
    INFO = "info"        # console.info
    WARN = "warn"        # console.warn
    ERROR = "error"      # console.error
    DEBUG = "debug"      # console.debug
    TRACE = "trace"      # console.trace


class ConsoleLogSource(str, Enum):
    """日志来源"""
    FRONTEND = "frontend"      # 前端JavaScript
    BACKEND = "backend"        # 后端Python
    SYSTEM = "system"          # 系统级别
    NETWORK = "network"        # 网络请求
    DATABASE = "database"      # 数据库操作
    AI_SERVICE = "ai_service"  # AI服务
    UNKNOWN = "unknown"        # 未知来源


class ConsoleLogEntry(BaseModel):
    """前端控制台日志条目"""
    timestamp: datetime = Field(default_factory=datetime.now, description="日志时间戳")
    level: ConsoleLogLevel = Field(..., description="前端控制台日志级别")
    message: str = Field(..., description="日志消息内容")
    source: ConsoleLogSource = Field(default=ConsoleLogSource.FRONTEND, description="日志来源")
    context: Optional[Dict[str, Any]] = Field(default=None, description="上下文信息")
    stack_trace: Optional[str] = Field(default=None, description="堆栈跟踪信息")
    component: Optional[str] = Field(default=None, description="前端组件名称")
    url: Optional[str] = Field(default=None, description="URL或文件路径")
    line_number: Optional[int] = Field(default=None, description="行号")
    column_number: Optional[int] = Field(default=None, description="列号")
    session_id: Optional[str] = Field(default=None, description="会话ID")
    user_id: Optional[int] = Field(default=None, description="用户ID")
    request_id: Optional[str] = Field(default=None, description="请求ID")
    metadata: Optional[Dict[str, Any]] = Field(default=None, description="附加元数据")

    @validator('timestamp', pre=True)
    def parse_timestamp(cls, value):
        """解析时间戳，支持字符串和datetime对象"""
        if value is None:
            return datetime.now()
        if isinstance(value, datetime):
            return value
        if isinstance(value, str):
            try:
                return datetime.fromisoformat(value.replace('Z', '+00:00'))
            except (ValueError, TypeError):
                # 如果解析失败，返回当前时间
                return datetime.now()
        # 其他类型尝试转换
        try:
            return datetime.fromtimestamp(float(value))
        except (ValueError, TypeError):
            return datetime.now()


class ConsoleLogBatch(BaseModel):
    """批量日志条目"""
    logs: List[ConsoleLogEntry] = Field(..., description="日志条目列表")
    batch_id: Optional[str] = Field(default=None, description="批次ID")
    session_id: Optional[str] = Field(default=None, description="会话ID")
    user_id: Optional[int] = Field(default=None, description="用户ID")


class LogLevelMapping(BaseModel):
    """日志级别映射配置"""
    console_log_level: ConsoleLogLevel = Field(..., description="前端控制台级别")
    backend_log_level: str = Field(..., description="后端日志级别")
    color: Optional[str] = Field(default=None, description="显示颜色")
    should_store: bool = Field(default=True, description="是否存储到文件")
    should_alert: bool = Field(default=False, description="是否需要告警")


class LogFilterConfig(BaseModel):
    """日志过滤配置"""
    min_level: ConsoleLogLevel = Field(default=ConsoleLogLevel.INFO, description="最小日志级别")
    sources: List[ConsoleLogSource] = Field(default_factory=list, description="允许的日志来源")
    components: List[str] = Field(default_factory=list, description="允许的组件")
    exclude_patterns: List[str] = Field(default_factory=list, description="排除模式")
