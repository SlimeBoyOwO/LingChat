# neochat/llm/clients/openai_compatible.py
import requests
import json
from typing import List, Dict, Generator, Any

from .base import BaseLLMClient
from neochat.platform.configuration import config as app_config
from neochat.platform.logging import log_debug, log_warning, log_error_color, log_error

class OpenAICompatibleClient(BaseLLMClient):
    """
    一个通用的客户端，用于与任何遵循 OpenAI API 规范的服务进行交互。
    """
    def __init__(self, config: Dict[str, Any]):
        super().__init__(config)
        self.api_key = config.get("api_key")
        self.base_url = config.get("base_url").rstrip('/')
        self.api_path = config.get("api_path")
        self.full_url = f"{self.base_url}{self.api_path}"

    def chat_completion(
        self,
        messages: List[Dict[str, Any]],
        model: str = None,
        stream: bool = True,
        **kwargs
    ) -> Generator[str, None, None]:
        
        if not self.api_key:
            log_error_color("致命错误: API Key 未被加载到客户端。")
            log_error("请检查: 1. .env 文件是否存在。 2. .env 文件中环境变量名是否与 config.yaml 中的 `${...}` 完全匹配。")
            return
        
        api_key_display = f"{self.api_key[:5]}...{self.api_key[-4:]}"
        log_debug(f"准备发起API请求。URL: {self.full_url}, Model: {model or self.default_model}, 使用 API Key: {api_key_display}")

        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}"
        }

        # <--- 问题修复在这里 --->
        # self.default_parameters 是一个对象，而不是字典。
        # 我们使用 vars() 将其转换为字典，然后再进行复制和更新。
        final_params = {}
        if self.default_parameters:
            final_params = vars(self.default_parameters).copy()
        
        final_params.update(kwargs)
        # <--- 修复结束 --->

        payload = {
            "model": model or self.default_model,
            "messages": messages,
            "stream": stream,
            **final_params
        }

        if app_config.debug.mode:
            self._log_request(payload)

        try:
            log_debug(f"正在向 {self.full_url} 发送 POST 请求...")
            
            response = requests.post(
                self.full_url,
                headers=headers,
                json=payload,
                stream=stream,
                timeout=app_config.llm.timeout_seconds
            )
            
            log_debug(f"收到服务器响应，状态码: {response.status_code}")
            
            response.raise_for_status()

            if stream:
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
                                    yield content_piece
                            except (json.JSONDecodeError, IndexError):
                                log_warning(f"API Stream: 解码或索引错误，数据块: {json_data_str}")
            else:
                data = response.json()
                full_content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
                yield full_content

        except requests.exceptions.HTTPError as e:
            log_error_color(f"\nAPI请求HTTP错误: {e} - {e.response.status_code} {e.response.reason}", exc_info=True)
            try:
                log_error_color(f"错误详情: {json.dumps(e.response.json(), ensure_ascii=False, indent=2)}")
            except ValueError:
                log_error_color(f"错误响应体 (非JSON): {e.response.text}")

        except requests.exceptions.RequestException as e:
            log_error_color(f"捕获到网络请求异常，具体类型: {type(e).__name__}")
            log_error("这通常是网络问题，例如无法连接到服务器、DNS解析失败、代理错误或SSL证书问题。")
            log_error_color(f"\nAPI请求失败: {e}", exc_info=True)

        except Exception as e:
            log_error_color(f"\n处理LLM响应时发生未知错误: {e}", exc_info=True)

    def _log_request(self, payload: dict):
        log_debug(f"--- 发送给 LLM API ({self.full_url}) ---")
        debug_payload = json.loads(json.dumps(payload))
        for msg in debug_payload.get("messages", []):
            if 'content' in msg and isinstance(msg['content'], str):
                msg['content'] = msg['content'].replace('\n', ' ')[:150] + ("..." if len(msg['content']) > 150 else "")
        formatted_payload = json.dumps(debug_payload, ensure_ascii=False, indent=2)
        for line in formatted_payload.splitlines():
            log_debug(line)
        log_debug("--- Payload 结束 ---")