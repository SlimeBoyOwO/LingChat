import os

from typing import List, Dict
from core.logger import logger
from core.llm_providers.manager import LLMManager

class Translator:
    def __init__(self, voice_maker):
        self.enable:bool = True
        self.translator_llm:'LLMManager' = LLMManager()
        self.messages = [{"role": "system", 
        "content": "你是一个二次元角色中文台词翻译师，任务是翻译二次元台词对话，将中文翻译成日语，允许意译，保持流畅自然生动。你的翻译格式和原文完全一致，没有任何多余内容。"}]
        self.voice_maker = voice_maker

        self.enable_translate:bool = os.environ.get("ENABLE_JAPANESE", "True").lower() == "true"

    def get_all_chinese_part(self, results: List[Dict]) -> str:
        result = ""
        for i in results:
            result += "<" + i["following_text"] + ">"
        return result

    async def translate_ai_response(self, results: List[Dict]):
        if not self.enable_translate:
            return

        full_chinese_response:str = self.get_all_chinese_part(results)

        # 第二步：用中文回答作为输入，流式翻译成日语
        if not full_chinese_response:
            logger.warning("AI回复没有中文，跳过日语翻译")
            return
        
        send_messages = self.messages.copy()
        send_messages.append({"role":"user","content":full_chinese_response})

        # 流式处理
        buffer = ""
        current_segment_index = 0

        japanese_stream = self.translator_llm.process_message_stream(send_messages)

        async for chunk in japanese_stream:
            print(chunk, end="", flush=True)
            buffer += chunk
            # 检测完整句子
            while "<" in buffer and ">" in buffer:
                start = buffer.index("<")
                end = buffer.index(">") + 1
                if start < end:
                    sentence = buffer[start:end]
                    buffer = buffer[end:]
                    
                    # 去除标记符号
                    clean_sentence = sentence[1:-1]
                    
                    # 找到对应的segment并更新
                    if current_segment_index < len(results):
                        results[current_segment_index]["japanese_text"] = clean_sentence
                        
                        # 实时生成语音
                        await self.voice_maker.generate_voice_files(
                            [results[current_segment_index]]
                        )
                        logger.info("开始生成下一条语音...")
                        
                        current_segment_index += 1