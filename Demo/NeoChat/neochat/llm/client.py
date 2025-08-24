# neochat/llm/client.py
import requests
import json
import sys

# 导入新的配置和日志模块
from neochat.platform.configuration import config
from neochat.platform.logging import (
    log_debug,
    log_warning,
    log_error_color,
    start_loading_animation,
    stop_loading_animation,
    TermColors
)

def chat_with_deepseek(messages_payload, character_name="AI", is_internal_thought=False, color_code=TermColors.CYAN):
    """
    与 DeepSeek API 进行通信的核心函数。
    
    :param messages_payload: 发送给 API 的消息列表。
    :param character_name: 显示在输出中的角色名。
    :param is_internal_thought: 是否为内部思考（不显示加载动画和流式输出）。
    :param color_code: 角色对话颜色。
    :return: API 返回的完整响应字符串，或在失败时返回 None。
    """
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {config.llm.api_key}"
    }
    payload = {
        "model": config.llm.model_name,
        "messages": messages_payload,
        "stream": True,
        "max_tokens": config.llm.max_tokens,
        "temperature": config.llm.temperature
    }

    if config.debug.mode:
        log_debug(f"--- 发送给 DeepSeek API 的 Payload (角色: {character_name}) ---")
        # 为了日志清晰，截断过长的 content
        debug_payload_display = json.loads(json.dumps(payload))
        for msg in debug_payload_display.get("messages", []):
            if 'content' in msg and isinstance(msg['content'], str):
                msg['content'] = msg['content'].replace('\n', ' ')[:150] + ("..." if len(msg['content']) > 150 else "")
        formatted_payload_str = json.dumps(debug_payload_display, ensure_ascii=False, indent=2)
        for line in formatted_payload_str.splitlines():
            log_debug(line)
        log_debug("--- Payload 结束 ---")

    assistant_full_response = ""
    api_call_succeeded = False
    animation_stopped_internally = False

    try:
        if not is_internal_thought:
            animation_msg = f"{TermColors.LIGHT_BLUE}{character_name} 正在思考...{TermColors.RESET}"
            start_loading_animation(message=animation_msg, animation_style_key='moon')

        response = requests.post(
            config.llm.api_url,
            headers=headers,
            json=payload,
            stream=True,
            timeout=config.llm.timeout_seconds
        )
        response.raise_for_status()

        first_chunk_received = False
        for chunk in response.iter_lines():
            if chunk:
                decoded_line = chunk.decode('utf-8')
                if decoded_line.startswith("data: "):
                    json_data_str = decoded_line[len("data: "):]
                    if json_data_str.strip() == "[DONE]":
                        break
                    try:
                        data = json.loads(json_data_str)
                        content_piece = data.get("choices", [{}])[0].get("delta", {}).get("content", "")
                        if content_piece:
                            if not is_internal_thought:
                                if not first_chunk_received:
                                    stop_loading_animation()
                                    animation_stopped_internally = True
                                    print(f"{color_code}{character_name}:{TermColors.RESET} ", end="", flush=True)
                                    first_chunk_received = True
                                sys.stdout.write(f"{color_code}{content_piece}{TermColors.RESET}")
                                sys.stdout.flush()
                            assistant_full_response += content_piece
                    except (json.JSONDecodeError, IndexError):
                        log_warning(f"API Stream: 解码或索引错误，数据块: {json_data_str}")

        if not is_internal_thought:
            if first_chunk_received:
                print() # 确保流式输出后换行
            api_call_succeeded = True
        else:
            api_call_succeeded = True

    except requests.exceptions.HTTPError as e:
        log_error_color(f"\nAPI请求HTTP错误: {e} - {e.response.status_code} {e.response.reason}")
        try:
            log_error_color(f"错误详情: {json.dumps(e.response.json(), ensure_ascii=False, indent=2)}")
        except ValueError:
            log_error_color(f"错误响应体 (非JSON): {e.response.text}")
    except requests.exceptions.RequestException as e:
        log_error_color(f"\nAPI请求失败: {e}")
    finally:
        if not is_internal_thought and not animation_stopped_internally:
            stop_loading_animation(success=api_call_succeeded, final_message="与API通信中断" if not api_call_succeeded else None)

    return assistant_full_response if api_call_succeeded else None