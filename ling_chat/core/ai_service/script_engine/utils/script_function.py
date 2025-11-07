from typing import Dict, Any
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker

class ScriptFunction:
    @staticmethod
    async def wait_for_user_input() -> str | None:
        """等待来自前端的用户输入"""
        # TODO: 获取客户端id
        client_id = "1"
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
    def extract_user_input(message: Dict[str, Any]) -> str:
        """从消息中提取用户输入文本"""
        try:
            # 根据实际的消息结构来提取用户输入
            # 这里假设消息中有 'text' 或 'input' 字段包含用户输入
            if isinstance(message, dict):
                return message.get('content','')
            else:
                return str(message)
        except Exception as e:
            logger.error(f"提取用户输入时发生错误: {e}")
            return ""

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
                # 不输入角色信息的上下文，直接无视就行
                continue

            if last_character != "" and last_character != current_character:
                # 假如角色切换，则把之前的内容先处理到最后要发给AI的消息里：
                if narration_parts:
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    # 假如最后一个对话是玩家，而且后面是 AI 的对话，则保留最后一个玩家的消息直接在大括号外面
                    if last_character == 'player' and current_character == character:
                        send_message_helper += (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n") if len(player_parts) > 1 else ""
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                    
                    player_parts.clear()
                if ai_parts:
                    # send_message_main += "".join(ai_parts)
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
                    send_message_helper += "旁白: \n" + "\n".join(narration_parts) + "\n"
                    narration_parts.clear()
                if player_parts:
                    # 假如最后一个对话是玩家，而且后面是 AI 的对话，则保留最后一个玩家的消息直接在大括号外面
                    if current_character == 'player':
                        send_message_helper += (f"{user_name}: \n" + "\n".join(player_parts[:-1]) + "\n") if len(player_parts) > 1 else ""
                        send_message_main += f"{player_parts[-1]}"
                    else:
                        send_message_helper += f"{user_name}: \n" + "\n".join(player_parts) + "\n"
                    
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