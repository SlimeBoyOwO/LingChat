import asyncio
import os
from typing import List, Dict, Optional
import traceback
import time

from ling_chat.utils.function import Function

class StreamProducer:
    """
    (已修正)
    消费 LLM 数据流，使用您提供的精确逻辑将其解析为句子，并放入队列中。
    """
    def __init__(self,
                 llm_stream,
                 sentence_queue: asyncio.Queue,
                 publish_events: Dict[int, asyncio.Event]):
        self.llm_stream = llm_stream
        self.sentence_queue = sentence_queue
        self.publish_events = publish_events

    async def runx(self) -> str:
        """
        启动从流中生产句子的过程。
        此方法现在使用了您提供的、经过验证的缓冲区处理逻辑。
        返回完整的、累积的 AI 响应文本。
        """
        accumulated_response = ""
        sentence_index = 0
        realtime_display_buffer = ""
        last_display_time = 0
        buffer = ""
        sentence = ""

        print("\n=== AI回复流式输出 ===")
        
        async for chunk in self.llm_stream:
            buffer += chunk
            accumulated_response += chunk
            realtime_display_buffer += chunk

            # 实时显示流式内容（完全按照您的逻辑）
            current_time = time.time()
            if (len(realtime_display_buffer) >= 3 or
                current_time - last_display_time > 0.1 or
                '\n' in realtime_display_buffer):
                
                display_text = realtime_display_buffer
                if display_text.strip():
                    print(display_text, end='', flush=True)
                
                realtime_display_buffer = ""
                last_display_time = current_time

            # 句子解析逻辑（完全按照您的逻辑）
            while "【" in buffer:
                # 如果已经有句子开头，检查是否有结束符
                if sentence and "】" in buffer:
                    end_index = buffer.find("】")
                    sentence += buffer[:end_index + 1]
                    buffer = buffer[end_index + 1:]

                    next_start = buffer.find("【")
                    if next_start != -1:
                        sentence += buffer[:next_start]
                        buffer = buffer[next_start:]
                    else:
                        sentence += buffer
                        buffer = ""

                    # 处理完整句子
                    current_index = sentence_index
                    self.publish_events[current_index] = asyncio.Event()
                    await self.sentence_queue.put((sentence, current_index, False))
                    sentence_index += 1
                    sentence = ""
                # 如果句子还没有开始，寻找句子的开头
                elif not sentence and "】" in buffer:
                    start_index = buffer.find("【")
                    # 确保我们处理的是有效的【...】对
                    if start_index < buffer.find("】"):
                        sentence_part = buffer[start_index:]
                        buffer = buffer[:start_index] # 保留【之前的内容
                        
                        end_index = sentence_part.find("】")
                        sentence = sentence_part[:end_index + 1]
                        sentence_part = sentence_part[end_index + 1:]

                        next_start = sentence_part.find("【")
                        if next_start != -1:
                            sentence += sentence_part[:next_start]
                            sentence_part = sentence_part[next_start:]
                        else:
                            sentence += sentence_part
                            sentence_part = ""

                        # 处理找到的句子
                        current_index = sentence_index
                        self.publish_events[current_index] = asyncio.Event()
                        await self.sentence_queue.put((sentence, current_index, False))
                        sentence_index += 1
                        sentence = ""
                        buffer += sentence_part # 将剩余部分放回缓冲区
                    else:
                        break # 避免【在】之后导致死循环
                else:
                    # 缓冲区中只有【但没有】，或者没有句子开头，等待更多数据
                    break

        # 显示剩余的内容
        if realtime_display_buffer:
            if realtime_display_buffer.strip():
                print(realtime_display_buffer, end='', flush=True)

        # 处理最后一个句子（完全按照您的逻辑）
        final_content = sentence + buffer
        if final_content:
            final_content = Function.fix_ai_generated_text(final_content)
            # 注意：这里的 accumulated_response 已经包含了所有chunk，我们只需要修复它
            accumulated_response = Function.fix_ai_generated_text(accumulated_response)

            if final_content.strip():
                print(final_content, end='', flush=True)

            current_index = sentence_index
            self.publish_events[current_index] = asyncio.Event()
            await self.sentence_queue.put((final_content, current_index, True)) # is_final=True
            sentence_index += 1

        print("\n=== 流式输出结束 ===")
        return accumulated_response
    
    async def run(self):

        accumulated_response = ""
        sentence_index = 0
        realtime_display_buffer = ""
        last_display_time = 0
        buffer = ""
        sentence = ""

        # 创建流式响应生成器
        ai_response_stream = self.llm_stream

        buffer = ""
        sentence = ""

        # 打印开始提示
        print("\n=== AI回复流式输出 ===")
        
        async for chunk in ai_response_stream:
            buffer += chunk
            accumulated_response += chunk
            realtime_display_buffer += chunk

            # 实时显示流式内容（每收到一定内容或时间间隔显示）
            current_time = time.time()
            if (len(realtime_display_buffer) >= 3 or  # 每3个字符显示一次
                current_time - last_display_time > 0.1 or  # 或者每100毫秒
                '\n' in realtime_display_buffer):  # 或者有换行符
                
                # 清理情绪标签以便更好地显示
                display_text = realtime_display_buffer
                if display_text.strip():
                    print(display_text, end='', flush=True)
                
                realtime_display_buffer = ""
                last_display_time = current_time

            while "【" in buffer:
                # 如果已经有句子开头，检查是否有结束符
                if sentence and "】" in buffer:
                    end_index = buffer.index("】")
                    sentence += buffer[:end_index+1]
                    buffer = buffer[end_index+1:]         # buffer 删除前面被裁剪的情绪部分【情绪】

                    # 检查是否还有内容直到下一个【
                    next_start = buffer.find("【")
                    if next_start != -1:
                        sentence += buffer[:next_start]   # buffer 删除情绪后面跟随的句子和动作等信息"你好呀（摇尾巴）"
                        buffer = buffer[next_start:]
                    else:
                        sentence += buffer
                        buffer = ""

                    # 处理完整句子
                    current_index = sentence_index
                    self.publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
                    await self.sentence_queue.put((sentence, current_index, False)) # is_final=False
                    sentence_index += 1
                    # asyncio.create_task(self.process_sentence_and_send(sentence, user_message, False))
                    # await self.process_sentence(sentence, emotion_segments)

                    sentence = ""
                else:
                    # 找到句子的开始
                    start_index = buffer.index("【")
                    sentence = buffer[:start_index+1]
                    buffer = buffer[start_index+1:]

                    # 查找结束括号
                    num_end = 0
                    while num_end < len(buffer) and buffer[num_end].isdigit():
                        num_end += 1

                    if num_end > 0 and num_end < len(buffer) and buffer[num_end] == "】":
                        sentence += buffer[:num_end+1]
                        buffer = buffer[num_end+1:]

                        # 查找下一个句子开始
                        next_start = buffer.find("【")
                        if next_start != -1:
                            sentence += buffer[:next_start]
                            buffer = buffer[next_start:]
                        else:
                            sentence += buffer
                            buffer = ""

                        # 处理完整句子
                        current_index = sentence_index
                        self.publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
                        await self.sentence_queue.put((sentence, current_index, False)) # is_final=False
                        sentence_index += 1
                        # asyncio.create_task(self.process_sentence_and_send(sentence, user_message, False))
                        # await self.process_sentence(sentence, emotion_segments)

                        sentence = ""
                    else:
                        # 不完整的句子部分，继续等待
                        break

        # 显示剩余的内容
        if realtime_display_buffer:
            display_text = realtime_display_buffer
            if display_text.strip():
                print(display_text, end='', flush=True)

        # 处理最后一个句子
        final_content = sentence + buffer
        if final_content:
            # 修复ai回复中可能出错的部分
            final_content = Function.fix_ai_generated_text(final_content)
            accumulated_response = Function.fix_ai_generated_text(accumulated_response)

            # 显示最后的内容
            final_display_text = final_content
            if final_display_text.strip():
                print(final_display_text, end='', flush=True)

            # 使用process_sentence方法处理最后一个句子
            current_index = sentence_index
            self.publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
            await self.sentence_queue.put((final_content, current_index, True)) # is_final=False
            sentence_index += 1
            # asyncio.create_task(self.process_sentence_and_send(final_content, user_message, True))
            # await self.process_sentence(final_content, emotion_segments)

        # 打印结束换行
        print("\n=== 流式输出结束 ===")