import base64
import os
import time
import asyncio
from io import BytesIO

import httpx
from openai import OpenAI
from PIL import ImageGrab

class DesktopAnalyzer:
    def __init__(self):
        """
        初始化桌面分析器
        """
        self.api_key = os.environ.get("VD_API_KEY", "")
        if not self.api_key:
            raise ValueError("VD_API_KEY 环境变量未设置。请设置您的API密钥。")

        self.base_url = os.environ.get("VD_BASE_URL", "https://dashscope.aliyuncs.com/compatible-mode/v1")
        self.model = os.environ.get("VD_MODEL", "qwen3.5-plus")

        self.last_response_time = None
        self.last_input_tokens = None
        self.last_output_tokens = None
        
        # 初始化 OpenAI 客户端
        self._timeout = httpx.Timeout(15.0)
        self.client = OpenAI(
            api_key=self.api_key,
            base_url=self.base_url,
            timeout=self._timeout
        )

    def _capture_desktop_sync(self):
        """
        同步的截图逻辑（专门用于在线程中运行）
        """
        # 截取屏幕
        screenshot = ImageGrab.grab()

        # 将截图转为Base64编码 (这个过程涉及图片压缩，是同步阻塞的)
        buffered = BytesIO()
        screenshot.save(buffered, format="PNG")
        base64_image = base64.b64encode(buffered.getvalue()).decode('utf-8')
        return base64_image

    async def capture_desktop(self):
        """
        异步截取整个桌面并返回Base64编码
        使用 asyncio.to_thread 防止图片处理阻塞主事件循环
        """
        return await asyncio.to_thread(self._capture_desktop_sync)

    @staticmethod
    def calculate_cost(input_tokens, output_tokens):
        """
        计算分析费用
        """
        # 注意：这里需要根据趋动云的实际计费标准调整
        input_cost = (input_tokens / 1000) * 0.00035
        output_cost = (output_tokens / 1000) * 0.00035
        return round(input_cost + output_cost, 4)

    async def analyze_desktop(self, prompt="这是用户的桌面内容，请你用100字左右描绘主要内容是什么，边角内容如任务栏不需要分析"):
        """
        执行桌面分析 (异步)
        
        Args:
            prompt (str): 发送给AI的提示文本
            
        Returns:
            str: AI生成的描述文本
        """
        # 异步获取截图（不卡死主线程）
        desktop_base64 = await self.capture_desktop()
        image_data = f"data:image/png;base64,{desktop_base64}"

        # 记录开始时间
        start_time = time.time()
        
        # 使用 OpenAI 客户端发送请求
        try:
            # 使用 asyncio.to_thread 将同步调用转为异步
            completion = await asyncio.to_thread(
                self.client.chat.completions.create,
                model=self.model,
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": prompt},
                            {"type": "image_url", "image_url": {"url": image_data}}
                        ]
                    }
                ],
                max_tokens=1024
            )
            
            # 获取响应数据
            content = completion.choices[0].message.content
            print("图片分析结果如下")
            print(content)

            # 记录性能数据
            self.last_response_time = time.time() - start_time
            self.last_input_tokens = completion.usage.prompt_tokens if completion.usage else -1
            self.last_output_tokens = completion.usage.completion_tokens if completion.usage else -1

            return content
        except Exception as e:
            raise Exception(f"请求失败: {e}")

    def get_analysis_report(self):
        """
        获取最后一次分析的报告
        """
        if not self.last_response_time:
            return {"error": "No analysis performed yet"}

        return {
            "response_time": round(self.last_response_time, 2),
            "input_tokens": self.last_input_tokens,
            "output_tokens": self.last_output_tokens,
            "cost": self.calculate_cost(self.last_input_tokens, self.last_output_tokens)
        }
