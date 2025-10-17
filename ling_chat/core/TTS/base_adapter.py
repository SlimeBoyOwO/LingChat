from abc import ABC, abstractmethod
from typing import Optional, AsyncGenerator


class TTSBaseAdapter(ABC):
    """VITS API适配器基类"""

    @abstractmethod
    async def generate_voice(self, text: str,) -> bytes:
        """生成语音的抽象方法"""
        pass

    async def generate_voice_stream(self, text: str) -> Optional[AsyncGenerator[bytes, None]]:
        """
        生成流式语音的默认实现
        对于不支持流式的引擎，可以返回None或抛出异常
        """
        # 默认返回None表示不支持
        return None

    @abstractmethod
    def get_params(self) -> dict[str, str|int|float|bool]:
        """获取某个适配器目前参数的抽象方法"""
        pass
