# response_factory.py
from .responses import *
from typing import Dict
import os

class ResponseFactory:
    @staticmethod
    def create_reply(seg: Dict, user_message: str, is_final: bool) -> ReplyResponse:
        return ReplyResponse(
            emotion=seg['predicted'] or seg["original_tag"],
            originalTag=seg['original_tag'],
            message=seg['following_text'],
            motionText=seg['motion_text'],
            audioFile=os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            originalMessage=user_message,
            isFinal=is_final
        )
    
    @staticmethod
    def create_background(image: str, **kwargs) -> ScriptBackgroundResponse:
        return ScriptBackgroundResponse(image=image, **kwargs)
    
    @staticmethod
    def create_narration(text: str) -> ScriptNarrationResponse:
        return ScriptNarrationResponse(text=text)

    @staticmethod
    def create_player_dialogue(text: str) -> ScriptPlayerResponse:
        return ScriptPlayerResponse(text=text)

