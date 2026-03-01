import base64
import os
import time
import asyncio
from io import BytesIO

import httpx
from PIL import ImageGrab

from ling_chat.core.logger import logger

class DesktopAnalyzer:
    def __init__(self):
        """
        初始化桌面分析器
        """
        self.api_key = os.environ.get("VD_API_KEY", "")
        self.base_url = os.environ.get("VD_BASE_URL", "https://api.siliconflow.cn/v1/chat/completions")
        self.model = os.environ.get("VD_MODEL", "Pro/Qwen/Qwen2.5-VL-7B-Instruct")

        if(self.api_key == "sk-114514" or self.api_key == ""):
            logger.info("【视觉识别】你没有改过VD_API_KEY，无法进行图像识别哦！")
        else:
            logger.info("【视觉识别】你填写了VD_API_KEY，现在你可以输入“看看我的桌面”加任意提示词实现让灵灵看桌面的功能哦~")

        self.last_response_time = None
        self.last_input_tokens = None
        self.last_output_tokens = None

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

    async def analyze_desktop(self, prompt="这是用户的桌面内容，请你用100字左右描绘主要内容，边角内容如任务栏不需要分析"):
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

        # 构建请求头和数据
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }

        payload = {
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {"type": "text", "text": prompt},
                        {"type": "image_url", "image_url": {"url": image_data}}
                    ]
                }
            ],
            "max_tokens": 1024
        }

        # 记录开始时间
        start_time = time.time()
        
        # 使用异步客户端发送请求
        # timeout 设置为 60 秒防止网络波动导致长时间挂起
        async with httpx.AsyncClient(timeout=60.0) as client:
            try:
                response = await client.post(self.base_url, headers=headers, json=payload)
                response.raise_for_status() # 检查 HTTP 错误
                response_data = response.json()
            except httpx.HTTPError as e:
                logger.error(f"【视觉识别】网络请求失败: {e}")
                raise Exception(f"请求失败: {e}")

        if "choices" not in response_data:
            raise Exception(f"API返回格式异常: {response_data}")

        content = response_data["choices"][0]["message"]["content"]
        logger.info("图片分析结果如下")
        logger.info(content)

        # 记录性能数据
        self.last_response_time = time.time() - start_time
        self.last_input_tokens = response_data.get("usage", {}).get("prompt_tokens", 0)
        self.last_output_tokens = response_data.get("usage", {}).get("completion_tokens", 0)

        return content

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


if __name__ == "__main__":
    # 单元测试需要使用 asyncio.run 来运行异步函数
    async def main():
        analyzer = DesktopAnalyzer()

        print("桌面内容分析器已启动...")
        input("按Enter键截取当前桌面并发送给AI分析...")

        # 执行分析
        try:
            print("正在异步分析中，请稍候...")
            description = await analyzer.analyze_desktop()
            report = analyzer.get_analysis_report()

            # 显示结果
            print("\n" + "=" * 50)
            print("AI生成的桌面描述:")
            print("-" * 50)
            print(description)
            print("\n" + "=" * 50)
            print("性能报告:")
            print("-" * 50)
            print(f"响应时间: {report['response_time']}秒")
            print(f"输入Token: {report['input_tokens']}")
            print(f"输出Token: {report['output_tokens']}")
            print(f"计算费用: ¥{report['cost']}")
            print("=" * 50)
        except Exception as e:
            print(f"发生错误: {str(e)}")

    asyncio.run(main())