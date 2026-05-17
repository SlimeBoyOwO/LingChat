import os
import re
import uuid
from datetime import datetime, timedelta
from typing import Dict, List

from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.emotion.classifier import emotion_classifier
from ling_chat.core.logger import logger
from ling_chat.core.pic_analyzer import DesktopAnalyzer
from ling_chat.utils.function import Function


class MessageProcessor:
    def __init__(self, game_status: GameStatus) -> None:
        # 记录消息发送间隔和次数提示
        self.last_time = datetime.now()
        self.sys_time_counter = 0

        # 用于分析图像信息
        self.desktop_analyzer = DesktopAnalyzer()
        self.time_sense_enabled = os.environ.get("USE_TIME_SENSE", True)

        # 用于存储语音目录位置，其实在voice_maker已经有了
        self.game_status = game_status

    def parse_and_classify_emotional_segments(self, text: str) -> List[Dict]:
        """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
        emotion_segments = re.findall(r"(【(.*?)】)([^【】]*)", text)

        if not emotion_segments:
            logger.warning("未在文本中找到【】格式的情绪标签，将尝试添加默认标签")
            return []

        results = []
        for i, (full_tag, emotion_tag, following_text) in enumerate(
            emotion_segments, 1
        ):
            following_text = following_text.replace("(", "（").replace(")", "）")

            japanese_match = re.search(r"<(.*?)>", following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            motion_match = re.search(r"（(.*?)）", following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            cleaned_text = re.sub(r"<.*?>|（.*?）", "", following_text).strip()

            enable_translate = (
                os.environ.get("ENABLE_TRANSLATE", "False").lower() == "true"
            )
            if not enable_translate:
                # 直接使用 cleaned_text 作为日语文本
                japanese_text = cleaned_text

            # 清洗日语文本，保证没有额外或者错误的输出
            if japanese_text:
                japanese_text = re.sub(r"（.*?）", "", japanese_text).strip()
                japanese_text = re.sub(r"【.*?】", "", japanese_text).strip()
                japanese_text = japanese_text.replace("~", "。")

            if not cleaned_text and not japanese_text and not motion_text:
                continue

            try:
                if japanese_text and cleaned_text:
                    lang_jp = Function.detect_language(japanese_text)
                    lang_clean = Function.detect_language(cleaned_text)

                    if (
                        lang_jp in ["Chinese", "Chinese_ABS"]
                        and lang_clean in ["Japanese", "Chinese"]
                    ) and lang_clean != "Chinese_ABS":
                        cleaned_text, japanese_text = japanese_text, cleaned_text

            except Exception as e:
                logger.warning(f"语言检测错误: {e}")

            try:
                predicted = emotion_classifier.predict(emotion_tag)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"],
                }
            except Exception as e:
                logger.error(f"情绪预测错误 '{emotion_tag}': {e}")
                prediction_result = {"label": "normal", "confidence": 0.5}

            file_str = ""
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            if self.game_status.current_character:
                voice_maker = self.game_status.current_character.voice_maker
                file_str = str(
                    voice_maker.tts_provider.temp_dir
                    / f"{uuid.uuid4()}_part_{i}.{voice_maker.tts_provider.format}"
                )

            results.append(
                {
                    "index": i,
                    "original_tag": emotion_tag,
                    "following_text": cleaned_text,
                    "motion_text": motion_text,
                    "japanese_text": japanese_text,
                    "predicted": prediction_result["label"],
                    "confidence": prediction_result["confidence"],
                    "voice_file": file_str,
                }
            )

        return results

    async def append_user_message(self, user_message: str) -> dict:
        """处理用户消息，添加系统信息，如时间、是否需要分析桌面，以及提取大括号内的用户指令"""

        # TODO: 当 AI 的回复句子总是固定的时候，增加提示让 AI 的回复句子适度调整

        current_time = datetime.now()
        processed_message = user_message

        sys_time_part = ""
        sys_desktop_part = ""
        user_instruction_part = ""
        temp_instruction_part = ""

        # 提取大括号内的用户指令
        import re

        bracket_pattern = r"\{([^}]+)\}"
        bracket_matches = re.findall(bracket_pattern, user_message)

        # --- 1. 处理大括号 {} ---
        if bracket_matches:
            processed_message = re.sub(bracket_pattern, "", processed_message).strip()
            user_instruction_part = "旁白: " + "; ".join(bracket_matches)

        # --- 2. 处理 [!Temp!] ---
        temp_pattern = r"\[!Temp!\](.*?)\[/!Temp!\]"

        # re.S (re.DOTALL) 让 . 可以匹配换行符
        temp_matches = re.findall(temp_pattern, user_message, flags=re.S)

        if temp_matches:
            processed_message = re.sub(
                temp_pattern, "", processed_message, flags=re.S
            ).strip()
            temp_instruction_part = "%".join([f"${match}$" for match in temp_matches])

        # 时间感知逻辑
        if self.time_sense_enabled and (
            (self.last_time and (current_time - self.last_time > timedelta(hours=1)))
            or self.sys_time_counter < 1
        ):
            formatted_time = current_time.strftime("%Y/%m/%d %H:%M")
            sys_time_part = f"{formatted_time} "

        # 桌面分析逻辑
        desktop_keywords = [
            "看桌面",
            "看看我的桌面",
            "看看桌面",
            "看我桌面",
            "看看我桌面",
            "看我的桌面",
            "看下我桌面",
            "看下桌面",
            "看下我的桌面",
        ]

        if any(keyword in user_message for keyword in desktop_keywords):
            analyze_prompt = (
                "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。"
                + '"'
                + user_message
                + '"'
                + "以上是用户发的消息，请切合用户实际获取信息的需要，获取桌面画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。但如果用户信息中明确提到了AI的立绘，背景等（比如用户消息说“看看你的周围，这是哪里呀？”）的时候，你可以描述AI的立绘或背景来告诉主AI的环境感知能力。"
            )
            analyze_info = await self.desktop_analyzer.analyze_desktop(analyze_prompt)
            sys_desktop_part = f"桌面信息: {analyze_info}"

        # 构建系统提醒部分
        system_parts = []
        sys_flag = False
        if sys_time_part:
            system_parts.append(sys_time_part)
            sys_flag = True
        if sys_desktop_part:
            system_parts.append(sys_desktop_part)
            sys_flag = True
        if user_instruction_part:
            system_parts.append(user_instruction_part)
        if temp_instruction_part:
            system_parts.append(temp_instruction_part)

        if system_parts:
            processed_message += (
                "\n{"
                + ("系统提醒: " if sys_flag else "")
                + " ".join(system_parts)
                + "}"
            )

        self.last_time = current_time
        self.sys_time_counter += 1

        if self.sys_time_counter >= 2:
            self.sys_time_counter = 0

        logger.info("处理后的用户信息是:" + processed_message)
        return {
            "main": processed_message,
            "temp": temp_instruction_part if temp_instruction_part else None,
        }

    def sys_prompt_builder(
        self,
        user_name: str,
        character_name: str,
        ai_prompt: str,
        ai_prompt_example: str,
        ai_prompt_example_old: str,
    ) -> str:
        """
        构建系统提示词，根据是否启用翻译功能来决定使用哪种对话格式

        该函数会根据环境变量 ENABLE_TRANSLATE 的值来决定是否添加日语翻译功能。
        如果启用翻译，则使用简单的中文对话格式；否则使用中日双语对照格式。
        同时会检查传入的示例是否为空，如果为空则使用默认示例进行替换。

        Args:
            user_name (str): 用户名称
            character_name (str): AI角色名称
            ai_prompt (str): 基础的AI提示词内容
            ai_prompt_example (str): 用于实时翻译模式的对话示例
            ai_prompt_example_old (str): 用于非翻译模式的对话示例（包含中日对照翻译）

        Returns:
            str: 构建完成的系统提示词，包含了对话格式要求和示例
        """

        # 从配置文件加载提示词模板
        from ling_chat.core.ai_service.prompt_config import prompt_config

        # 从配置文件读取情绪限制设置
        emotion_limit_enabled = (
            os.environ.get("NO_EMOTION_LIMIT_PROMPT", "False").lower() != "true"
        )
        dialog_format_prompt_2 = prompt_config.get_emotion_limit_prompt(
            emotion_limit_enabled
        )
        if not emotion_limit_enabled:
            logger.warning("提示词将不再限制模型输出的情绪")

        # 从配置文件读取对话格式和示例
        dialog_format_prompt_cn = prompt_config.dialog_format_cn
        dialog_format_prompt_jp = prompt_config.dialog_format_jp
        default_example_cn = prompt_config.get_default_example(True)
        default_example_jp = prompt_config.get_default_example(False)
        format_prompt_template = prompt_config.format_prompt_template

        # 根据环境变量决定使用哪种语言模式
        use_cn_mode = (
            os.environ.get("LLM_OUTPUT_SEC_LANG", "False").lower() == "true"
        )

        if use_cn_mode:
            # 中文模式（无翻译）
            if ai_prompt_example == ("", None):
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                ai_prompt_example = default_example_cn

            if "日语翻译" in ai_prompt:
                logger.warning("你使用的人物为旧版，不能使用实时翻译功能")
                ai_prompt = ai_prompt
            elif "以下是我的对话格式提示" in ai_prompt:
                logger.warning("你使用的人物为旧版，不进行拼接prompt")
                ai_prompt = ai_prompt
            else:
                ai_prompt = (
                    ai_prompt
                    + format_prompt_template.format(
                        character_name=character_name, user_name=user_name
                    )
                    + dialog_format_prompt_cn
                    + ai_prompt_example
                    + dialog_format_prompt_2
                )
        else:
            # 日语翻译模式
            if ai_prompt_example == ("", None):
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                ai_prompt_example_old = default_example_jp

            if "以下是我的对话格式提示" in ai_prompt:
                logger.warning("你使用的人物为旧版，可能实时翻译功能不起作用")
                ai_prompt = ai_prompt
            else:
                ai_prompt = (
                    ai_prompt
                    + format_prompt_template.format(
                        character_name=character_name, user_name=user_name
                    )
                    + dialog_format_prompt_jp
                    + ai_prompt_example_old
                    + "\n"
                    + dialog_format_prompt_2
                )

        return ai_prompt
