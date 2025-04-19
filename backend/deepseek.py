from openai import OpenAI
import os

class DeepSeek:
    def __init__(self, api_key=None, base_url=None):
        # OpenAI API 初始化    
        api_key = api_key or os.environ.get("CHAT_API_KEY")
        base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)

        self.settings = os.environ.get("SYSTEM_PROMPT")
        
        # 强化系统指令
        self.messages = [
            {
                "role": "system", 
                "content": self.settings
            }
        ]

    def process_message(self, user_input):
        if user_input.lower() in ["退出", "结束"]:
            print("再见！")
            return "程序终止"
        self.messages.append({"role": "user", "content": user_input})

        try:
            response = self.client.chat.completions.create(
                model="deepseek-chat",
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.messages.append({"role": "assistant", "content": ai_response})

            return ai_response

        except Exception as e:
            print(f"\n错误: {str(e)}")
            return "ERROR"