from typing import Any, Dict, List, Optional, Tuple

from ling_chat.core.ai_service.exceptions import RoleNotFoundError
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.type import GameRole, ScriptStatus
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.game_database.models import LineAttribute, LineBase


class ScriptFunction:
    """提供剧本事件中常用的静态工具方法，如等待用户输入、角色获取、动作处理等。"""

    @staticmethod
    async def wait_for_user_input(client_id: str) -> str | None:
        """等待来自前端的用户输入，返回输入的文本内容。"""
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
        """等待来自前端的用户选择（如分支选项），返回选择的文本。"""
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
    async def handle_actions(
        game_status: GameStatus, script_status: ScriptStatus, actions: list[dict]
    ) -> None:
        """处理剧本中定义的动作（如添加对话行等）。"""
        for action in actions:
            if action.get("type", "") == "add_line":
                user_input = action.get("content", "")
                game_status.add_line(
                    LineBase(
                        content=user_input,
                        attribute=LineAttribute.USER,
                        display_name=game_status.player.user_name,
                    )
                )

    @staticmethod
    def extract_user_input(message: Dict[str, Any]) -> str:
        """从消息中提取用户输入文本。"""
        try:
            if isinstance(message, dict):
                return message.get("content", "")
            else:
                return str(message)
        except Exception as e:
            logger.error(f"提取用户输入时发生错误: {e}")
            return ""

    @staticmethod
    def get_role(
        game_status: GameStatus, script_status: ScriptStatus, character: str
    ) -> GameRole:
        """
        根据角色标识（"MAIN" 或 script_role_key）获取运行时角色对象。
        """
        role: GameRole | None = None
        if character == "MAIN":
            role = game_status.main_role
        else:
            role = game_status.role_manager.get_role_by_script_keys(
                script_status.folder_key, character
            )
        if role is None:
            logger.error(f"角色 {character} 未找到")
            raise RoleNotFoundError(f"角色 {character} 未找到")
        return role

    @staticmethod
    def user_message_builder(user_message: str, prompt: str) -> str:
        """
        构建用户消息：若存在剧情提示，则以 {剧情提示: ...} 格式附加到消息末尾。
        """
        extra_user_message = ("\n{剧情提示: " + prompt + "}") if prompt else ""
        if user_message is not None:
            if extra_user_message != "":
                user_message += extra_user_message
        return user_message

    @staticmethod
    def memory_builder(
        game_context, memory: List[Dict], character: str, prompt: str = ""
    ) -> None:
        """
        根据游戏对话记录构建 AI 的记忆结构（角色对话流）。
        game_context: 应包含 dialogue 列表和 player 对象。
        memory: 输出参数，将被填充为消息列表（role: user/assistant）。
        """
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
            current_character = context.get("character", "")
            text = context.get("text", "")

            if current_character == "":
                continue

            # 角色切换时，将前一组内容整合进消息
            if last_character != "" and last_character != current_character:
                if narration_parts:
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    if last_character == "player" and current_character == character:
                        send_message_helper += (
                            f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n"
                            if len(player_parts) > 1
                            else ""
                        )
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                    player_parts.clear()
                if ai_parts:
                    ai_parts.clear()

            next_character = "none"
            if i + 1 < len(game_context.dialogue):
                next_character = game_context.dialogue[i + 1].get("character", "")
                logger.info(f"下一个角色是: {next_character}")

            # 分类处理不同角色的对话
            if current_character == "narration":
                narration_parts.append(text)
            elif current_character == "player":
                player_parts.append('"' + text + '"')
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

            # 处理最后一部分内容
            if next_character == "none":
                if narration_parts:
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    if current_character == "player":
                        send_message_helper += (
                            f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n"
                            if len(player_parts) > 1
                            else ""
                        )
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
            name = opt.get("name", "").strip().lower()
            if name and name == ai_response_lower:
                return opt.get("next")
        return None

    @staticmethod
    def parse_variable_action(action: str) -> Tuple[Optional[str], Optional[str], Any]:
        """
        解析变量操作字符串，返回 (操作符, 变量名, 值)
        操作符: 'assign', 'add', 'sub'
        如果解析失败，返回 (None, None, None)
        """
        import re

        action = action.strip()
        match = re.match(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*([+\-]?=)\s*(.+)$", action)
        if not match:
            return None, None, None

        var_name = match.group(1)
        operator = match.group(2)
        value_str = match.group(3).strip()
        value = ScriptFunction.parse_value(value_str)

        if operator == "=":
            return "assign", var_name, value
        elif operator == "+=":
            return "add", var_name, value
        elif operator == "-=":
            return "sub", var_name, value
        else:
            return None, None, None

    @staticmethod
    def parse_value(s: str) -> Any:
        """将字符串转换为 Python 对象（bool、数字、字符串）。"""
        s_lower = s.lower()
        if s_lower == "true":
            return True
        if s_lower == "false":
            return False

        try:
            if "." in s:
                return float(s)
            else:
                return int(s)
        except ValueError:
            pass

        if (s.startswith('"') and s.endswith('"')) or (s.startswith("'") and s.endswith("'")):
            return s[1:-1]

        return s

    @staticmethod
    def apply_variable_action(op: str, current_val: Any, value: Any) -> Any:
        """
        根据操作符应用变量操作，返回新值。
        op: 'assign', 'add', 'sub'
        """
        if op == "assign":
            return value
        elif op == "add":
            return (current_val + value) if current_val is not None else value
        elif op == "sub":
            return (current_val - value) if current_val is not None else -value
        else:
            raise ValueError(f"未知的操作符: {op}")