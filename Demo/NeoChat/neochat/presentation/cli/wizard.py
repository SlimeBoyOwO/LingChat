# neochat/presentation/cli/wizard.py
import os
import json
from typing import List, Dict

# 更新 import 路径
from neochat.platform.configuration import config
from neochat.llm.client import chat_with_deepseek
from neochat.presentation.cli.ui import ConsoleUI
from neochat.platform.logging import log_info, log_error, TermColors

class SetupWizard:
    """
    负责处理应用的首次启动设置流程。
    """
    XIAO_NUO_CORE_SYSTEM_PROMPT = (
        "你的名字是小诺..." # (提示词内容不变)
    )

    def __init__(self, ui: ConsoleUI):
        self.ui = ui
        self.messages: List[Dict[str, str]] = [
            {"role": "system", "content": self.XIAO_NUO_CORE_SYSTEM_PROMPT}
        ]
        self.conversation_history = []

    @staticmethod
    def is_first_launch() -> bool:
        """检查用户配置文件是否存在，以判断是否为首次启动。"""
        return not os.path.exists(config.paths.user_config)

    def run(self):
        """执行完整的首次用户设置流程。"""
        log_info("启动新用户初始化流程...")

        # 1. 小诺开场白
        greeting_instruction = "..." # (指令内容不变)
        xiao_nuo_greeting = self._get_xiao_nuo_response(user_instruction=greeting_instruction)
        if not xiao_nuo_greeting:
            log_error("初始化失败：无法从小诺获取问候语。")
            return

        user_name_input = self.ui.prompt_for_input()
        self.messages.append({"role": "user", "content": user_name_input})
        self.conversation_history.append({"role": "user", "content": user_name_input})

        # 2. 小诺对用户名的反应及名字提取
        # (这部分逻辑与原版完全相同，无需修改)
        # ...
        
        # LLM 分析并提取用户的名字
        extract_prompt = (
            "请从以下对话历史中，提取出用户的名字。以严格的JSON格式返回，格式为 {\"name\": \"提取到的名字\"}...\n"
            # (提取逻辑的提示词不变)
        )
        # ... (提取逻辑代码不变) ...
        
        final_user_name = "玩家" # 使用一个更通用的默认名
        # ... (解析 JSON 响应的代码不变) ...
            
        self._save_user_config(final_user_name)
        self.ui.display_system_message(f"好的，你的名字已设置为 '{final_user_name}'。初始化完成！", TermColors.GREEN)

    def _get_xiao_nuo_response(self, user_instruction: str) -> str:
        """
        调用LLM获取小诺的回应并处理。
        user_instruction 是给LLM的额外指令，作为用户消息添加到当前请求中。
        """
        current_messages = list(self.messages)
        current_messages.append({"role": "user", "content": user_instruction})

        response = chat_with_deepseek(current_messages, character_name="小诺", color_code=TermColors.CYAN)
        if response:
            self.messages.append({"role": "assistant", "content": response})
            self.conversation_history.append({"role": "assistant", "content": response})
        else:
            log_error("初始化失败：无法从小诺获取回应。")
        return response

    def _save_user_config(self, user_name: str):
        """将用户信息写入 user_config.json 文件。"""
        user_data = {
            "user_name": user_name
        }
        try:
            # 确保 user_data 目录存在
            os.makedirs(os.path.dirname(config.paths.user_config), exist_ok=True)
            with open(config.paths.user_config, 'w', encoding='utf-8') as f:
                json.dump(user_data, f, ensure_ascii=False, indent=4)
            log_info(f"用户信息已保存至 {config.paths.user_config}")
        except IOError as e:
            log_error(f"保存用户信息失败: {e}")