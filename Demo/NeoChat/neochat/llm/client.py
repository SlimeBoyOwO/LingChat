# neochat/llm/client.py
import sys
import json
from typing import List, Dict, Any # 引入 List, Dict, Any

from neochat.llm import llm_manager
# <--- 新增导入 config 对象 --->
from neochat.platform.configuration import config
from neochat.platform.logging import (
    start_loading_animation,
    stop_loading_animation,
    log_error,
    log_debug, # 引入 log_debug 用于调试
    TermColors
)


# <--- [新增] 辅助函数：合并连续的用户消息 --->
def _merge_consecutive_user_messages(messages: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """
    将消息列表中连续的 'user' 角色消息合并成一条。
    """
    if not messages:
        return []

    merged_messages = []
    for msg in messages:
        # 创建一个副本以避免修改原始数据
        current_msg = msg.copy()
        
        # 检查当前消息是否为 'user' 角色，并且合并列表中的最后一条消息也是 'user' 角色
        if (current_msg.get("role") == "user" and 
                merged_messages and 
                merged_messages[-1].get("role") == "user"):
            
            # 将当前消息内容追加到上一条消息中，用换行符分隔
            merged_messages[-1]["content"] += "\n" + str(current_msg.get("content", ""))
        else:
            # 否则，直接将当前消息添加到新列表中
            merged_messages.append(current_msg)
            
    return merged_messages


def generate_chat_response(messages_payload, character_name="AI", is_internal_thought=False, color_code=TermColors.CYAN, **kwargs):
    """
    与 LLM 进行通信的高级封装函数，包含 UI 交互。
    
    :param messages_payload: 发送给 API 的消息列表。
    :param character_name: 显示在输出中的角色名。
    :param is_internal_thought: 是否为内部思考（不显示加载动画和流式输出）。
    :param color_code: 角色对话颜色。
    :param kwargs: 传递给 LLM 客户端的其他参数 (如 model, temperature)。
    :return: API 返回的完整响应字符串，或在失败时返回 None。
    """
    # <--- [修改] 在此处应用消息合并逻辑 --->
    final_messages_payload = messages_payload
    if config.llm.conversation_settings.merge_consecutive_user_messages:
        original_count = len(messages_payload)
        final_messages_payload = _merge_consecutive_user_messages(messages_payload)
        merged_count = len(final_messages_payload)
        if original_count > merged_count:
            log_debug(f"消息合并已执行: 消息数从 {original_count} 条减少到 {merged_count} 条。")

    full_response = ""
    api_call_succeeded = False
    animation_stopped = False

    try:
        if not is_internal_thought:
            animation_msg = f"{TermColors.LIGHT_BLUE}{character_name} 正在思考...{TermColors.RESET}"
            start_loading_animation(message=animation_msg, animation_style_key='moon')

        # 通过 llm_manager 调用，使用处理后的 final_messages_payload
        response_generator = llm_manager.client.chat_completion(
            messages=final_messages_payload, # <--- 使用处理后的消息列表
            stream=True,
            **kwargs
        )

        first_chunk = True
        for chunk in response_generator:
            if not is_internal_thought:
                if first_chunk:
                    stop_loading_animation()
                    animation_stopped = True
                    print(f"{color_code}{character_name}:{TermColors.RESET} ", end="", flush=True)
                    first_chunk = False
                sys.stdout.write(f"{color_code}{chunk}{TermColors.RESET}")
                sys.stdout.flush()
            
            full_response += chunk
        
        if not is_internal_thought and not first_chunk:
            print() # 确保流式输出后换行

        api_call_succeeded = True

    except Exception as e:
        log_error(f"在 'generate_chat_response' 中捕获到未处理的异常: {e}", exc_info=True)
        pass 
    finally:
        if not is_internal_thought and not animation_stopped:
            stop_loading_animation(success=api_call_succeeded, final_message="与API通信中断" if not api_call_succeeded else None)

    return full_response if api_call_succeeded else None