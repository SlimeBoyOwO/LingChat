from typing import Any, Dict, List, Optional
from pydantic import BaseModel, ConfigDict, Field

class VoiceModel(BaseModel):
    """
    语音模型
    """
    sva_speaker_id: Optional[str] = None
    sbv2_name: Optional[str] = None
    sbv2_speaker_id: Optional[str] = None
    bv2_speaker_id: Optional[str] = None
    sbv2api_name: Optional[str] = None
    sbv2api_speaker_id: Optional[str] = None
    gsv_voice_text: Optional[str] = None
    gsv_voice_filename: Optional[str] = None
    gsv_gpt_model_name: Optional[str] = None
    gsv_sovits_model_name: Optional[str] = None
    aivis_model_uuid: Optional[str] = None


class CharacterSettings(BaseModel):
    """
    角色设定模型
    """

    model_config = ConfigDict(extra="allow")

    # 基础信息
    ai_name: str = Field(default="ai_name未设定")
    ai_subtitle: Optional[str] = ""
    user_name: str = Field(default="user_name未设定")
    user_subtitle: Optional[str] = ""
    title: Optional[str] = None
    info: Optional[str] = None

    # 身体与外观
    body_part: Optional[Dict[str, Any]] = None
    scale: float = 1.0
    offset: float = 0.0
    clothes_name: Optional[str] = None
    clothes: Optional[List[Dict[str, str]]] = None

    # 语音与TTS
    voice_models: Optional[VoiceModel] = None
    tts_type: Optional[str] = None

    # UI / 气泡
    thinking_message: str = "正在思考中..."
    bubble_top: int = 5
    bubble_left: int = 20

    # 提示词
    system_prompt: Optional[str] = None
    system_prompt_example: Optional[str] = None
    system_prompt_example_old: Optional[str] = None

    # 内部数据
    resource_path: Optional[str] = None
    script_role_key: Optional[str] = None
    script_key: Optional[str] = None
    character_id: Optional[int] = None
