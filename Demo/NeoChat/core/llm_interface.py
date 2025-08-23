import requests
import json
import sys
import time

import config
from .logger import (
    log_debug,
    log_info_color,
    log_warning,
    log_error_color,
    start_loading_animation,
    stop_loading_animation,
    TermColors
)

def chat_with_deepseek(messages_payload, character_name="AI", is_internal_thought=False, color_code=TermColors.CYAN):
    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {config.API_KEY}"}
    payload = {
        "model": config.MODEL_NAME, "messages": messages_payload, "stream": True,
        "max_tokens": config.MAX_TOKENS, "temperature": config.TEMPERATURE
    }

    if config.DEBUG_MODE:
        log_debug(f"--- 发送给 DeepSeek API 的 Payload (角色: {character_name}) ---")
        debug_payload_display = json.loads(json.dumps(payload))
        for msg in debug_payload_display.get("messages", []):
            if 'content' in msg and isinstance(msg['content'], str):
                msg['content'] = msg['content'].replace('\n', ' ')[:150] + ("..." if len(msg['content']) > 150 else "")
        formatted_payload_str = json.dumps(debug_payload_display, ensure_ascii=False, indent=2)
        for line in formatted_payload_str.splitlines(): log_debug(line)
        log_debug("--- Payload 结束 ---")

    assistant_full_response = ""
    api_call_succeeded = False
    animation_stopped_internally = False

    try:
        if not is_internal_thought:
            animation_msg = f"{TermColors.LIGHT_BLUE}{character_name} 正在思考...{TermColors.RESET}"
            start_loading_animation(
                message=animation_msg,
                animation_style_key='moon',
            )

        response = requests.post(config.API_URL, headers=headers, json=payload, stream=True,
                                 timeout=config.API_TIMEOUT_SECONDS)
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
                print()
                api_call_succeeded = True
            elif response.ok:
                log_warning("API 响应流结束，但未返回任何文本内容。")
                api_call_succeeded = True
        else:
            # 内部思考模式，只要有响应就算成功
            api_call_succeeded = response.ok

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
