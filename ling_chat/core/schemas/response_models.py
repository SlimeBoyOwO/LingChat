# response_factory.py
import os
from typing import Dict, Optional

from ling_chat.core.schemas.responses import (
    ChapterChangeResponse,
    ChoiceResponse,
    FreeDialogueResponse,
    ReplyResponse,
    ScriptBackgroundEffectResponse,
    ScriptBackgroundResponse,
    ScriptEndResponse,
    ScriptInputResponse,
    ScriptModifyCharacterResponse,
    ScriptMusicResponse,
    ScriptNarrationResponse,
    ScriptPlayerResponse,
    ScriptPresentPicResponse,
    ScriptSoundResponse,
    ThinkingResponse,
)

"""
响应工厂类，用于创建各种类型的响应对象
"""


class ResponseFactory:
    @staticmethod
    def create_reply(
        seg: Dict,  # 段落字典，包含角色相关信息
        user_message: str,  # 用户消息内容
        is_final: bool,  # 是否为最终回复
        display_name: Optional[str] = None,  # 可选的显示名称
        display_subtitle: Optional[str] = None,  # 可选的显示副标题
    ) -> ReplyResponse:
        return ReplyResponse(
            character=seg.get("character", "default"),
            roleId=seg.get("role_id", None),
            emotion=seg["predicted"] or seg["original_tag"],
            originalTag=seg["original_tag"],
            message=seg["following_text"],
            ttsText=seg.get("japanese_text", None),
            motionText=seg["motion_text"],
            audioFile=os.path.basename(seg["voice_file"])
            if os.path.exists(seg["voice_file"])
            else None,
            originalMessage=user_message,
            isFinal=is_final,
            displayName=display_name,
            displaySubtitle=display_subtitle,
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
        )

    @staticmethod
    def create_free_dialogue(
        switch: bool, max_rounds: int, end_line: str
    ) -> FreeDialogueResponse:
        return FreeDialogueResponse(
            switch=switch,
            maxRounds=max_rounds,
            endLine=end_line,
        )

    @staticmethod
    def create_thinking(is_thinking: bool) -> ThinkingResponse:
        return ThinkingResponse(isThinking=is_thinking)

    @staticmethod
    def create_chapter_change(chapter_info: str) -> ChapterChangeResponse:
        return ChapterChangeResponse(chapterName=chapter_info)

    @staticmethod
    def create_choice(choices: list[str], allow_free: bool = False) -> ChoiceResponse:
        return ChoiceResponse(choices=choices, allowFree=allow_free)

    @staticmethod
    def create_input(hint: str, **kwargs) -> ScriptInputResponse:
        return ScriptInputResponse(hint=hint, isFinal=True, **kwargs)

    @staticmethod
    def create_background(
        image: str, transition: float, **kwargs
    ) -> ScriptBackgroundResponse:
        return ScriptBackgroundResponse(
            imagePath=image, transition=transition, **kwargs
        )

    @staticmethod
    def create_present_pic(
        image: str, scale: int, **kwargs
    ) -> ScriptPresentPicResponse:
        return ScriptPresentPicResponse(imagePath=image, scale=scale, **kwargs)

    @staticmethod
    def create_background_effect(
        effect: str, **kwargs
    ) -> ScriptBackgroundEffectResponse:
        return ScriptBackgroundEffectResponse(effect=effect, **kwargs)

    @staticmethod
    def create_sound(sound: str, **kwargs) -> ScriptSoundResponse:
        return ScriptSoundResponse(soundPath=sound, **kwargs)

    @staticmethod
    def create_music(music: str, **kwargs) -> ScriptMusicResponse:
        return ScriptMusicResponse(musicPath=music, **kwargs)

    @staticmethod
    def create_narration(
        text: str, display_name: Optional[str] = None, duration: float = -1
    ) -> ScriptNarrationResponse:
        return ScriptNarrationResponse(
            text=text, displayName=display_name, duration=duration
        )

    @staticmethod
    def create_player_dialogue(
        text: str,
        display_name: Optional[str] = None,
        display_subtitle: Optional[str] = None,
    ) -> ScriptPlayerResponse:
        return ScriptPlayerResponse(
            text=text, displayName=display_name, displaySubtitle=display_subtitle
        )

    @staticmethod
    def create_modify_character(**kwargs) -> ScriptModifyCharacterResponse:
        return ScriptModifyCharacterResponse(**kwargs)

    @staticmethod
    def create_script_end() -> ScriptEndResponse:
        return ScriptEndResponse()
