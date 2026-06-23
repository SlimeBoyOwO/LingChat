"""LingChat 记忆管理工具注册与处理器"""

import json
from datetime import datetime
from pathlib import Path
from typing import Any

from ling_chat.core.agent_tools.spec import ToolSpec
from ling_chat.utils.runtime_path import user_data_path


class MemoryToolProvider:
    """记忆管理工具：读取和添加手动记忆笔记。"""

    def register(self, registry: dict[str, ToolSpec]) -> None:
        specs = [
            ToolSpec(
                name="get_memory_notes",
                description=(
                    "仅读取从日程记忆面板手动保存的 LingChat 记忆笔记（schedules.json 的 memoryNotes）。"
                    "这与 memory_add_note 使用的存储相同。"
                ),
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_memory_notes,
            ),
            ToolSpec(
                name="memory_add_note",
                description=(
                    "向日程记忆面板中添加持久的手动记忆笔记（schedules.json 的 memoryNotes）。"
                    "这不会更新角色的自动角色记忆库。"
                ),
                parameters={
                    "type": "object",
                    "properties": {
                        "content": {"type": "string", "description": "记忆笔记内容。"},
                        "tags": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "可选的标签列表。",
                        },
                        "source": {"type": "string", "description": "可选的来源标签。"},
                    },
                    "required": ["content"],
                    "additionalProperties": False,
                },
                handler=self._memory_add_note,
            ),
        ]
        for spec in specs:
            registry[spec.name] = spec

    # --- 共用 JSON 工具方法 ---
    def _read_json(self, path: Path, default: dict[str, Any]) -> dict[str, Any]:
        if not path.exists():
            return default
        with path.open("r", encoding="utf-8") as file:
            data = json.load(file)
        if not isinstance(data, dict):
            return default
        for key, value in default.items():
            data.setdefault(key, value)
        return data

    def _default_schedule_data(self) -> dict[str, Any]:
        return {
            "scheduleGroups": {},
            "todoGroups": {},
            "importantDays": [],
            "memoryNotes": [],
            "updatedPlan": None,
        }

    # --- 处理器 ---
    def _get_memory_notes(self, _: dict[str, Any]) -> dict[str, Any]:
        data_path = user_data_path / "game_data" / "schedules.json"
        data = self._read_json(data_path, self._default_schedule_data())
        notes = data.get("memoryNotes", []) or []
        return {
            "path": str(data_path),
            "count": len(notes),
            "items": notes,
        }

    def _memory_add_note(self, arguments: dict[str, Any]) -> dict[str, Any]:
        content = str(arguments.get("content", "")).strip()
        if not content:
            return {"ok": False, "error": "content is required"}

        raw_tags = arguments.get("tags") or []
        tags = (
            [str(tag).strip() for tag in raw_tags if str(tag).strip()]
            if isinstance(raw_tags, list)
            else []
        )
        source = str(arguments.get("source") or "AI").strip() or "AI"

        data_path = user_data_path / "game_data" / "schedules.json"
        data_path.parent.mkdir(parents=True, exist_ok=True)
        data = self._read_json(data_path, self._default_schedule_data())
        notes = data.setdefault("memoryNotes", [])

        note = {
            "id": f"mem_{int(datetime.now().timestamp() * 1000)}",
            "content": content,
            "tags": tags,
            "source": source,
            "createdAt": datetime.now().isoformat(timespec="seconds"),
        }
        notes.insert(0, note)

        with data_path.open("w", encoding="utf-8") as file:
            json.dump(data, file, ensure_ascii=False, indent=2)

        return {
            "ok": True,
            "path": str(data_path),
            "memory": note,
            "count": len(notes),
        }
