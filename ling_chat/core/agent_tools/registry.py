"""LingChat 工具注册表 — 聚合所有工具分类，提供统一的注册、查询和执行接口。"""

import json
from typing import Any

from ling_chat.core.agent_tools.memory_tools import MemoryToolProvider
from ling_chat.core.agent_tools.sandbox import sandbox_read_file
from ling_chat.core.agent_tools.sandbox_tools import SandboxToolProvider
from ling_chat.core.agent_tools.scene_character_tools import SceneCharacterToolProvider
from ling_chat.core.agent_tools.schedule_tools import ScheduleToolProvider
from ling_chat.core.agent_tools.spec import ToolSpec
from ling_chat.core.agent_tools.status_tools import StatusToolProvider
from ling_chat.core.ai_service.game_system.game_status import GameStatus


class SimpleToolRegistry:
    def __init__(self, game_status: GameStatus):
        self.game_status = game_status
        self._tools: dict[str, ToolSpec] = {}
        self._register_all()

    def _register_all(self) -> None:
        StatusToolProvider(self.game_status).register(self._tools)
        ScheduleToolProvider().register(self._tools)
        MemoryToolProvider().register(self._tools)
        SceneCharacterToolProvider(self.game_status).register(self._tools)
        SandboxToolProvider().register(self._tools)

    def register(self, spec: ToolSpec) -> None:
        self._tools[spec.name] = spec

    @property
    def names(self) -> list[str]:
        return sorted(self._tools)

    def _add_step_hint_param(self, parameters: dict[str, Any]) -> dict[str, Any]:
        """为工具参数添加统一的 step_hint 字段"""
        result = dict(parameters)
        props = dict(result.get("properties", {}))
        props["step_hint"] = {
            "type": "string",
            "description": (
                "在执行此工具前，向用户简要说明接下来要做什么。强烈建议填写，可减少用户等待时的困惑。"
                "例如：'正在读取文件 xxx'、'正在创建新文件'、'正在运行命令 xxx'。不填则直接执行。"
            ),
        }
        result["properties"] = props
        return result

    def get_tool_specs(self, names: set[str] | None = None) -> list[ToolSpec]:
        """获取工具规格列表（公开接口）

        Args:
            names: 可选，只获取指定名称的工具；None 表示获取全部

        Returns:
            list[ToolSpec]: 工具规格列表（每个工具自动附带 step_hint 参数）
        """
        selected = self._tools.values()
        if names is not None:
            selected = [spec for spec in selected if spec.name in names]
        return [
            ToolSpec(
                name=spec.name,
                description=spec.description,
                parameters=self._add_step_hint_param(spec.parameters),
                handler=spec.handler,
            )
            for spec in selected
        ]

    def describe_for_prompt(self, names: set[str] | None = None) -> str:
        selected_tools = self._tools.values()
        if names is not None:
            selected_tools = [spec for spec in selected_tools if spec.name in names]
        payload = [
            {
                "name": spec.name,
                "description": spec.description,
                "parameters": spec.parameters,
            }
            for spec in selected_tools
        ]
        return json.dumps(payload, ensure_ascii=False, indent=2)

    async def execute(
        self, name: str, arguments: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        arguments = arguments or {}
        spec = self._tools.get(name)
        if not spec:
            return {
                "ok": False,
                "error": f"Unknown tool: {name}",
                "available_tools": self.names,
            }
        try:
            result = spec.handler(arguments)
            ok = bool(result.get("ok", True)) if isinstance(result, dict) else True

            # 对于写入文件操作，自动读取内容并附加到结果中，防止 LLM 幻觉
            if name == "sandbox_write_file" and ok:
                file_path = arguments.get("path", "")
                read_result = sandbox_read_file(file_path)
                if read_result.get("ok"):
                    content = read_result.get("content", "")
                    max_preview = 3000
                    if len(content) > max_preview:
                        content = content[:max_preview] + "\n...[truncated]"
                    result["content_preview"] = content

            return {"ok": ok, "tool": name, "result": result}
        except Exception as exc:
            return {"ok": False, "tool": name, "error": str(exc)}
