"""LingChat 状态查询工具注册与处理器"""

from datetime import datetime
from typing import Any

from ling_chat.core.agent_tools.spec import ToolSpec
from ling_chat.core.ai_service.game_system.game_status import GameStatus


class StatusToolProvider:
    """状态查询工具：读取 LingChat 当前运行时状态、场景、记忆和时间。"""

    def __init__(self, game_status: GameStatus):
        self.game_status = game_status

    def register(self, registry: dict[str, ToolSpec]) -> None:
        specs = [
            ToolSpec(
                name="get_current_status",
                description="读取 LingChat 当前运行时状态，包括当前角色、场景、背景和消息数量。",
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_current_status,
            ),
            ToolSpec(
                name="get_current_scene",
                description="读取当前场景描述和角色所在的场景名称。",
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_current_scene,
            ),
            ToolSpec(
                name="get_memory",
                description=(
                    "仅读取当前角色的自动角色记忆库，包括短期记忆、长期记忆、用户信息和承诺。"
                    "不要将此方法用于从日程记忆面板手动保存的记忆笔记。"
                ),
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_memory,
            ),
            ToolSpec(
                name="get_current_time",
                description="获取当前日期和时间。",
                parameters={
                    "type": "object",
                    "properties": {},
                    "additionalProperties": False,
                },
                handler=self._get_current_time,
            ),
        ]
        for spec in specs:
            registry[spec.name] = spec

    def _get_current_status(self, _: dict[str, Any]) -> dict[str, Any]:
        role = self.game_status.current_character
        player = self.game_status.player
        return {
            "current_character": {
                "role_id": role.role_id if role else None,
                "display_name": role.display_name if role else None,
                "resource_path": role.resource_path if role else None,
            },
            "player": {
                "user_name": player.user_name,
                "user_subtitle": player.user_subtitle,
            },
            "scene": {
                "current_scene": self.game_status.current_scene,
                "scene_description": self.game_status.scene_description,
            },
            "media": {
                "background": self.game_status.background,
                "background_effect": self.game_status.background_effect,
                "background_music": self.game_status.background_music,
                "present_pic": self.game_status.present_pic,
            },
            "message_count": self.game_status.get_chat_message_count(),
            "active_save_id": self.game_status.active_save_id,
        }

    def _get_current_scene(self, _: dict[str, Any]) -> dict[str, Any]:
        return {
            "current_scene": self.game_status.current_scene,
            "scene_description": self.game_status.scene_description,
        }

    def _get_memory(self, _: dict[str, Any]) -> dict[str, Any]:
        role = self.game_status.current_character
        if not role:
            return {"error": "No current character is active."}
        mb = role.memory_bank
        return {
            "character": role.display_name,
            "short_term": mb.data.short_term,
            "long_term": mb.data.long_term,
            "user_info": mb.data.user_info,
            "promises": mb.data.promises,
        }

    def _get_current_time(self, _: dict[str, Any]) -> dict[str, Any]:
        now = datetime.now()
        return {
            "datetime": now.isoformat(timespec="seconds"),
            "date": now.strftime("%Y-%m-%d"),
            "time": now.strftime("%H:%M:%S"),
            "weekday": now.strftime("%A"),
        }
