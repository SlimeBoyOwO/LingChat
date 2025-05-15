from openai import OpenAI
import os
from .logger import Logger

class DeepSeek:
    def __init__(self, api_key=None, base_url=None, logger=None):
        self.logger = logger or Logger()
        
        # OpenAI API 初始化    
        api_key = api_key or os.environ.get("CHAT_API_KEY")
        base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)

        self.settings = os.environ.get("SYSTEM_PROMPT")

        self.debug_mode = os.environ.get("DEBUG_MODE", "False").lower() == "true"
        
        # 强化系统指令
        self.messages = [
            {
                "role": "system", 
                "content": self.settings
            }
        ]
        
        self.logger.debug("DeepSeek LLM 服务已初始化")

    def process_message(self, user_input):
        if user_input.lower() in ["退出", "结束"]:
            self.logger.info("用户请求终止程序")
            return "程序终止"
            
        self.messages.append({"role": "user", "content": user_input})

        # 若Debug模式开启，则截取发送到llm的文字信息打印到终端
        if self.debug_mode:
            self.logger.debug("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in self.messages:
                self.logger.debug(f"Role: {message['role']}\nContent: {message['content']}\n")
            self.logger.debug("------ 结束 ------")

        try:
            self.logger.debug("正在发送请求到DeepSeek LLM...")
            response = self.client.chat.completions.create(
                model=os.environ.get("MODEL_TYPE"),
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.messages.append({"role": "assistant", "content": ai_response})
            # self.logger.log_text(self.messages)
            self.logger.debug("成功获取LLM响应")

            return ai_response

        except Exception as e:
            self.logger.error(f"LLM请求失败: {str(e)}")
            return "ERROR"

    def load_memory(self, memory):
        self.messages = memory
        self.logger.log_text("记忆存档已经加载")
