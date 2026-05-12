import base64
import os
import uuid
from pathlib import Path

import httpx
import requests
from openai import OpenAI

from ling_chat.core.logger import logger


class ImageGenerator:
    """AI 图片生成器，使用 OpenAI 兼容的 images.generate 接口"""

    def __init__(self):
        self.api_key = os.environ.get("IMAGE_API_KEY", "")
        self.base_url = os.environ.get("IMAGE_BASE_URL", "https://api.openai.com/v1")

        if not self.api_key:
            raise ValueError("IMAGE_API_KEY 环境变量未设置，无法使用图片生成功能。")

        # 图片生成耗时较长，设置较大的超时时间
        self._timeout = httpx.Timeout(connect=20.0, read=300.0, write=20.0, pool=20.0)
        self.client = OpenAI(
            api_key=self.api_key,
            base_url=self.base_url,
            timeout=self._timeout,
        )

    def generate(self, prompt: str) -> bytes:
        """
        调用 images.generate 接口生成图片

        :param prompt: 生成提示词
        :return: 图片的原始字节数据
        """
        logger.info(f"开始生成 AI 背景图，提示词: {prompt[:50]}...")

        response = self.client.images.generate(
            model="gpt-image-1",
            prompt=prompt,
            n=1,
            size="2880x2048",  # type: ignore
            quality="auto",
        )

        if not response.data or len(response.data) == 0:
            logger.error(f"API 返回的数据为空，完整响应: {response}")
            raise ValueError("API 返回的图片数据为空")

        image_data = response.data[0]
        image_bytes = None

        # 尝试方式1: 使用b64_json
        if hasattr(image_data, "b64_json") and image_data.b64_json:
            image_bytes = base64.b64decode(image_data.b64_json)

        # 尝试方式2: 使用url
        elif hasattr(image_data, "url") and image_data.url:
            print(f"从URL下载图片: {image_data.url}")
            try:
                img_response = requests.get(image_data.url, timeout=30)
                if img_response.status_code == 200:
                    image_bytes = img_response.content
                else:
                    logger.error(f"从URL下载图片失败: {img_response.status_code}")
            except Exception as e:
                logger.error(f"从URL下载图片失败: {e}")

        if image_bytes is None:
            logger.warning(f"出错的图片请求回复，源数据: {response}")
            raise Exception("API 返回的内容不包含图片数据")

        logger.info(f"AI 背景图生成成功，大小: {len(image_bytes)} bytes")
        return image_bytes

    def save_image(self, image_bytes: bytes, output_dir: Path) -> str:
        """
        保存图片到指定目录

        :param image_bytes: 图片的原始字节数据
        :param output_dir: 输出目录
        :return: 保存的文件名
        """
        output_dir.mkdir(parents=True, exist_ok=True)

        filename = f"ai_{uuid.uuid4().hex[:8]}.png"
        filepath = output_dir / filename
        filepath.write_bytes(image_bytes)

        logger.info(f"AI 背景图已保存: {filepath}")
        return filename
