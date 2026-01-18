"""
前端控制台日志服务 - 接收前端控制台输出并分类为不同日志级别
"""
import json
import os
import re
from datetime import datetime
from typing import Any, Dict, Optional

from ling_chat.core.logger import TermColors, logger
from ling_chat.core.schemas.console_logs import (
    ConsoleLogEntry,
    ConsoleLogLevel,
    ConsoleLogSource,
    LogFilterConfig,
    LogLevelMapping,
)


class ConsoleLogService:
    """前端控制台日志服务"""

    # 默认的日志级别映射
    DEFAULT_LEVEL_MAPPINGS = {
        ConsoleLogLevel.ERROR: LogLevelMapping(
            console_log_level=ConsoleLogLevel.ERROR,
            backend_log_level="ERROR",
            color=TermColors.RED,
            should_store=True,
            should_alert=True
        ),
        ConsoleLogLevel.WARN: LogLevelMapping(
            console_log_level=ConsoleLogLevel.WARN,
            backend_log_level="WARNING",
            color=TermColors.YELLOW,
            should_store=True,
            should_alert=False
        ),
        ConsoleLogLevel.INFO: LogLevelMapping(
            console_log_level=ConsoleLogLevel.INFO,
            backend_log_level="INFO",
            color=TermColors.GREEN,
            should_store=True,
            should_alert=False
        ),
        ConsoleLogLevel.LOG: LogLevelMapping(
            console_log_level=ConsoleLogLevel.LOG,
            backend_log_level="INFO",
            color=TermColors.WHITE,
            should_store=True,
            should_alert=False
        ),
        ConsoleLogLevel.DEBUG: LogLevelMapping(
            console_log_level=ConsoleLogLevel.DEBUG,
            backend_log_level="DEBUG",
            color=TermColors.GREY,
            should_store=False,  # 默认不存储DEBUG日志到文件
            should_alert=False
        ),
        ConsoleLogLevel.TRACE: LogLevelMapping(
            console_log_level=ConsoleLogLevel.TRACE,
            backend_log_level="DEBUG",
            color=TermColors.GREY,
            should_store=False,  # 默认不存储TRACE日志到文件
            should_alert=False
        )
    }

    # 常见错误模式识别
    ERROR_PATTERNS = [
        (r"error|exception|failed|failure", ConsoleLogLevel.ERROR),
        (r"warning|warn|caution|alert", ConsoleLogLevel.WARN),
        (r"debug|trace|verbose", ConsoleLogLevel.DEBUG),
        (r"info|log|message|status", ConsoleLogLevel.INFO),
    ]

    # 来源识别模式
    SOURCE_PATTERNS = [
        (r"network|http|fetch|axios|request", ConsoleLogSource.NETWORK),
        (r"database|sql|query|mongodb|redis", ConsoleLogSource.DATABASE),
        (r"ai|llm|model|inference|generation", ConsoleLogSource.AI_SERVICE),
        (r"system|os|process|memory|cpu", ConsoleLogSource.SYSTEM),
        (r"frontend|vue|react|component|dom", ConsoleLogSource.FRONTEND),
    ]

    def __init__(self):
        self.level_mappings = self.DEFAULT_LEVEL_MAPPINGS.copy()
        self.filter_config = self._get_default_filter_config()
        self._setup_logger()

    def _setup_logger(self):
        """设置日志器"""
        self.backend_logger = logger

    def _get_default_filter_config(self) -> LogFilterConfig:
        """根据环境变量获取默认过滤配置"""
        # 检查是否启用前端日志转发
        enable_forwarding = os.environ.get('ENABLE_FRONTEND_LOG_FORWARDING', 'true').lower() == 'true'

        # 创建过滤配置 - 前端转发所有日志，由后端根据LOG_LEVEL过滤
        # 这里设置最小级别为TRACE，让所有日志都通过前端过滤
        # 实际过滤将在_log_to_backend中根据LOG_LEVEL进行
        config = LogFilterConfig(
            min_level=ConsoleLogLevel.TRACE,  # 前端转发所有日志
            sources=[],  # 默认不过滤来源
            components=[],  # 默认不过滤组件
            exclude_patterns=[
                'password', 'token', 'secret', 'key',  # 敏感信息
            ]
        )

        # 如果不启用转发，设置一个非常高的最小级别来过滤所有日志
        if not enable_forwarding:
            config.min_level = ConsoleLogLevel.ERROR  # 只允许ERROR级别

        return config

    def process_log_entry(self, log_entry: ConsoleLogEntry) -> Dict[str, Any]:
        """
        处理单个日志条目

        Args:
            log_entry: 前端控制台日志条目

        Returns:
            处理结果字典
        """
        try:
            # 1. 应用过滤规则
            if not self._should_process(log_entry):
                return {"processed": False, "reason": "filtered"}

            # 2. 获取级别映射
            mapping = self._get_level_mapping(log_entry.level)
            if not mapping:
                mapping = self._infer_level_mapping(log_entry)

            # 3. 增强日志信息
            enhanced_entry = self._enhance_log_entry(log_entry, mapping)

            # 4. 检查是否应该根据LOG_LEVEL记录
            should_log = self._should_log_by_backend_level(mapping.backend_log_level)

            # 5. 记录到后端日志系统（如果LOG_LEVEL允许）
            if should_log:
                self._log_to_backend(enhanced_entry, mapping)
            else:
                # 即使不记录到日志，也标记为已处理（只是不输出）
                pass

            # 6. 触发告警（如果需要）
            if mapping.should_alert:
                self._trigger_alert(enhanced_entry)

            return {
                "processed": True,
                "backend_level": mapping.backend_log_level,
                "stored": mapping.should_store,
                "alerted": mapping.should_alert,
                "logged": should_log,  # 新增字段，表示是否实际记录到日志
                "enhanced_entry": enhanced_entry.model_dump()
            }

        except Exception as e:
            self.backend_logger.error(f"处理前端日志失败: {str(e)}")
            return {"processed": False, "error": str(e)}

    def _should_process(self, log_entry: ConsoleLogEntry) -> bool:
        """检查是否应该处理此日志条目"""
        # 检查日志级别（前端过滤，现在设置为TRACE，所以所有日志都会通过）
        if self._compare_levels(log_entry.level.value, self.filter_config.min_level.value) < 0:
            return False

        # 检查来源
        if self.filter_config.sources and log_entry.source not in self.filter_config.sources:
            return False

        # 检查组件
        if self.filter_config.components and log_entry.component not in self.filter_config.components:
            return False

        # 检查排除模式
        message = log_entry.message.lower()
        for pattern in self.filter_config.exclude_patterns:
            if re.search(pattern, message, re.IGNORECASE):
                return False

        return True

    def _should_log_by_backend_level(self, backend_level: str) -> bool:
        """根据LOG_LEVEL环境变量检查是否应该记录此后端日志级别"""
        import os
        log_level = os.environ.get('LOG_LEVEL', 'INFO').upper()

        # 后端日志级别优先级
        backend_level_order = {
            "DEBUG": 10,
            "INFO": 20,
            "WARNING": 30,
            "ERROR": 40,
            "CRITICAL": 50
        }

        # 环境变量LOG_LEVEL优先级
        log_level_order = {
            "DEBUG": 10,
            "INFO": 20,
            "WARNING": 30,
            "ERROR": 40,
            "CRITICAL": 50
        }

        current_level = backend_level_order.get(backend_level.upper(), 20)
        min_level = log_level_order.get(log_level, 20)

        # 只有当当前级别 >= 最小级别时才记录
        return current_level >= min_level

    def _get_level_mapping(self, console_level: ConsoleLogLevel) -> Optional[LogLevelMapping]:
        """获取日志级别映射"""
        return self.level_mappings.get(console_level)

    def _infer_level_mapping(self, log_entry: ConsoleLogEntry) -> LogLevelMapping:
        """根据消息内容推断日志级别映射"""
        message = log_entry.message.lower()

        # 检查错误模式
        for pattern, level in self.ERROR_PATTERNS:
            if re.search(pattern, message, re.IGNORECASE):
                return self.level_mappings.get(level, self.DEFAULT_LEVEL_MAPPINGS[ConsoleLogLevel.INFO])

        # 默认返回INFO级别
        return self.DEFAULT_LEVEL_MAPPINGS[ConsoleLogLevel.INFO]

    def _enhance_log_entry(self, log_entry: ConsoleLogEntry, mapping: LogLevelMapping) -> ConsoleLogEntry:
        """增强日志条目信息"""
        enhanced_data = log_entry.model_dump()

        # 自动识别来源（如果未指定）
        if log_entry.source == ConsoleLogSource.UNKNOWN:
            enhanced_data["source"] = self._infer_source(log_entry.message)

        # 添加处理时间戳
        enhanced_data["processed_at"] = datetime.now().isoformat()

        # 添加后端日志级别
        enhanced_data["backend_log_level"] = mapping.backend_log_level

        # 尝试解析JSON消息
        if isinstance(log_entry.message, str) and log_entry.message.strip().startswith("{"):
            try:
                parsed = json.loads(log_entry.message)
                if isinstance(parsed, dict):
                    enhanced_data["parsed_message"] = parsed
                    # 提取关键字段
                    if "error" in parsed:
                        enhanced_data["error_type"] = parsed.get("error")
                    if "code" in parsed:
                        enhanced_data["error_code"] = parsed.get("code")
            except json.JSONDecodeError:
                pass

        return ConsoleLogEntry(**enhanced_data)

    def _infer_source(self, message: str) -> ConsoleLogSource:
        """根据消息内容推断来源"""
        message_lower = message.lower()
        for pattern, source in self.SOURCE_PATTERNS:
            if re.search(pattern, message_lower, re.IGNORECASE):
                return source
        return ConsoleLogSource.UNKNOWN

    def _log_to_backend(self, log_entry: ConsoleLogEntry, mapping: LogLevelMapping):
        """记录到后端日志系统"""
        # 构建格式化消息
        timestamp = log_entry.timestamp.strftime("%Y-%m-%d %H:%M:%S")
        source = log_entry.source.value
        component = f"[{log_entry.component}]" if log_entry.component else ""

        message = f"[前端{source}]{component} {log_entry.message}"

        if log_entry.stack_trace:
            message += f"\n堆栈跟踪:\n{log_entry.stack_trace}"

        if log_entry.context:
            context_str = json.dumps(log_entry.context, ensure_ascii=False, indent=2)
            message += f"\n上下文:\n{context_str}"

        # 根据映射的级别记录日志
        backend_level = mapping.backend_log_level.upper()

        if backend_level == "DEBUG":
            self.backend_logger.debug(message)
        elif backend_level == "INFO":
            self.backend_logger.info(message)
        elif backend_level == "WARNING":
            self.backend_logger.warning(message)
        elif backend_level == "ERROR":
            self.backend_logger.error(message)
        elif backend_level == "CRITICAL":
            self.backend_logger.critical(message)
        else:
            self.backend_logger.info(message)  # 默认使用INFO

    def _trigger_alert(self, log_entry: ConsoleLogEntry):
        """触发告警（预留接口）"""
        # 这里可以集成邮件、Slack、钉钉等告警系统
        alert_message = f"前端错误告警: {log_entry.message}"
        if log_entry.component:
            alert_message += f" (组件: {log_entry.component})"

        self.backend_logger.warning(f"告警触发: {alert_message}")

    def _compare_levels(self, level1: str, level2: str) -> int:
        """比较两个日志级别的优先级"""
        level_order = {
            "trace": 0,
            "debug": 1,
            "log": 2,
            "info": 3,
            "warn": 4,
            "error": 5
        }
        order1 = level_order.get(level1.lower(), 2)
        order2 = level_order.get(level2.lower(), 2)
        return order1 - order2

    def update_level_mapping(self, console_level: ConsoleLogLevel, mapping: LogLevelMapping):
        """更新日志级别映射"""
        self.level_mappings[console_level] = mapping

    def update_filter_config(self, config: LogFilterConfig):
        """更新过滤配置"""
        self.filter_config = config

    def get_stats(self) -> Dict[str, Any]:
        """获取服务统计信息"""
        return {
            "level_mappings": {k.value: v.model_dump() for k, v in self.level_mappings.items()},
            "filter_config": self.filter_config.model_dump(),
            "default_mappings": {k.value: v.model_dump() for k, v in self.DEFAULT_LEVEL_MAPPINGS.items()}
        }


# 全局服务实例
console_log_service = ConsoleLogService()
