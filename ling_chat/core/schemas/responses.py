# responses.py
from unittest.mock import Base
from numpy import character
from pydantic import BaseModel
from .response_types import ResponseType
from typing import Optional, Union

class BaseResponse(BaseModel):
    type: str
    duration: float = -1
    isFinal: bool = False

class ReplyResponse(BaseResponse):
    type: str = ResponseType.AI_REPLY
    character: Optional[str] = None
    emotion: str
    originalTag: str
    message: str
    motionText: Optional[str] = None
    audioFile: Optional[str] = None
    originalMessage: str

class ScriptBackgroundResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_BACKGROUND
    imagePath: str

class ScriptBackgroundEffectResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_BACKGROUND_EFFECT
    effect: str

class ScriptSoundResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_SOUND
    soundPath: str

class ScriptMusicResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_MUSIC
    musicPath: str

class ScriptNarrationResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_NARRATION
    text: str

class ScriptDialogResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_DIALOG
    emotion: str
    originalTag: str
    message: str
    motionText: Optional[str] = None
    audioFile: Optional[str] = None

class ScriptModifyCharacterResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_MODIFY_CHARACTER
    character: str
    emotion: Optional[str] = None
    action: Optional[str] = None

class ScriptPlayerResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_PLAYER
    text: str

class ScriptInputResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_INPUT
    hint: str

# 所有响应类型
Response = Union[ReplyResponse, ScriptBackgroundResponse, ScriptNarrationResponse]