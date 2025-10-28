# responses.py
from pydantic import BaseModel
from .response_types import ResponseType
from typing import Optional, Union

class BaseResponse(BaseModel):
    type: str

class ReplyResponse(BaseResponse):
    type: str = ResponseType.AI_REPLY
    emotion: str
    originalTag: str
    message: str
    motionText: Optional[str] = None
    audioFile: Optional[str] = None
    originalMessage: str
    isFinal: bool

class ScriptBackgroundResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_BACKGROUND
    image: str

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

class ScriptPlayerResponse(BaseResponse):
    type: str = ResponseType.SCRIPT_PLAYER
    text: str

# 所有响应类型
Response = Union[ReplyResponse, ScriptBackgroundResponse, ScriptNarrationResponse]