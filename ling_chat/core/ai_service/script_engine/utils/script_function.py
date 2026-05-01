from typing import Any, Dict, List, Optional, Tuple

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
            # 订阅特定的输入频道
            subscription = message_broker.subscribe("ai_script_input_" + client_id)

            # 使用异步for循环来消费消息
            async for message in subscription:
                user_input = ScriptFunction.extract_user_input(message)
                if user_input:
                    return user_input

        except Exception as e:
            logger.error(f"等待用户输入时发生错误: {e}")
            return ""

    @staticmethod
    async def wait_for_user_choice(client_id: str) -> str | None:
        """等待来自前端的用户输入"""
        try:
            # 订阅特定的输入频道
            subscription = message_broker.subscribe("ai_script_choice_" + client_id)

            # 使用异步for循环来消费消息
            async for message in subscription:
                user_input = ScriptFunction.extract_user_input(message)
                if user_input:
                    return user_input

        except Exception as e:
            logger.error(f"等待选择事件时发生错误: {e}")
            return ""

    @staticmethod
    async def process_options(
        game_status: GameStatus,
        script_status: ScriptStatus,
        options: list[dict],
        input: Optional[str] = None,
    ) -> bool:
        """匹配选项并执行actions，如果有匹配的则返回 True，否则返回 False"""
        for option in options:
            actions = option.get("actions", [])
            if not actions:
                continue

            # 基础匹配，输入与选项文本相同
            if input and option.get("text", "") == input:
                await ScriptFunction.handle_actions(game_status, script_status, actions)
                return True

            # 正则表达式匹配
            if option.get("condition", ""):
                condition = option.get("condition", "")
                condition_met = ScriptFunction.evaluate(condition, script_status.vars)

                if condition_met:
                    await ScriptFunction.handle_actions(
                        game_status, script_status, actions
                    )
                    return True
        return False

    @staticmethod
    def replace_placeholder(
        text: str, game_status: GameStatus, script_status: ScriptStatus
    ) -> str:
        """替换文本中的占位符"""
        # 规则：占位符有%player%，用game_status.player.user_name替换
        return text.replace("%player%", game_status.player.user_name)

    @staticmethod
    async def handle_actions(
        game_status: GameStatus, script_status: ScriptStatus, actions: list[dict]
    ) -> None:
        """处理脚本中的动作"""
        # 根据action类型执行相应的操作
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
            # 修复了原代码中的 typo: action.get({"type"})
            elif action.get("type", "") == "set_var":
                content = action.get("content", "")
                op, var_name, value = ScriptFunction.parse_variable_action(content)
                if op is None:
                    logger.error(f"无法解析设置变量运算符: {content}")
                    return

                if var_name is None:
                    logger.error(f"操作没有指定变量: {content}")
                    return

                current_val = script_status.get_variable(var_name)

                try:
                    new_val = ScriptFunction.apply_variable_action(
                        op, current_val, value
                    )
                except Exception as e:
                    logger.error(f"执行设置变量操作 '{content}' 时出错: {e}")
                    return

                script_status.set_variable(var_name, new_val)

    @staticmethod
    def extract_user_input(message: Dict[str, Any]) -> str:
        """从消息中提取用户输入文本"""
        try:
            # 根据实际的消息结构来提取用户输入
            # 这里假设消息中有 'text' 或 'input' 字段包含用户输入
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
        role: GameRole | None = None
        if character == "MAIN":
            role = game_status.main_role
        else:
            role = game_status.role_manager.get_role_by_script_keys(
                script_status.path_key, character
            )
        if role is None:
            logger.error(f"角色 {character} 未找到")
            raise RoleNotFoundError(f"角色 {character} 未找到")
        return role

    @staticmethod
    def user_message_builder(user_message, prompt) -> str:
        extra_user_message = ("\n{剧情提示: " + prompt + "}") if prompt else ""

        # 将用户输入（加上剧情提示）存储到游戏上下文
        if user_message is not None:
            if extra_user_message != "":
                user_message += extra_user_message

        return user_message

    @staticmethod
    def evaluate(expr: str, variables: dict) -> bool:
        """
        对表达式 expr 求值，返回布尔值。
        使用安全的 eval 包装，只允许访问 variables 中的变量。
        """
        if not expr:
            return True

        # 构建安全的全局和局部命名空间
        safe_globals = {
            "__builtins__": {},  # 禁用所有内置函数
            "True": True,
            "False": False,
            "None": None,
        }
        safe_locals = variables.copy()

        try:
            result = eval(expr, safe_globals, safe_locals)
            return bool(result)
        except NameError as e:
            logger.warning(f"表达式 '{expr}' 中的变量未定义: {e}")
            return False
        except SyntaxError as e:
            logger.error(f"表达式 '{expr}' 语法错误: {e}")
            return False
        except Exception as e:
            logger.error(f"表达式求值出错: {expr} - {e}")
            return False

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
            current_character = context.get("character", "")
            text = context.get("text", "")

            if current_character == "":
                # 不输入角色信息的上下文，直接无视就行
                continue

            if last_character != "" and last_character != current_character:
                # 假如角色切换，则把之前的内容先处理到最后要发给AI的消息里：
                if narration_parts:
                    send_message_helper += (
                        "旁白: \n" + "\n".join(narration_parts) + "\n"
                    )
                    narration_parts.clear()
                if player_parts:
                    # 假如最后一个对话是玩家，而且后面是 AI 的对话，则保留最后一个玩家的消息直接在大括号外面
                    if last_character == "player" and current_character == character:
                        send_message_helper += (
                            (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n")
                            if len(player_parts) > 1
                            else ""
                        )
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += (
                            f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                        )

                    player_parts.clear()
                if ai_parts:
                    # send_message_main += "".join(ai_parts)
                    ai_parts.clear()

            next_character = "none"

            if i + 1 < len(game_context.dialogue):
                next_character = game_context.dialogue[i + 1].get("character", "")
                logger.info(f"下一个角色是: {next_character}")

            if current_character == "narration":
                narration_parts.append(text)

            elif current_character == "player":
                player_parts.append('"' + text + '"')

            elif current_character == character:
                # 遇到当前角色信息，则把之前的信息全打包好统计到 User 里去，更新 memory
                ai_parts.append(text)
                # 假如上一个角色不是当前角色，说明用户的输入信息已经全部完毕了，统计到 User 信息
                if last_character != current_character:
                    final_message = ""
                    if send_message_helper:
                        final_message += "{" + send_message_helper + "}\n"
                    final_message += send_message_main

                    memory.append({"role": "user", "content": final_message})
                    send_message_helper = ""
                    send_message_main = ""

                # 假如下一个角色不是当前角色，说明这是最后一句 AI 回复了，统计到 AI 信息并且结束
                if next_character != current_character:
                    ai_message += "".join(ai_parts)
                    memory.append({"role": "assistant", "content": ai_message})
                    ai_parts.clear()
                    ai_message = ""

            # 假如所有的对话都完成了
            if next_character == "none":
                if narration_parts:
                    send_message_helper += (
                        "旁白: \n" + "\n".join(narration_parts) + "\n"
                    )
                    narration_parts.clear()
                if player_parts:
                    # 假如最后一个对话是玩家，而且后面是 AI 的对话，则保留最后一个玩家的消息直接在大括号外面
                    if current_character == "player":
                        send_message_helper += (
                            (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n")
                            if len(player_parts) > 1
                            else ""
                        )
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += (
                            f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                        )

                    player_parts.clear()
                if ai_parts:
                    # send_message_main += "".join(ai_parts)
                    ai_parts.clear()

                # 把剩余的对话都打包好
                final_message = ""
                if send_message_helper:
                    final_message += "{" + send_message_helper + "}\n"
                final_message += send_message_main + send_message_tail

                memory.append({"role": "user", "content": final_message})

            last_character = current_character

    @staticmethod
    def match_ai_response_options(
        ai_response: str, options: List[Dict]
    ) -> Optional[str]:
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
        支持随机数赋值，例如: var_name = random(1, 100)
        """
        import re
        import random  

        action = action.strip()
        match = re.match(r"^([a-zA-Z_][a-zA-Z0-9_]*)\s*([+\-]?=)\s*(.+)$", action)
        if not match:
            return None, None, None

        var_name = match.group(1)
        operator = match.group(2)
        value_str = match.group(3).strip()

        # 新增：匹配 random(min, max) 语法
        random_match = re.match(r"^random\(\s*(-?\d+)\s*,\s*(-?\d+)\s*\)$", value_str)
        if random_match:
            min_val = int(random_match.group(1))
            max_val = int(random_match.group(2))
            # 容错处理：确保 min <= max
            if min_val > max_val:
                min_val, max_val = max_val, min_val
            value = random.randint(min_val, max_val)
        else:
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

        if (s.startswith('"') and s.endswith('"')) or (
            s.startswith("'") and s.endswith("'")
        ):
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
