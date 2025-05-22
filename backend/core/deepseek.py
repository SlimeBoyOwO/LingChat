from openai import OpenAI
import os
import json
import copy
from .logger import log_debug, log_info, log_error, log_text
from dotenv import load_dotenv

class DeepSeek:
    def __init__(self, api_key=None, base_url=None, logger=None):
        self.logger = logger 
        # 加载环境变量
        load_dotenv()
        
        # OpenAI API 初始化    
        api_key = api_key or os.environ.get("CHAT_API_KEY") or os.getenv("OPENAI_API_KEY")
        # 看起来增强了撸棒性，实际上也增强了撸棒性
        if not api_key:
            try:
                with open(".env", "r") as f:
                    for line in f:
                        if line.strip().startswith("CHAT_API_KEY"):
                            key_part = line.split("#")[0].strip()
                            if "=" in key_part:
                                api_key = key_part.split("=", 1)[1].strip()
                                log_debug("从.env文件直接读取API key成功")
                                break
            except Exception as e:
                log_error(f"尝试直接读取.env文件失败: {e}")
        base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
        if not api_key:
            log_error("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
            raise ValueError("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
        log_debug(f"API key 状态：{'已加载' if api_key else '未加载'}")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)

        self.settings = os.environ.get("SYSTEM_PROMPT", "你是一个AI助手，请尽可能准确地回答问题。")

        self.debug_mode = os.environ.get("DEBUG_MODE", "False").lower() == "true"
        
        self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")
        
        # 强化系统指令
        self.messages = [
            {
                "role": "system", 
                "content": self.settings
            }
        ]
        
        log_debug("DeepSeek LLM 服务已初始化")

    def process_message(self, user_input):
        if user_input.lower() in ["退出", "结束"]:
            log_info("用户请求终止程序")
            return "程序终止"
            
        self.messages.append({"role": "user", "content": user_input})

        # 若Debug模式开启，则截取发送到llm的文字信息打印到终端
        if self.debug_mode:
            log_debug("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in self.messages:
                log_debug(f"Role: {message['role']}\nContent: {message['content']}\n")
            log_debug("------ 结束 ------")

        try:
            log_debug("正在发送请求到DeepSeek LLM...")
            response = self.client.chat.completions.create(
                model=self.model_type,
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.messages.append({"role": "assistant", "content": ai_response})
            # 使用新的接口记录消息
            # log_text(str(self.messages))
            log_debug("成功获取LLM响应")

            return ai_response

        except Exception as e:
            log_error(f"LLM请求失败: {str(e)}")
            return "ERROR"

    def load_memory(self, memory):
        if isinstance(memory, str):
            memory = json.loads(memory)  # 将JSON字符串转为Python列表
        self.messages = copy.deepcopy(memory)  # 使用深拷贝
        log_info("记忆存档已经加载")
        log_info(f"内容是：{memory}")
        log_info(f"新的messages是：{self.messages}")

    def get_messsages(self):
        return self.messages