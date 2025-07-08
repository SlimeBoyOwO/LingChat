import os
import glob
import asyncio
import re
import json, copy
from typing import List, Dict, Optional
from datetime import datetime, timedelta

from .llm_providers.manager import LLMManager
from .predictor import EmotionClassifier  # 导入情绪分类器
from .VitsTTS.vits_tts import VitsTTS
from .logger import logger, TermColors
from .dialog_logger import DialogLogger
from .pic_analyzer import DesktopAnalyzer

from utils.function import Function

TEMP_VOICE_DIR = "../public/audio"
WS_HOST = "localhost"
WS_PORT = 8765

COLOR = {
    "user": "\033[92m",
    "ai": "\033[96m",
    "emotion": "\033[93m",
    "reset": "\033[0m"
}

class AIService:
    def __init__(self, settings: dict):
        """初始化所有服务组件"""
        self.llm_model = LLMManager()
        self.emotion_classifier = EmotionClassifier()
        self.dialog_logger = DialogLogger()
        self.desktop_analyzer = DesktopAnalyzer()
        self._prepare_directories()

        # 这里记录上次对话的时间
        self.last_time = datetime.now()
        self.sys_time_counter = 0
        self.time_sense_enabled = os.environ.get("USE_TIME_SENSE",True)

        self.tts_engine = self._init_tts_engine()

        # RAG 系统
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_systems_cache = {}  # 缓存RAG实例 {character_id: rag_system_instance}
        self.rag_config = None       # 存储RAG配置
        self.session_file_path = None
        self.active_rag_system = None # 当前激活的RAG实例
        self.rag_window = int(os.environ.get("RAG_WINDOW_COUNT", 5)) # 短期记忆窗口大小
        if self.use_rag: 
            logger.info(f"当前RAG窗口大小是：{self.rag_window}")

        self.import_settings(settings=settings)
        
        self.messages = [
            {
                "role": "system", 
                "content": self.ai_prompt
            }
        ]

        self.temp_voice_dir = os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio")
        os.makedirs(self.temp_voice_dir, exist_ok=True)
        
        self._init_rag_config()
        
    def _init_rag_config(self):
        """初始化RAG相关配置并加载RAG系统"""
        class Config:
            def __init__(self, **kwargs):
                for key, value in kwargs.items():
                    setattr(self, key, value)
                    
        use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        ai_name = os.environ.get("AI_NAME", "钦灵")
        log_level_str = os.environ.get("LOG_LEVEL", "INFO").upper()
        print_context = os.environ.get("PRINT_CONTEXT", "False").lower() == "true"
        rag_history_path = os.environ.get("RAG_HISTORY_PATH", "rag_chat_history")
        chroma_db_path = os.environ.get("CHROMA_DB_PATH", "chroma_db_store")
        rag_retrieval_count = int(os.environ.get("RAG_RETRIEVAL_COUNT", "3"))
        rag_candidate_multiplier = int(os.environ.get("RAG_CANDIDATE_MULTIPLIER", "3"))
        rag_context_m_before = int(os.environ.get("RAG_CONTEXT_M_BEFORE", "2"))
        rag_context_n_after = int(os.environ.get("RAG_CONTEXT_N_AFTER", "2"))
        rag_prompt_prefix = os.environ.get("RAG_PROMPT_PREFIX", 
                                        "以下是根据你的问题从历史对话中检索到的相关片段，其中包含了对话发生的大致时间：")
        rag_prompt_suffix = os.environ.get("RAG_PROMPT_SUFFIX", "")
        
        rag_config = Config(
            USE_RAG=use_rag,
            AI_NAME=ai_name,
            LOG_LEVEL=log_level_str,
            PRINT_CONTEXT=print_context,
            RAG_HISTORY_PATH=rag_history_path,
            CHROMA_DB_PATH=chroma_db_path,
            RAG_RETRIEVAL_COUNT=rag_retrieval_count,
            RAG_CANDIDATE_MULTIPLIER=rag_candidate_multiplier,
            RAG_CONTEXT_M_BEFORE=rag_context_m_before,
            RAG_CONTEXT_N_AFTER=rag_context_n_after,
            RAG_PROMPT_PREFIX=rag_prompt_prefix,
            RAG_PROMPT_SUFFIX=rag_prompt_suffix
        )
        
        if use_rag:
            logger.info("正在初始化RAG系统...")
            rag_initialized = self.init_rag_system(rag_config, self.character_id)
            if rag_initialized:
                logger.info("RAG系统初始化成功")
            else:
                logger.warning("RAG系统初始化失败或禁用")
        else:
            logger.info("RAG系统已禁用")

    def init_rag_system(self, config, initial_character_id: int):
        """初始化RAG系统（如果启用）"""
        if not self.use_rag:
            logger.debug("RAG系统未启用，跳过初始化")
            return False
        
        self.rag_config = config # 存储配置以备后用
        return self.switch_rag_system_character(initial_character_id)

    def switch_rag_system_character(self, character_id: int) -> bool:
        """切换或初始化指定角色的RAG系统"""
        if not self.use_rag:
            return False

        # 如果已缓存，直接切换
        if character_id in self.rag_systems_cache:
            self.active_rag_system = self.rag_systems_cache[character_id]
            logger.info(f"RAG记忆库已切换至已缓存的角色 (ID: {character_id})")
            return True

        # 如果未缓存，则创建新的实例
        try:
            from .RAG import RAGSystem
            logger.info(f"正在为新角色 (ID: {character_id}) 初始化RAG记忆库...")
            
            # 记录RAG初始化的详细配置
            if logger.should_print_context():
                logger.debug("\n------ RAG初始化配置详情 ------")
                config_attrs = [attr for attr in dir(self.rag_config) if not attr.startswith('_') and not callable(getattr(self.rag_config, attr))]
                for attr in sorted(config_attrs):
                    value = getattr(self.rag_config, attr)
                    logger.debug(f"RAG配置: {attr} = {value}")
                logger.debug("------ RAG配置结束 ------\n")
            
            new_rag_system = RAGSystem(self.rag_config, character_id) # 传入character_id
            
            if new_rag_system.initialize():
                self.rag_systems_cache[character_id] = new_rag_system
                self.active_rag_system = new_rag_system
                logger.info(f"角色 (ID: {character_id}) 的RAG记忆库初始化成功并已缓存。")
                
                if logger.should_print_context():
                    # 记录初始化后的状态信息
                    history_count = 0
                    chroma_count = 0
                    if hasattr(new_rag_system, 'flat_historical_messages'):
                        history_count = len(new_rag_system.flat_historical_messages)
                    if new_rag_system.chroma_collection:
                        chroma_count = new_rag_system.chroma_collection.count()
                    
                    logger.debug(f"RAG初始化状态: 历史消息数={history_count}, ChromaDB条目数={chroma_count}")
                
                return True
            else:
                logger.error(f"为角色 (ID: {character_id}) 初始化RAG记忆库失败。")
                return False
        except ImportError as e:
            logger.error(f"RAG模块: {e}")
            return False
        except Exception as e:
            logger.error(f"切换RAG角色 (ID: {character_id}) 时出错: {e}", exc_info=True)
            return False
            
    def _init_tts_engine(self) -> VitsTTS:
        """初始化TTS引擎"""
        return VitsTTS()
    
    def _prepare_directories(self):
        """准备必要的目录"""
        os.makedirs(TEMP_VOICE_DIR, exist_ok=True)

    def _append_user_message(self, user_message: str) -> str:
        """处理用户消息，添加系统信息，如时间与是否需要分析桌面"""
        current_time = datetime.now()
        processed_message = user_message

        sys_time_part = ""
        sys_desktop_part = ""
        
        if self.time_sense_enabled and ((self.last_time and 
            (current_time - self.last_time > timedelta(hours=1))) or \
            self.sys_time_counter < 1):
            
            formatted_time = current_time.strftime("%Y/%m/%d %H:%M")
            sys_time_part = f"{formatted_time} "
        
        if "看桌面" in user_message or "看看我的桌面" in user_message:
            analyze_prompt = "\"" + user_message + "\"" + "以上是用户发的消息，请切合用户实际获取信息的需要，获取桌面画面中的重点内容，用200字描述主体部分即可。"
            analyze_info = self.desktop_analyzer.analyze_desktop(analyze_prompt)
            sys_desktop_part = f"桌面信息: {analyze_info}"
        
        if sys_time_part or sys_desktop_part:
            processed_message += "\n{系统: " + (sys_time_part if sys_time_part else "") + (sys_desktop_part if sys_desktop_part else "") + "}"

        self.last_time = current_time
        self.sys_time_counter += 1

        if self.sys_time_counter >= 2:
                self.sys_time_counter = 0
        
        return processed_message
    
    async def process_message(self, user_message: str) -> Optional[List[Dict]]:
        """处理用户消息的完整流程"""

        processed_user_message = self._append_user_message(user_message)

        try:
            # 1. 获取AI回复
            self.messages.append({"role": "user", "content": processed_user_message})
            rag_messages = []

            current_context = self.messages.copy()
            # 如果启用了RAG系统，保存本次会话到RAG历史记录
            self._rag_append_sys_message(current_context, rag_messages, processed_user_message)
            # 若打印上下文选项开启且在DEBUG级别，则截取发送到llm的文字信息打印到终端
            # self._print_debug_message(current_context, rag_messages, processed_user_message)

            ai_response = self.llm_model.process_message(current_context)

            # 1.5 修复ai回复中可能出错的部分，防止下一次对话被带歪
            ai_response = Function.fix_ai_generated_text(ai_response)
            self.messages.append({"role": "assistant", "content": ai_response})

            # 如果有RAG系统，则把这段对话保存在RAG中 TODO 只获取最后两条最新的
            self._save_messages_to_rag(self.messages)

            self._log_conversation("用户", processed_user_message)
            self._log_conversation("钦灵", ai_response)
            
            # 2. 分析情绪和生成语音
            final_response = await self._process_ai_response(ai_response, user_message)
            if final_response is None:
                logger.error("AI服务返回了None响应")
                error_response = [{
                                "type": "reply",
                                "emotion": "sad",
                                "originalTag": "错误",
                                "message": "抱歉，处理您的消息时出现了问题。",
                                "motionText": "困惑",
                                "audioFile": None,
                                "originalMessage": user_message,
                                "isMultiPart": False,
                                "partIndex": 0,
                                "totalParts": 1
                            }]
                return error_response
            else: return final_response
                
        except Exception as e:
            logger.error(f"处理消息时出错: {e}")
            logger.error(f"详细错误信息: ", exc_info=True)
            
            error_response = [{
                "type": "reply",
                "emotion": "sad",
                "originalTag": "错误",
                "message": f"抱歉，处理消息时出现错误: {str(e)}",
                "motionText": "困惑",
                "audioFile": None,
                "originalMessage": user_message,
                "isMultiPart": False,
                "partIndex": 0,
                "totalParts": 1
            }]
            return error_response
    
    def load_memory(self, memory):
        original_messages_count = len(self.messages)
        
        if isinstance(memory, str):
            memory = json.loads(memory)
        self.messages = copy.deepcopy(memory)
        
        logger.info("记忆存档已经加载")
        logger.info(f"内容是：{memory}")
        logger.info(f"新的messages是：{self.messages}")
        
        if logger.should_print_context():
            new_messages_count = len(self.messages)
            
            role_counts = {}
            for msg in self.messages:
                role = msg.get('role', 'unknown')
                role_counts[role] = role_counts.get(role, 0) + 1
                
            role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
            
            logger.debug("\n------ 记忆加载详情 ------")
            logger.debug(f"原始消息数: {original_messages_count}, 加载后消息数: {new_messages_count}")
            logger.debug(f"消息角色分布: {role_stats}")
            logger.debug(f"------ 记忆加载结束 ------\n")
        logger.info("新的记忆已经被加载")

    def import_settings(self, settings: dict):
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            self.model_name = settings.get("model_name", None)
            self.speaker_id = int(settings.get("speaker_id", 4))
            self.character_path = settings.get("resource_path")
            self.character_id = settings.get("character_id")
            self.settings = settings

            if self.use_rag:
                logger.info(f"检测到角色切换，正在为角色 (ID: {self.character_id}) 准备长期记忆...")
                self.switch_rag_system_character(self.character_id)
        else:
            logger.error("角色信息settings没有被正常导入，请检查问题！")

    async def _process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self._clean_temp_voice_files()
        
        emotion_segments = self._analyze_emotions(ai_response)
        if not emotion_segments:
            logger.warning("未检测到有效情绪片段")
            emotion_segments = [{
                "index": 1,
                "original_tag": "normal",
                "following_text": ai_response,
                "motion_text": "",
                "japanese_text": "",
                "predicted": "normal",
                "confidence": 0.8,
                "voice_file": os.path.join(self.temp_voice_dir, f"part_1.{self.tts_engine.format}")
            }]
        
        self._delete_voice_files()
        await self._generate_voice_files(emotion_segments)
        responses = self._create_responses(emotion_segments, user_message)
    
        logger.debug("--- AI 回复分析结果 ---")
        self._log_analysis_result(emotion_segments)
        logger.debug("--- 分析结束 ---")

        return responses
    
    def _analyze_emotions(self, text: str) -> List[Dict]:
        """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
        emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)
        
        if not emotion_segments:
            logger.warning(f"未在文本中找到【】格式的情绪标签，将尝试添加默认标签")
            return []

        results = []
        for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
            following_text = following_text.replace('(', '（').replace(')', '）')

            japanese_match = re.search(r'<(.*?)>', following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            motion_match = re.search(r'（(.*?)）', following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

            if japanese_text:
                japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

            if not cleaned_text and not japanese_text and not motion_text:
                continue

            try:
                if japanese_text and cleaned_text:
                    lang_jp = Function.detect_language(japanese_text)
                    lang_clean = Function.detect_language(cleaned_text)

                    if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                        lang_clean != 'Chinese_ABS':
                            cleaned_text, japanese_text = japanese_text, cleaned_text

            except Exception as e:
                logger.warning(f"语言检测错误: {e}")

            try:
                predicted = self.emotion_classifier.predict(emotion_tag)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"]
                }
            except Exception as e:
                logger.error(f"情绪预测错误 '{emotion_tag}': {e}")
                prediction_result = {
                    "label": "normal",
                    "confidence": 0.5
                }
            
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

            results.append({
                "index": i,
                "original_tag": emotion_tag,
                "following_text": cleaned_text,
                "motion_text": motion_text,
                "japanese_text": japanese_text,
                "predicted": prediction_result["label"],
                "confidence": prediction_result["confidence"],
                "voice_file": os.path.join(self.temp_voice_dir, f"{timestamp}_part_{i}.{self.tts_engine.format}")
            })

        return results
    
    async def _generate_voice_files(self, segments: List[Dict]):
        """生成语音文件。只有在有日语文本时才生成语音"""
        tasks = []
        for seg in segments:
            if seg["japanese_text"]:
                output_file = self.tts_engine.generate_voice(seg["japanese_text"], seg["voice_file"], speaker_id=self.speaker_id, model_name=self.model_name)
                if output_file is not None:
                    tasks.append(output_file)
                else:
                    seg["voice_file"] = "none"
                
            elif seg["following_text"] and not seg.get("japanese_text"):
                logger.warning(f"片段 {seg['index']} 没有日语文本，跳过语音生成")
                
        if tasks:
            await asyncio.gather(*tasks)
        # else:
            # logger.warning("没有任何片段包含日语文本，跳过所有语音生成")
    
    def _delete_voice_files(self):
        self.tts_engine.cleanup()
    
    def _create_responses(self, segments: List[Dict], user_message: str) -> List[Dict]:
        """构造响应消息"""
        total_parts = len(segments)
        return [{
            "type": "reply",
            "emotion": seg['predicted'] or seg["original_tag"],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,
            "partIndex": idx,
            "totalParts": total_parts
        } for idx, seg in enumerate(segments)]
    
    def _clean_temp_voice_files(self):
        """清理临时语音文件"""
        for file in glob.glob(os.path.join(TEMP_VOICE_DIR, "*.wav")):
            try:
                os.remove(file)
            except OSError as e:
                logger.warning(f"删除文件 {file} 失败: {e}")
    
    def _log_conversation(self, speaker: str, message: str):
        """记录对话日志"""
        log_message = f"{speaker}: {message}"
        logger.info_color(log_message, TermColors.WHITE)
        self.dialog_logger.log_conversation(speaker,message)

    def _log_analysis_result(self, segments):
        """记录分析结果"""
        for segment in segments:
            logger.debug(f"\n分析结果 (片段 {segment['index']}):")
            logger.debug(f"  原始标记: 【{segment['original_tag']}】")
            logger.debug(f"  中文文本: {segment['following_text']}")
            if segment['motion_text']:
                logger.debug(f"  动作文本: （{segment['motion_text']}）")
            if segment['japanese_text']:
                logger.debug(f"  日文文本: <{segment['japanese_text']}>")
            logger.debug(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            if os.path.exists(segment['voice_file']):
                logger.debug(f"  对应语音: {os.path.basename(segment['voice_file'])}")
            else:
                if segment['japanese_text']:
                    logger.debug(f"  对应语音: (未生成或生成失败)")
    
    def get_memory(self):
        return self.messages
    
    def reset_memory(self):
        self.messages = [
            {
                "role": "system", 
                "content": self.ai_prompt
            }
        ]
    
    def _save_messages_to_rag(self, messages):
        if self.use_rag and self.active_rag_system:
            if not self.session_file_path:
                self.session_file_path = self.active_rag_system.get_history_filepath()
            try:
                self.active_rag_system.add_session_to_history(messages, session_filepath=self.session_file_path)
                logger.debug("当前会话已保存到RAG历史记录")
            except Exception as e:
                logger.error(f"保存会话到RAG历史记录失败: {e}")
            
        logger.debug("成功获取LLM响应")
    
    
    def _print_debug_message(self, current_context, rag_messages, messages):
        # if logger.should_print_context():
            logger.info("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in current_context:
                logger.info(f"Role: {message['role']}\nContent: {message['content']}\n")
                
            # 增加更详细的RAG信息日志
            if self.use_rag and rag_messages:
                logger.info("\n------ RAG增强信息详情 ------")
                logger.info(f"原始消息数: {len(messages)}，RAG增强后消息数: {len(current_context)}")
                logger.info(f"RAG增强消息数量: {len(rag_messages)}，位置: 系统提示后、用户消息前")
                
                # 计算并输出RAG消息的总长度（字符数）
                total_rag_chars = sum(len(msg.get('content', '')) for msg in rag_messages)
                logger.info(f"RAG增强内容总长度: {total_rag_chars} 字符")
                
                # 分析RAG消息类型统计
                role_counts = {}
                for msg in rag_messages:
                    role = msg.get('role', 'unknown')
                    role_counts[role] = role_counts.get(role, 0) + 1
                
                role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
                logger.info(f"RAG消息角色分布: {role_stats}")
                
            logger.info("------ 结束 ------")
    
    def _rag_append_sys_message(self, current_context, rag_messages, user_input):
        if not (self.use_rag and self.active_rag_system):
            return
        try:
            logger.debug("正在调用RAG系统检索相关历史信息...")
            # 清空原有内容，再 extend 新的消息
            rag_messages.clear()  
            new_messages = self.active_rag_system.prepare_rag_messages(user_input)
            rag_messages.extend(new_messages)  # 直接修改外部传入的列表
            if rag_messages:
                logger.debug(f"RAG系统返回了 {len(rag_messages)} 条上下文增强消息")
                    
                # 将RAG消息插入到系统提示后，用户消息前
                # 注意: 防止系统提示重复出现
                # 1. 找到人设提示位置
                last_system_index = 0
                        
                # 2. 过滤RAG消息中的系统提示词，避免重复
                filtered_rag_messages = []
                for msg in rag_messages:
                    # 只有当RAG消息是前缀/后缀提示，且不与原系统提示重复时才添加
                    if msg["role"] == "system":
                        is_duplicate = False
                        # 检查是否与原系统提示重复
                        for sys_msg in current_context[:last_system_index+1]:
                            if sys_msg["role"] == "system" and sys_msg["content"] == msg["content"]:
                                is_duplicate = True
                                break
                        if not is_duplicate:
                            filtered_rag_messages.append(msg)
                    else:
                        # 非系统消息直接添加
                        filtered_rag_messages.append(msg)
                
                if filtered_rag_messages:
                    # 计算插入位置：最后rag_window条消息之后，但至少要在第一个系统消息之后
                    insert_position = max(
                        last_system_index + 1,  # 确保在系统消息之后
                        len(current_context) - min(self.rag_window, len(current_context))  # 最后N条之后
                    )
                    
                    # 关键修改：直接操作原列表的切片赋值
                    current_context[insert_position:insert_position] = [
                        msg for msg in filtered_rag_messages 
                        if not (msg["role"] == "system" and 
                            any(sys_msg["content"] == msg["content"] 
                                for sys_msg in current_context[:last_system_index+1]))
                    ]
                else:
                    logger.debug("所有RAG消息被过滤，未向上下文添加新消息")
            else:
                logger.debug("RAG系统未返回相关历史信息")
        except Exception as e:
            logger.error(f"RAG处理过程中出错: {e}")
            logger.debug(f"RAG process error: {e}", exc_info=True)

    # 暂未调用该段代码↓        
    def load_memory_to_rag(self, messages):
        # 如果启用了RAG，尝试将加载的记忆添加到RAG历史记录
        if self.use_rag and self.active_rag_system:
            try:
                # 过滤掉系统提示词，只保留用户和助手的消息
                filtered_messages = [msg for msg in messages if msg.get('role') in ['user', 'assistant']]
                    
                if filtered_messages:
                    self.active_rag_system.add_session_to_history(filtered_messages)
                    logger.debug(f"加载的记忆已添加到RAG历史记录 (过滤后: {len(filtered_messages)}/{len(messages)} 条消息)")
                else:
                    logger.debug("过滤后无历史消息可添加到RAG")
            except Exception as e:
                    logger.error(f"将加载的记忆添加到RAG历史记录时出错: {e}")