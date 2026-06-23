"""LingChat 场景/角色工具注册与处理器"""

from typing import Any

from ling_chat.core.agent_tools.spec import ToolSpec
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.utils.scene_manager import SceneManager
from ling_chat.utils.scene_utils import list_available_scenes


class SceneCharacterToolProvider:
    """场景/角色工具：列出和切换场景与角色。"""

    def __init__(self, game_status: GameStatus):
        self.game_status = game_status

    def register(self, registry: dict[str, ToolSpec]) -> None:
        specs = [
            ToolSpec(
                name="list_scenes",
                description="列出可用的 LingChat 场景及其描述。",
                parameters={
                    "type": "object",
                    "properties": {
                        "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                    },
                    "additionalProperties": False,
                },
                handler=self._list_scenes,
            ),
            ToolSpec(
                name="list_characters",
                description="列出已知的 LingChat 角色。",
                parameters={
                    "type": "object",
                    "properties": {
                        "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                    },
                    "additionalProperties": False,
                },
                handler=self._list_characters,
            ),
            ToolSpec(
                name="switch_scene",
                description="切换到不同的场景。通过场景名称或部分名称匹配。",
                parameters={
                    "type": "object",
                    "properties": {
                        "scene_name": {
                            "type": "string",
                            "description": "要切换到的场景名称或部分名称。",
                        },
                    },
                    "required": ["scene_name"],
                    "additionalProperties": False,
                },
                handler=self._switch_scene,
            ),
            ToolSpec(
                name="switch_character",
                description="切换到不同的角色。通过角色名称或部分名称匹配。",
                parameters={
                    "type": "object",
                    "properties": {
                        "character_name": {
                            "type": "string",
                            "description": "要切换到的角色名称或部分名称。",
                        },
                    },
                    "required": ["character_name"],
                    "additionalProperties": False,
                },
                handler=self._switch_character,
            ),
        ]
        for spec in specs:
            registry[spec.name] = spec

    # --- 处理器 ---
    def _switch_scene(self, arguments: dict[str, Any]) -> dict[str, Any]:
        scene_name = str(arguments.get("scene_name", "")).strip()
        if not scene_name:
            return {"ok": False, "error": "scene_name is required"}

        scene_manager = SceneManager()
        scenes = scene_manager.list_scenes()

        # 先尝试完全匹配
        target = None
        for scene in scenes:
            if scene.get("sceneName", "").lower() == scene_name.lower():
                target = scene
                break

        # 再尝试部分匹配
        if target is None:
            for scene in scenes:
                if scene_name.lower() in scene.get("sceneName", "").lower():
                    target = scene
                    break

        # 再尝试匹配 txt 场景
        if target is None:
            for txt_scene in list_available_scenes():
                filename = str(txt_scene.get("filename", "")).replace(".txt", "")
                if scene_name.lower() in filename.lower():
                    target = {
                        "id": f"txt_{txt_scene.get('filename', '')}",
                        "sceneName": filename,
                        "sceneImage": txt_scene.get("filename"),
                        "sceneDescription": txt_scene.get("description", ""),
                        "source": "txt",
                    }
                    break

        if target is None:
            return {
                "ok": False,
                "error": f"Scene '{scene_name}' not found",
                "available_scenes": [s.get("sceneName") for s in scenes],
            }

        self.game_status.current_scene = target.get("sceneDescription", "")
        return {
            "ok": True,
            "scene": target,
            "message": f"Switched to scene: {target.get('sceneName')}",
        }

    def _switch_character(self, arguments: dict[str, Any]) -> dict[str, Any]:
        character_name = str(arguments.get("character_name", "")).strip()
        if not character_name:
            return {"ok": False, "error": "character_name is required"}

        role, settings = self._find_character_role(character_name)

        if role is None or not role.id or settings is None:
            return {"ok": False, "error": f"Character '{character_name}' not found"}

        settings.character_id = role.id

        try:
            from ling_chat.core.service_manager import service_manager
            from ling_chat.game_database.managers.user_manager import UserManager

            if service_manager.ai_service is not None:
                service_manager.ai_service.import_settings(settings=settings)
                service_manager.ai_service.reset_lines()
            else:
                game_role = self.game_status.role_manager.get_role(role.id)
                self.game_status.current_character = game_role
                self.game_status.main_role = game_role
                self.game_status.onstage_role(game_role)

            UserManager.update_last_character(user_id=1, role_id=role.id)
        except Exception as exc:
            return {"ok": False, "error": f"Failed to switch character: {exc}"}

        game_role = (
            self.game_status.current_character
            or self.game_status.role_manager.get_role(role.id)
        )
        display_name = game_role.display_name if game_role else settings.ai_name

        return {
            "ok": True,
            "character": {
                "id": role.id,
                "role_id": role.id,
                "title": role.name,
                "name": settings.ai_name,
                "subtitle": settings.ai_subtitle,
                "folder_name": role.resource_folder,
                "resource_folder": role.resource_folder,
            },
            "message": f"Switched to character: {display_name}",
        }

    def _find_character_role(self, character_name: str):
        needle = character_name.strip()
        if not needle:
            return None, None

        role = RoleManager.get_role_by_name(needle)
        if role and role.id:
            settings = RoleManager.get_role_settings_by_id(role.id)
            if settings:
                return role, settings

        roles = RoleManager.get_all_main_roles()

        exact_matches = []
        partial_matches = []
        needle_lower = needle.lower()
        for role in roles:
            if not role.id:
                continue
            settings = RoleManager.get_role_settings_by_id(role.id)
            if settings is None:
                continue
            candidates = [
                role.name,
                role.resource_folder,
                settings.ai_name,
                settings.title,
            ]
            normalized = [str(value).strip() for value in candidates if value]
            if any(value.lower() == needle_lower for value in normalized):
                exact_matches.append((role, settings))
            elif any(needle_lower in value.lower() for value in normalized):
                partial_matches.append((role, settings))

        if exact_matches:
            return exact_matches[0]
        if partial_matches:
            return partial_matches[0]

        roles = RoleManager.search_roles_by_name(needle, limit=5)
        for role in roles:
            if role.id:
                settings = RoleManager.get_role_settings_by_id(role.id)
                if settings:
                    return role, settings

        return None, None

    def _list_scenes(self, arguments: dict[str, Any]) -> dict[str, Any]:
        limit = self._coerce_limit(arguments.get("limit"), 10)
        scene_manager = SceneManager()
        scenes = scene_manager.list_scenes()

        existing_images = {
            scene.get("sceneImage") for scene in scenes if scene.get("sceneImage")
        }
        for txt_scene in list_available_scenes():
            if txt_scene.get("filename") not in existing_images:
                scenes.append(
                    {
                        "id": f"txt_{txt_scene.get('filename', '')}",
                        "sceneName": str(txt_scene.get("filename", "")).replace(
                            ".txt", ""
                        ),
                        "sceneImage": txt_scene.get("filename"),
                        "sceneDescription": txt_scene.get("description", ""),
                        "source": "txt",
                    }
                )

        return {"count": len(scenes), "items": scenes[:limit]}

    def _list_characters(self, arguments: dict[str, Any]) -> dict[str, Any]:
        limit = self._coerce_limit(arguments.get("limit"), 10)
        roles = RoleManager.get_all_main_roles()
        items = []
        for role in roles[:limit]:
            settings = RoleManager.get_role_settings_by_id(role.id) if role.id else None
            items.append(
                {
                    "role_id": role.id,
                    "name": settings.ai_name if settings else role.name,
                    "subtitle": settings.ai_subtitle if settings else "",
                    "resource_folder": role.resource_folder,
                    "info": settings.info if settings else "",
                }
            )
        return {"count": len(roles), "items": items}

    def _coerce_limit(self, value: Any, default: int) -> int:
        try:
            limit = int(value)
        except (TypeError, ValueError):
            limit = default
        return max(1, min(limit, 20))
