from typing import Any, Dict, List, Optional

from ling_chat.core.ai_service.exceptions import RoleNotFoundError
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.type import GameRole, ScriptStatus
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.game_database.models import LineAttribute, LineBase


class ScriptFunction:
    @staticmethod
    async def wait_for_user_input(client_id: str) -> str | None:
        """等待来自前端的用户输入"""
        try:
            subscription = message_broker.subscribe("ai_script_input_" + client_id)
            async for message in subscription:
                user_input = ScriptFunction.extract_user_input(message)
                if user_input:
                    return user_input
        except Exception as e:
            logger.error(f"等待用户输入时发生错误: {e}")
            return ""

    @staticmethod
    async def wait_for_user_choice(client_id: str) -> str | None:
        """等待来自前端的用户选择（如分支选项）"""
        try:
            subscription = message_broker.subscribe("ai_script_choice_" + client_id)
            async for message in subscription:
                user_input = ScriptFunction.extract_user_input(message)
                if user_input:
                    return user_input
        except Exception as e:
            logger.error(f"等待选择事件时发生错误: {e}")
            return ""

    @staticmethod
    async def handle_actions(game_status: GameStatus, script_status: ScriptStatus, actions: list[dict]) -> None:
        """处理脚本中的动作"""
        for action in actions:
            if action.get("type", "") == "add_line":
                user_input = action.get("content", "")
                game_status.add_line(
                    LineBase(content=user_input, attribute=LineAttribute.USER, display_name=game_status.player.user_name)
                )

    @staticmethod
    def extract_user_input(message: Dict[str, Any]) -> str:
        """从消息中提取用户输入文本"""
        try:
            if isinstance(message, dict):
                return message.get('content', '')
            else:
                return str(message)
        except Exception as e:
            logger.error(f"提取用户输入时发生错误: {e}")
            return ""

    @staticmethod
    def get_role(game_status: GameStatus, script_status: ScriptStatus, character: str) -> GameRole:
        role: GameRole | None = None
        if character == "MAIN":
            role = game_status.main_role
        else:
            role = game_status.role_manager.get_role_by_script_keys(script_status.folder_key, character)
        if role is None:
            logger.error(f"角色 {character} 未找到")
            raise RoleNotFoundError(f"角色 {character} 未找到")
        return role

    @staticmethod
    def user_message_builder(user_message, prompt) -> str:
        extra_user_message = ("\n{剧情提示: " + prompt + "}") if prompt else ""
        if user_message is not None:
            if extra_user_message != "":
                user_message += extra_user_message
        return user_message

    @staticmethod
    def memory_builder(game_context, memory, character: str, prompt: str = ""):
        user_name = game_context.player.user_name

        send_message_helper = ""
        send_message_main = ""
        send_message_tail = ("\n{剧情提示: " + prompt + "}") if prompt else ""

        ai_message = ""

        narration_parts = []
        player_parts = []
        ai_parts = []

        last_character = ""

        for i, context in enumerate(game_context.dialogue):
            current_character = context.get('character', '')
            text = context.get('text', '')

            if current_character == '':
                continue

            if last_character != "" and last_character != current_character:
                if narration_parts:
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    if last_character == 'player' and current_character == character:
                        send_message_helper += (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n") if len(player_parts) > 1 else ""
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                    player_parts.clear()
                if ai_parts:
                    ai_parts.clear()

            next_character = "none"
            if i + 1 < len(game_context.dialogue):
                next_character = game_context.dialogue[i + 1].get('character', '')
                logger.info(f"下一个角色是: {next_character}")

            if current_character == 'narration':
                narration_parts.append(text)
            elif current_character == 'player':
                player_parts.append("\"" + text + "\"")
            elif current_character == character:
                ai_parts.append(text)
                if last_character != current_character:
                    final_message = ""
                    if send_message_helper:
                        final_message += "{" + send_message_helper + "}\n"
                    final_message += send_message_main
                    memory.append({"role": "user", "content": final_message})
                    send_message_helper = ""
                    send_message_main = ""

                if next_character != current_character:
                    ai_message += "".join(ai_parts)
                    memory.append({"role": "assistant", "content": ai_message})
                    ai_parts.clear()
                    ai_message = ""

            if next_character == "none":
                if narration_parts:
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    if current_character == 'player':
                        send_message_helper += (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n") if len(player_parts) > 1 else ""
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                    player_parts.clear()
                if ai_parts:
                    ai_parts.clear()

                final_message = ""
                if send_message_helper:
                    final_message += "{" + send_message_helper + "}\n"
                final_message += send_message_main + send_message_tail
                memory.append({"role": "user", "content": final_message})

            last_character = current_character

    @staticmethod
    def match_option(ai_response: str, options: List[Dict]) -> Optional[str]:
        """
        匹配 AI 回复与选项名称，返回对应的 next 或 None。
        """
        ai_response_lower = ai_response.strip().lower()
        for opt in options:
            name = opt.get('name', '').strip().lower()
            if name and name == ai_response_lower:
                return opt.get('next')
        return None