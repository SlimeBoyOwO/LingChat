from abc import ABC, abstractmethod


class BaseVitsAdapter(ABC):
    """VITS API适配器基类"""

    @abstractmethod
    async def generate_voice(self, text: str, params: dict) -> bytes:
        """生成语音的抽象方法"""
        pass

    @abstractmethod
    def get_default_params(self) -> dict:
        """获取默认参数的抽象方法"""
        pass
