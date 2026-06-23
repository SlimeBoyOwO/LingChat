"""LingChat 日程/计划工具注册与处理器"""

import json
from datetime import datetime
from pathlib import Path
from typing import Any

from ling_chat.core.agent_tools.spec import ToolSpec
from ling_chat.utils.runtime_path import user_data_path


class ScheduleToolProvider:
    """日程/计划工具：读取和修改日程、待办、计划数据。"""

    def register(self, registry: dict[str, ToolSpec]) -> None:
        specs = [
            ToolSpec(
                name="get_schedules",
                description=(
                    "从本地 LingChat 日程数据中读取日程、待办和重要日子。"
                    "这可能包括 memoryNotes，但当用户特别询问手动记忆库时，请使用 get_memory_notes。"
                ),
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_schedules,
            ),
            ToolSpec(
                name="get_updated_plan",
                description=(
                    "读取 LingChat 当前的 Updated Plan（来自 schedules.json 的 updatedPlan）。"
                    "用于查看清单式计划，不要用于待办、日程或记忆。"
                ),
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_updated_plan,
            ),
            ToolSpec(
                name="update_plan",
                description=(
                    "用带步骤和状态的清单替换 LingChat 当前的 Updated Plan。"
                    "当用户要求更新、创建、修订或显示计划进度时使用此工具。"
                    "这会写入 schedules.json 的 updatedPlan，不会创建待办事项。"
                ),
                parameters={
                    "type": "object",
                    "properties": {
                        "title": {"type": "string", "description": "可选的计划标题。"},
                        "items": {
                            "type": "array",
                            "description": "按显示顺序排列的计划步骤。",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "step": {
                                        "type": "string",
                                        "description": "计划步骤内容。",
                                    },
                                    "status": {
                                        "type": "string",
                                        "enum": [
                                            "pending",
                                            "in_progress",
                                            "completed",
                                            "cancelled",
                                        ],
                                        "description": "当前状态。",
                                    },
                                    "note": {
                                        "type": "string",
                                        "description": "可选的简短备注。",
                                    },
                                },
                                "required": ["step"],
                                "additionalProperties": False,
                            },
                        },
                        "source": {
                            "type": "string",
                            "description": "可选的来源标签，默认为 AI。",
                        },
                    },
                    "required": ["items"],
                    "additionalProperties": False,
                },
                handler=self._update_plan,
            ),
            ToolSpec(
                name="schedule_add_todo",
                description="向 LingChat 日程/待办数据中添加待办事项，使其出现在日程界面中。",
                parameters={
                    "type": "object",
                    "properties": {
                        "text": {"type": "string", "description": "要添加的待办内容。"},
                        "group_title": {
                            "type": "string",
                            "description": "可选的待办分组标题，默认为'AI 添加'。",
                        },
                        "priority": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 5,
                            "description": "优先级，1 到 5。",
                        },
                        "deadline": {
                            "type": "string",
                            "description": "可选的截止时间文本或日期。",
                        },
                    },
                    "required": ["text"],
                    "additionalProperties": False,
                },
                handler=self._schedule_add_todo,
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
    def _get_schedules(self, _: dict[str, Any]) -> dict[str, Any]:
        data_path = user_data_path / "game_data" / "schedules.json"
        data = self._read_json(data_path, self._default_schedule_data())
        data.setdefault("updatedPlan", None)
        return {
            "path": str(data_path),
            "summary": {
                "schedule_group_count": len(data.get("scheduleGroups", {}) or {}),
                "todo_group_count": len(data.get("todoGroups", {}) or {}),
                "important_day_count": len(data.get("importantDays", []) or []),
                "memory_note_count": len(data.get("memoryNotes", []) or []),
                "has_updated_plan": bool(data.get("updatedPlan")),
            },
            "data": data,
        }

    def _get_updated_plan(self, _: dict[str, Any]) -> dict[str, Any]:
        data_path = user_data_path / "game_data" / "schedules.json"
        data = self._read_json(data_path, self._default_schedule_data())
        plan = data.get("updatedPlan")
        return {
            "path": str(data_path),
            "plan": plan,
            "item_count": len(plan.get("items", []) or [])
            if isinstance(plan, dict)
            else 0,
        }

    def _update_plan(self, arguments: dict[str, Any]) -> dict[str, Any]:
        raw_items = arguments.get("items")
        if not isinstance(raw_items, list) or not raw_items:
            return {"ok": False, "error": "items must be a non-empty array"}

        items: list[dict[str, Any]] = []
        for raw_item in raw_items:
            if isinstance(raw_item, dict):
                step = str(raw_item.get("step", "")).strip()
                status = str(raw_item.get("status") or "pending").strip()
                note = str(raw_item.get("note") or "").strip()
            else:
                step = str(raw_item).strip()
                status = "pending"
                note = ""

            if not step:
                continue
            if status not in {"pending", "in_progress", "completed", "cancelled"}:
                status = "pending"

            item = {"step": step, "status": status}
            if note:
                item["note"] = note
            items.append(item)

        if not items:
            return {"ok": False, "error": "at least one non-empty step is required"}

        data_path = user_data_path / "game_data" / "schedules.json"
        data_path.parent.mkdir(parents=True, exist_ok=True)
        data = self._read_json(data_path, self._default_schedule_data())
        data["updatedPlan"] = {
            "title": str(arguments.get("title") or "Updated Plan").strip()
            or "Updated Plan",
            "items": items,
            "source": str(arguments.get("source") or "AI").strip() or "AI",
            "updatedAt": datetime.now().isoformat(timespec="seconds"),
        }

        with data_path.open("w", encoding="utf-8") as file:
            json.dump(data, file, ensure_ascii=False, indent=2)

        return {
            "ok": True,
            "path": str(data_path),
            "plan": data["updatedPlan"],
            "item_count": len(items),
        }

    def _schedule_add_todo(self, arguments: dict[str, Any]) -> dict[str, Any]:
        text = str(arguments.get("text", "")).strip()
        if not text:
            return {"ok": False, "error": "text is required"}

        group_title = (
            str(arguments.get("group_title") or "AI 添加").strip() or "AI 添加"
        )
        try:
            priority = int(arguments.get("priority", 1))
        except (TypeError, ValueError):
            priority = 1
        priority = max(1, min(priority, 5))
        deadline = str(arguments.get("deadline") or "").strip()

        data_path = user_data_path / "game_data" / "schedules.json"
        data_path.parent.mkdir(parents=True, exist_ok=True)
        data = self._read_json(data_path, self._default_schedule_data())
        todo_groups = data.setdefault("todoGroups", {})

        group_id = None
        for existing_id, group in todo_groups.items():
            if isinstance(group, dict) and group.get("title") == group_title:
                group_id = existing_id
                break

        if group_id is None:
            group_id = f"ai_{int(datetime.now().timestamp() * 1000)}"
            todo_groups[group_id] = {
                "title": group_title,
                "description": "AI 添加的待办",
                "todos": [],
            }

        todo_item = {
            "id": int(datetime.now().timestamp() * 1000),
            "text": text,
            "priority": priority,
            "completed": False,
        }
        if deadline:
            todo_item["deadline"] = deadline

        group = todo_groups[group_id]
        group.setdefault("todos", []).append(todo_item)

        with data_path.open("w", encoding="utf-8") as file:
            json.dump(data, file, ensure_ascii=False, indent=2)

        return {
            "ok": True,
            "path": str(data_path),
            "group_id": group_id,
            "group_title": group_title,
            "todo": todo_item,
        }
