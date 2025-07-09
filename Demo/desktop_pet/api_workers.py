# file: api_workers.py

from PyQt6.QtCore import QThread, pyqtSignal
from openai import OpenAI

from config import CONFIG
from logger import Logger

# Get the singleton logger instance
logger = Logger()

class ChatWorker(QThread):
    response_ready = pyqtSignal(str)
    error_occurred = pyqtSignal(str)

    def __init__(self, messages):
        super().__init__()
        self.messages = messages
        self.client = OpenAI(
            api_key=CONFIG["CHAT_API_KEY"],
            base_url=CONFIG["CHAT_BASE_URL"],
        )

    def run(self):
        try:
            response = self.client.chat.completions.create(
                model=CONFIG["CHAT_MODEL"],
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.response_ready.emit(ai_response)
        except Exception as e:
            logger.error(f"聊天API请求出错: {e}", exc_info=True)
            self.error_occurred.emit(f"呜...网络出错了，请检查API Key和网络连接。")

class VisionWorker(QThread):
    description_ready = pyqtSignal(str)
    error_occurred = pyqtSignal(str)

    def __init__(self, base64_image):
        super().__init__()
        self.base64_image = base64_image
        self.vision_client = OpenAI(
            api_key=CONFIG["VD_API_KEY"],
            base_url=CONFIG["VD_BASE_URL"],
        )

    def run(self):
        try:
            logger.info("向视觉模型发送请求以获取描述...")
            vision_response = self.vision_client.chat.completions.create(
                model=CONFIG["VD_MODEL"],
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": CONFIG["SCREENSHOT_PROMPT"]},
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url": f"data:image/png;base64,{self.base64_image}"
                                },
                            },
                        ],
                    }
                ],
                max_tokens=300,
            )
            image_description = vision_response.choices[0].message.content
            logger.info(f"视觉模型返回的描述: {image_description}")
            self.description_ready.emit(image_description)
        except Exception as e:
            logger.error(f"视觉模型请求出错: {e}", exc_info=True)
            self.error_occurred.emit(f"呜...分析图片时出错了，请检查VD_API_KEY和网络。")