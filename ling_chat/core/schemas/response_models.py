# response_factory.py
import os
from typing import Dict

from .responses import *


class ResponseFactory:
    @staticmethod
    def create_reply(seg: Dict, user_message: str, is_final: bool) -> ReplyResponse:
        return ReplyResponse(
            character=seg.get("character", "default"),
            roleId=seg.get("role_id", None),
            emotion=seg['predicted'] or seg["original_tag"],
            originalTag=seg['original_tag'],
            message=seg['following_text'],
            ttsText=seg.get('japanese_text', None),
            motionText=seg['motion_text'],
            audioFile=os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            originalMessage=user_message,
            isFinal=is_final
        )

    @staticmethod
    def create_error_reply(error_message: str) -> ReplyResponse:
        return ReplyResponse(
            character="default",
            emotion="伤心",
            originalTag="error",
            message=error_message,
            motionText="",
            audioFile=None,
            originalMessage="",
            isFinal=True,
            duration=0
        )
    
    @staticmethod
    def create_thinking(is_thinking: bool) -> ThinkingResponse:
        return ThinkingResponse(isThinking=is_thinking)
    
    @staticmethod
    def create_chapter_change(chapter_info: str) -> ChapterChangeResponse:
        return ChapterChangeResponse(chapterName=chapter_info)
    
    @staticmethod
    def create_choice(choices: list[str],allow_free: bool = False) -> ChoiceResponse:
        return ChoiceResponse(choices=choices, allowFree=allow_free)

    @staticmethod
    def create_input(hint: str, **kwargs) -> ScriptInputResponse:
        return ScriptInputResponse(hint=hint, isFinal=True, **kwargs)

    @staticmethod
    def create_background(image: str, **kwargs) -> ScriptBackgroundResponse:
        return ScriptBackgroundResponse(imagePath=image, **kwargs)

    @staticmethod
    def create_background_effect(effect: str, **kwargs) -> ScriptBackgroundEffectResponse:
        return ScriptBackgroundEffectResponse(effect=effect, **kwargs)

    @staticmethod
    def create_sound(sound: str, **kwargs) -> ScriptSoundResponse:
        return ScriptSoundResponse(soundPath=sound, **kwargs)

    @staticmethod
    def create_music(music: str, **kwargs) -> ScriptMusicResponse:
        return ScriptMusicResponse(musicPath=music, **kwargs)

    @staticmethod
    def create_narration(text: str) -> ScriptNarrationResponse:
        return ScriptNarrationResponse(text=text)

    @staticmethod
    def create_player_dialogue(text: str) -> ScriptPlayerResponse:
        return ScriptPlayerResponse(text=text)

    @staticmethod
    def create_modify_character(**kwargs) -> ScriptModifyCharacterResponse:
        return ScriptModifyCharacterResponse(**kwargs)
    
    @staticmethod
    def create_script_end() -> ScriptEndResponse:
        return ScriptEndResponse()

