"""LingChat 沙盒工具注册与处理器"""

from typing import Any

from ling_chat.core.agent_tools.sandbox import (
    sandbox_delete_file,
    sandbox_execute_command,
    sandbox_list_files,
    sandbox_read_file,
    sandbox_write_file,
)
from ling_chat.core.agent_tools.spec import ToolSpec


class SandboxToolProvider:
    """沙盒工具：在隔离环境中读取、写入、列出、删除文件和执行命令。"""

    def register(self, registry: dict[str, ToolSpec]) -> None:
        specs = [
            ToolSpec(
                name="sandbox_read_file",
                description="读取沙盒内的文件内容。只能访问沙盒内的文件。",
                parameters={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "沙盒内文件的相对路径。",
                        },
                    },
                    "required": ["path"],
                    "additionalProperties": False,
                },
                handler=self._sandbox_read_file,
            ),
            ToolSpec(
                name="sandbox_write_file",
                description="写入或覆盖沙盒内的文件。只能修改沙盒内的文件。",
                parameters={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "沙盒内文件的相对路径。",
                        },
                        "content": {
                            "type": "string",
                            "description": "要写入文件的内容。",
                        },
                        "append": {
                            "type": "boolean",
                            "description": "如果为 true，则追加到文件而不是覆盖。",
                        },
                    },
                    "required": ["path", "content"],
                    "additionalProperties": False,
                },
                handler=self._sandbox_write_file,
            ),
            ToolSpec(
                name="sandbox_list_files",
                description="列出沙盒内的文件和目录。",
                parameters={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "沙盒内目录的相对路径，默认为根目录。",
                        },
                    },
                    "additionalProperties": False,
                },
                handler=self._sandbox_list_files,
            ),
            ToolSpec(
                name="sandbox_delete_file",
                description="删除沙盒内的文件或目录。对非空目录使用 recursive=true，或 path='.' 清空沙盒内容。",
                parameters={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "沙盒内文件环境路径。",
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "递归删除非空目录。与 path='.' 一起使用可清空沙盒内容。",
                        },
                    },
                    "required": ["path"],
                    "additionalProperties": False,
                },
                handler=self._sandbox_delete_file,
            ),
            ToolSpec(
                name="sandbox_execute_command",
                description="在沙盒内执行安全命令。只允许白名单中的命令（python、node、npm、git、ls 等）。危险命令会被拦截。",
                parameters={
                    "type": "object",
                    "properties": {
                        "command": {"type": "string", "description": "要执行的命令。"},
                        "timeout": {
                            "type": "integer",
                            "description": "超时时间（秒），1-120。不填则默认 30 秒。对于简单命令（ls、cat）可用 5-10 秒，运行程序或测试可用 30-60 秒。",
                        },
                    },
                    "required": ["command"],
                    "additionalProperties": False,
                },
                handler=self._sandbox_execute_command,
            ),
        ]
        for spec in specs:
            registry[spec.name] = spec

    # --- 处理器 ---
    def _sandbox_read_file(self, args: dict[str, Any]) -> dict[str, Any]:
        return sandbox_read_file(args.get("path", ""))

    def _sandbox_write_file(self, args: dict[str, Any]) -> dict[str, Any]:
        return sandbox_write_file(
            args.get("path", ""),
            args.get("content", ""),
            args.get("append", False),
        )

    def _sandbox_list_files(self, args: dict[str, Any]) -> dict[str, Any]:
        return sandbox_list_files(args.get("path", "."))

    def _sandbox_delete_file(self, args: dict[str, Any]) -> dict[str, Any]:
        return sandbox_delete_file(
            args.get("path", ""),
            args.get("recursive", False),
        )

    def _sandbox_execute_command(self, args: dict[str, Any]) -> dict[str, Any]:
        return sandbox_execute_command(
            args.get("command", ""),
            args.get("timeout", 30),
        )
