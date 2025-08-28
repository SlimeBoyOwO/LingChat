from abc import ABC, abstractmethod


class TTSBaseAdapter(ABC):
    """VITS API适配器基类"""

    @abstractmethod
    async def generate_voice(self, text: str,) -> bytes:
        """生成语音的抽象方法"""
        pass

    @abstractmethod
    def get_params(self) -> dict[str, str|int|float|bool]:
        """获取某个适配器目前参数的抽象方法"""
        pass
