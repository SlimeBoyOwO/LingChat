from dataclasses import dataclass, field, asdict
from typing import Optional, List, Dict, Any, Self

from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.schemas.character_settings import CharacterSettings, VoiceModel

@dataclass
class GameMemoryBankMeta:
    """
    MemoryBank 元信息（运行时缓存与 DB JSON 共用）
    - last_processed_global_idx: 用全局 line_list 的索引作为“已归档指针”
    """
    last_processed_global_idx: int = 0
    updated_at: str = ""

    def model_dump(self) -> Dict[str, Any]:
        return asdict(self)

    @classmethod
    def model_validate(cls, obj: Dict[str, Any] | None) -> "GameMemoryBankMeta":
        obj = obj or {}
        return cls(
            last_processed_global_idx=int(obj.get("last_processed_global_idx", 0) or 0),
            updated_at=str(obj.get("updated_at", "") or ""),
        )


@dataclass
class GameMemoryBankData:
    """
    MemoryBank 的结构化正文。
    """
    short_term: str = "暂无近期对话摘要。"
    long_term: str = "暂无长期关键经历。"
    user_info: str = "暂无用户特征记录。"
    promises: str = "暂无未完成的约定。"

    def model_dump(self) -> Dict[str, Any]:
        return asdict(self)

    @classmethod
    def model_validate(cls, obj: Dict[str, Any] | None) -> "GameMemoryBankData":
        obj = obj or {}
        return cls(
            short_term=str(obj.get("short_term", cls().short_term) or cls().short_term),
            long_term=str(obj.get("long_term", cls().long_term) or cls().long_term),
            user_info=str(obj.get("user_info", cls().user_info) or cls().user_info),
            promises=str(obj.get("promises", cls().promises) or cls().promises),
        )


@dataclass
class GameMemoryBank:
    """
    运行时缓存用的 MemoryBank 结构。
    约定：与 DB MemoryBank.info 的 JSON 结构保持一致：
    {
      "schema_version": 1,
      "meta": {...},
      "data": {...}
    }
    """
    schema_version: int = 1
    meta: GameMemoryBankMeta = field(default_factory=GameMemoryBankMeta)
    data: GameMemoryBankData = field(default_factory=GameMemoryBankData)

    def model_dump(self) -> Dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "meta": self.meta.model_dump(),
            "data": self.data.model_dump(),
        }

    @classmethod
    def model_validate(cls, obj: Dict[str, Any] | None) -> "GameMemoryBank":
        obj = obj or {}
        schema_version = int(obj.get("schema_version", 1) or 1)
        meta = GameMemoryBankMeta.model_validate(obj.get("meta"))
        data = GameMemoryBankData.model_validate(obj.get("data"))
        return cls(schema_version=schema_version, meta=meta, data=data)

    def to_prompt_text(self) -> str:
        """
        将结构化记忆渲染为注入到 system prompt 的文本。
        注意：为了保持 LLM 消息结构标准，推荐“合并进同一条 system”，而不是新增 system 消息。
        """
        return (
            "\n\n====== 核心记忆库 (Memory Bank) ======\n"
            f"【用户信息】：{self.data.user_info}\n"
            f"【重要约定】：{self.data.promises}\n"
            f"【长期经历】：{self.data.long_term}\n"
            f"【近期回顾】：{self.data.short_term}\n"
            "====================================\n"
        )

@dataclass
class GameRole:
    """
    游戏角色数据模型
    """
    role_id: Optional[int] = None
    memory: List[Dict[str, Any]] = field(default_factory=list)
    
    display_name: Optional[str] = None
    settings: CharacterSettings = field(default_factory=CharacterSettings)
    resource_path: Optional[str] = None
    prompt: Optional[str] = None
    memory_bank: GameMemoryBank = field(default_factory=GameMemoryBank)

    voice_maker: VoiceMaker = field(default_factory=VoiceMaker)

    def __post_init__(self):
        """
        在 dataclass 自动生成的 __init__ 执行完毕后，自动调用此方法。
        用于初始化 VoiceMaker 的配置。
        """
        if self.resource_path:
            self.voice_maker.set_character_path(self.resource_path)
            
        current_tts_type = self.settings.tts_type or "sbv"
        role_name = self.display_name if self.display_name else f"Role_{self.role_id}"

        self.voice_maker.set_tts(
            tts_type=current_tts_type,
            tts_settings=self.settings.voice_models if self.settings.voice_models else VoiceModel(),
            name=role_name
        )
    
    def __hash__(self):
        return hash(self.role_id) if self.role_id is not None else id(self)
    
    def __eq__(self, other):
        if not isinstance(other, GameRole):
            return False
        if self.role_id is None or other.role_id is None:
            return self is other
        return self.role_id == other.role_id

@dataclass
class Player:
    user_name: str = ""
    user_subtitle: str = ""
    user_prompt: str = "" # 用于设定玩家的信息，如性格、喜好等

@dataclass
class ScriptStatus:
    folder_key: str

    name: str
    description: str
    intro_chapter: str
    settings: dict

    # 正在进行剧本模式的 client
    running_client_id: Optional[str] = None

    # 记录剧本进度
    current_chapter_key: str = field(default_factory=str)
    current_event_process: int = field(default_factory=int)

    # 剧本包含的变量
    vars: dict = field(default_factory=dict)
