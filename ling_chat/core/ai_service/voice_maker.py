import asyncio
import os
from pathlib import Path
from typing import Awaitable, Dict, List, Optional, Tuple

from ling_chat.core.logger import logger
from ling_chat.core.TTS.tts_provider import TTS
from ling_chat.schemas.character_settings import VoiceModel


class VoiceMaker:
    def __init__(self) -> None:
        self.tts_provider = TTS()
        self.model_name = ""
        self.speaker_id = 4
        self.tts_type = ""
        self.lang = "ja"  # 默认语言为日语
        self.character_path = ""  # 添加角色卡路径，以便用于 gsv

        # 初始化语音合成器可用状态
        self.sva_available = False
        self.sbv2_available = False
        self.bv2_available = False
        self.sbv2api_available = False
        self.gsv_available = False
        self.aivis_available = False
        self.openai_tts_available = False

    def check_tts_availability(self, tts_settings: VoiceModel) -> None:
        """检查 TTS 配置可用性，设置各语音合成器状态"""

        def _is_valid(value: str | None) -> bool:
            """检查字符串是否有效（非空且非空格）"""
            return value is not None and value.strip() != ""

        # 检查 SVA 配置
        sva_speaker_id = tts_settings.sva_speaker_id
        self.sva_available = _is_valid(sva_speaker_id)

        # 检查 SBV2 配置
        sbv2_speaker_id = tts_settings.sbv2_speaker_id
        sbv2_name = tts_settings.sbv2_name
        self.sbv2_available = _is_valid(sbv2_speaker_id) and _is_valid(sbv2_name)

        # 检查 BV2 配置
        bv2_speaker_id = tts_settings.bv2_speaker_id
        self.bv2_available = _is_valid(bv2_speaker_id)

        # 检查 SBV2API 配置
        sbv2api_name = tts_settings.sbv2api_name
        sbv2api_speaker_id = tts_settings.sbv2api_speaker_id
        self.sbv2api_available = _is_valid(sbv2api_name) and _is_valid(
            sbv2api_speaker_id
        )

        # 检查 GSV 配置
        gsv_voice_filename = tts_settings.gsv_voice_filename
        gsv_voice_text = tts_settings.gsv_voice_text
        gsv_gpt_model_name = tts_settings.gsv_gpt_model_name
        gsv_sovits_model_name = tts_settings.gsv_sovits_model_name
        self.gsv_available = (
            _is_valid(gsv_voice_filename) and _is_valid(gsv_voice_text)
        ) or (_is_valid(gsv_gpt_model_name) and _is_valid(gsv_sovits_model_name))

        # 检查 AIVIS 配置
        aivis_model_uuid = tts_settings.aivis_model_uuid
        self.aivis_available = _is_valid(aivis_model_uuid)

        # 检查 OpenAI TTS 配置
        openai_tts_model = tts_settings.openai_tts_model
        openai_tts_voice = tts_settings.openai_tts_voice
        self.openai_tts_available = _is_valid(openai_tts_model) and _is_valid(
            openai_tts_voice
        )

    def set_tts_settings(self, tts_settings: VoiceModel, name: str, tts_language: str = "ja") -> None:
        """获取可用的 TTS 配置并且进行基础配置"""
        try:
            # 先检查所有 TTS 配置的可用性
            logger.debug("开始验证 TTS 配置可用性")
            self.check_tts_availability(tts_settings)

            # 根据当前设置的 TTS 类型进行初始化
            if self.tts_type == "sva-vits" and self.sva_available:
                self.tts_provider.init_sva_adapter(
                    speaker_id=int(tts_settings.sva_speaker_id)
                )
            elif self.tts_type == "sbv2" and self.sbv2_available:
                self.tts_provider.init_sbv2_adapter(
                    speaker_id=int(tts_settings.sbv2_speaker_id),
                    model_name=tts_settings.sbv2_name,
                )
            elif self.tts_type == "sva-bv2" and self.bv2_available:
                self.tts_provider.init_bv2_adapter(
                    speaker_id=int(tts_settings.bv2_speaker_id)
                )
            elif self.tts_type == "sbv2api" and self.sbv2api_available:
                self.tts_provider.init_sbv2api_adapter(
                    model_name=tts_settings.sbv2api_name,
                    speaker_id=int(tts_settings.sbv2api_speaker_id),
                )
            elif self.tts_type == "gsv" and self.gsv_available:
                # 获取参考音频文件名
                ref_audio_filename = tts_settings.gsv_voice_filename
                ref_audio_path = ref_audio_filename

                # 检查参考音频路径是否为绝对路径，如果是则发出警告
                if os.path.isabs(ref_audio_filename):
                    logger.warning(
                        f"角色 {name} 的参考音频路径为绝对路径：{ref_audio_filename}，这可能导致 gsv 出错"
                    )

                # 拼接角色路径
                ref_audio_path = os.path.join(self.character_path, ref_audio_filename)
                logger.debug(f"gsv 拼接后的参考音频路径：{ref_audio_path}")

                # 优先使用环境变量定义的语音文件
                if os.environ.get("GPT_SOVITS_REF_AUDIO", "") == "":
                    self.tts_provider.init_gsv_adapter(
                        ref_audio_path=ref_audio_path,
                        prompt_text=tts_settings.gsv_voice_text,
                    )
                else:
                    self.tts_provider.init_gsv_adapter(
                        ref_audio_path=os.environ.get("GPT_SOVITS_REF_AUDIO", ""),
                        prompt_text=os.environ.get("GPT_SOVITS_PROMPT_TEXT", ""),
                    )
                    logger.warning("你正在使用环境变量中的 GPT-SoVITS 配置")

                # 处理模型设置
                gpt_model_name = tts_settings.gsv_gpt_model_name
                sovits_model_name = tts_settings.gsv_sovits_model_name

                # 检查环境变量中的模型配置
                env_gpt_model = os.environ.get("GPT_SOVITS_GPT_MODEL", "")
                env_sovits_model = os.environ.get("GPT_SOVITS_SOVITS_MODEL", "")

                # 异步设置模型
                async def _set_models(gpt_model_path: str, sovits_model_path: str):
                    # 确保 gsv_adapter 不为 None (pylance 如是说)
                    if self.tts_provider.gsv_adapter is not None:
                        success = await self.tts_provider.gsv_adapter.set_model(
                            gpt_model_path, sovits_model_path
                        )
                        if success:
                            logger.info(
                                f"GSV 模型设置成功：GPT={gpt_model_path}, SoVITS={sovits_model_path}"
                            )
                        else:
                            logger.error(
                                f"GSV 模型设置失败：GPT={gpt_model_path}, SoVITS={sovits_model_path}"
                            )
                    else:
                        logger.error("GSV 适配器未初始化，无法设置模型")

                # 如果环境变量中有模型配置，则优先使用环境变量，否则使用 setting 设置
                if env_gpt_model and env_sovits_model:
                    # 创建异步任务
                    asyncio.create_task(_set_models(env_gpt_model, env_sovits_model))
                    logger.warning("你正在使用环境变量中的 GSV 模型配置")
                elif gpt_model_name and sovits_model_name:
                    # 构建模型的绝对路径
                    models_dir = os.path.join(self.character_path, "models", "gsv")
                    gpt_model_path = os.path.join(models_dir, gpt_model_name)
                    sovits_model_path = os.path.join(models_dir, sovits_model_name)

                    # 创建异步任务
                    asyncio.create_task(_set_models(gpt_model_path, sovits_model_path))
            elif self.tts_type == "aivis" and self.aivis_available:
                self.tts_provider.init_aivis_adapter(
                    model_uuid=tts_settings.aivis_model_uuid
                )
            elif self.tts_type == "indextts2":
                self.tts_provider.init_index_adapter()
            elif self.tts_type == "openai-tts" and self.openai_tts_available:
                self.tts_provider.init_openai_tts_adapter(
                    model=tts_settings.openai_tts_model,
                    voice=tts_settings.openai_tts_voice,
                )
            else:
                logger.warning(
                    f"你的环境变量中 TTS 设置有误，此角色{name}不支持{self.tts_type}，将使用角色卡的默认语音合成器！"
                )
                raise ValueError

            # 统一设置 TTS 语言（ja=日语，zh=中文）
            if tts_language in ("ja", "zh"):
                self.set_lang(tts_language)
                logger.info(f"角色{name}的 TTS 语言已设置为：{tts_language}")
            elif self.tts_type == "indextts2":
                self.set_lang("zh")
                logger.info(f"角色{name}的 TTS 语言已设置为：zh (indextts2 默认)")
        except KeyError as e:
            logger.error(f"当前角色卡{name}的 TTS 设置出错，问题是：{e}")

    def set_tts(self, tts_type: str, tts_settings: VoiceModel, name: str, tts_language: str = "ja") -> None:
        """设置默认的 TTS 类型"""
        available_tts_types = (
            "sva-bv2",
            "gsv",
            "sbv2",
            "sva-vits",
            "sbv2api",
            "aivis",
            "indextts2",
            "openai-tts",
        )
        try:
            if tts_type in available_tts_types:
                self.tts_type = tts_type
                self.set_tts_settings(tts_settings, name, tts_language)
        except ValueError:
            if tts_type in available_tts_types:
                self.tts_type = tts_type
                self.set_tts_settings(tts_settings, name, tts_language)
            else:
                logger.error(
                    f"角色卡中有未知的 TTS 类型：{tts_type}，请联系角色卡制造者。"
                )

    def set_lang(self, lang: str) -> None:
        """设置语言"""
        if lang not in ["ja", "zh"]:
            raise ValueError(f"不支持的语言：{lang}")
        self.lang = lang

    def set_character_path(self, character_path: str) -> None:
        """设置角色卡路径"""
        self.character_path = character_path

    async def generate_voice_files(self, segments: List[Dict[str, str]]):
        """生成语音文件"""
        tasks: List[Awaitable[str | None]] = []
        logger.debug(f"生成语音文件：{segments}")
        for seg in segments:
            if self.lang == "ja":
                if seg["japanese_text"]:
                    task = self.tts_provider.generate_voice(
                        seg["japanese_text"],
                        seg["voice_file"],
                        tts_type=self.tts_type,
                        lang="ja",
                    )
                    tasks.append(task)
                elif seg["following_text"] and not seg.get("japanese_text"):
                    logger.warning(f"片段 {seg['index']} 没有日语文本，跳过语音生成")
            elif self.lang == "zh":
                if seg["following_text"]:
                    task = self.tts_provider.generate_voice(
                        seg["following_text"],
                        seg["voice_file"],
                        tts_type=self.tts_type,
                        emo=seg.get("predict", ""),
                        lang="zh",
                    )
                    tasks.append(task)
                else:
                    logger.warning(
                        f"片段 {seg['index']} 没有中文文本，跳过语音生成\n"
                        f"Tips：要真出现这情况，你应该检查 LLM 是否正常输出。"
                    )
        if tasks:
            await asyncio.gather(*tasks)

    async def regenerate_missing_audio(
        self,
        segments: List[Dict[str, str]],
    ) -> Tuple[List[Dict[str, str]], int]:
        """检查并重新合成缺失的语音文件

        Args:
            segments: 包含语音文件路径和文本的片段列表

        Returns:
            (处理后的片段列表，重新合成的数量)
        """
        regenerated_count = 0
        result_segments = []

        for seg in segments:
            audio_file = seg.get("audio_file") or seg.get("voice_file")

            if not audio_file:
                # 没有音频文件引用，直接添加到结果
                result_segments.append(seg)
                continue

            # 检查音频文件是否存在
            audio_path = Path(audio_file)
            if audio_path.exists():
                # 文件存在，无需处理
                result_segments.append(seg)
                continue

            # 文件不存在，需要重新合成
            logger.info(f"语音文件不存在，准备重新合成：{audio_file}")

            # 获取用于合成的文本
            if self.lang == "ja":
                text_to_synthesize = seg.get("japanese_text") or seg.get("tts_content")
            elif self.lang == "zh":
                text_to_synthesize = seg.get("following_text") or seg.get("tts_content")
            else:
                text_to_synthesize = seg.get("japanese_text") or seg.get("following_text") or seg.get("tts_content")

            if not text_to_synthesize or not text_to_synthesize.strip():
                logger.warning(f"无法找到合成文本，跳过：{audio_file}")
                result_segments.append(seg)
                continue

            try:
                # 重新合成语音
                output_path = seg.get("voice_file") or audio_file
                emo = seg.get("predict", "") if self.lang == "zh" else ""

                success = await self.tts_provider.generate_voice(
                    text_to_synthesize,
                    output_path,
                    tts_type=self.tts_type,
                    lang=self.lang,
                    emo=emo,
                )

                if success:
                    regenerated_count += 1
                    logger.info(f"语音重新合成成功：{output_path}")
                    # 更新片段中的音频文件路径
                    seg["audio_file"] = output_path
                    seg["voice_file"] = output_path
                else:
                    logger.error(f"语音重新合成失败：{output_path}")

            except Exception as e:
                logger.error(f"语音重新合成出错 {audio_file}: {e}")

            result_segments.append(seg)

        return result_segments, regenerated_count
