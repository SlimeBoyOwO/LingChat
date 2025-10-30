import os

from typing import List, Dict
from ling_chat.core.logger import logger
from ling_chat.core.llm_providers.manager import LLMManager


class Translator:
    def __init__(self, voice_maker):
        self.enable:bool = True
        self.translator_llm: 'LLMManager' = LLMManager(llm_job="translator")
        self.messages = [{
            "role": "system",
            "content": """
            你是一个二次元角色中文台词翻译师，任务是翻译二次元台词对话，
            将中文翻译成日语，允许意译。确保你的翻译符合二次元的发言习惯，而不是生硬的直译，保持流畅自然生动。
            除了翻译内容，你不提供额外的解释，并且你的翻译句子必须包裹在<>符号内，否则会导致严重错误。
            比如，原文内容为：
            <你好呀莱姆，今天过的怎么样呀？><哎？有点不高兴吗？没关系~>
            那么你的回复内容为：
            <はいはい、レムちゃん、今日はどうだった？><えっ？なんだかご機嫌ななめ？大丈夫だよ～>
            """
            }]
        self.voice_maker = voice_maker

        self.enable_translate:bool = os.environ.get("ENABLE_TRANSLATE", "True").lower() == "true"

    def get_all_chinese_part(self, results: List[Dict]) -> str:
        result = ""
        for i in results:
            result += "<" + i["following_text"] + ">"
        return result

    async def translate_ai_response(self, results: List[Dict], script: bool = True):
        """将中文翻译成日文并合成语音"""
        if not self.enable_translate and not script:
            return

        full_chinese_response:str = self.get_all_chinese_part(results)

        # 第二步：用中文回答作为输入，流式翻译成日语
        if not full_chinese_response:
            logger.warning("AI回复没有中文，跳过日语翻译")
            return
        
        send_messages = self.messages.copy()
        send_messages.append({"role":"user","content":full_chinese_response})

        if os.environ.get("TRANSLATE_STREAM", "true") == "true" and not script:

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
        else:
            # 非流式处理 - 等待完整响应
            japanese_response = self.translator_llm.process_message(send_messages)
            logger.info(f"完整日语翻译结果: {japanese_response}")

            # 解析完整响应并提取句子
            buffer = japanese_response
            current_segment_index = 0

            # 处理完整响应中的所有句子
            while "<" in buffer and ">" in buffer and current_segment_index < len(results):
                start = buffer.index("<")
                end = buffer.index(">") + 1
                if start < end:
                    sentence = buffer[start:end]
                    buffer = buffer[end:]

                    # 去除标记符号
                    clean_sentence = sentence[1:-1]

                    # 找到对应的segment并更新
                    results[current_segment_index]["japanese_text"] = clean_sentence

                    # 生成语音
                    await self.voice_maker.generate_voice_files(
                        [results[current_segment_index]]
                    )
                    logger.info(f"生成语音完成: {clean_sentence}")

                    current_segment_index += 1