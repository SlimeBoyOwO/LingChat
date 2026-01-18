import os
import re
import zipfile
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List

import py7zr
import yaml

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import temp_path


class Function:
    # 该列表内被管理的字段,在值为空字符串时,会被解析为None
    HIDE_NONE_FIELDS = [
        'ai_name', 'ai_subtitle', 'user_name', 'user_subtitle', 'thinking_message',
    ]
    @staticmethod
    def detect_language(text):
        """
        判断输入文本是中文还是日文

        参数:
            text (str): 要检测的文本

        返回:
            str: "Chinese", "Japanese" 或 "Unknown"
        """
        chinese_ranges = [
            (0x4E00, 0x9FFF),  # 基本汉字
            (0x3400, 0x4DBF),  # 扩展A
            (0x20000, 0x2A6DF),  # 扩展B
            (0x2A700, 0x2B73F),  # 扩展C
            (0x2B740, 0x2B81F),  # 扩展D
            (0x2B820, 0x2CEAF),  # 扩展E
            (0xF900, 0xFAFF),  # 兼容汉字
            (0x3300, 0x33FF),  # 兼容符号
        ]

        japanese_ranges = [
            (0x3040, 0x309F),  # 平假名
            (0x30A0, 0x30FF),  # 片假名
            (0x31F0, 0x31FF),  # 片假名音标扩展
            (0xFF65, 0xFF9F),  # 半角片假名
        ]

        chinese_count = 0
        japanese_count = 0

        for char in text:
            code = ord(char)

            for start, end in chinese_ranges:
                if start <= code <= end:
                    chinese_count += 1
                    break

            for start, end in japanese_ranges:
                if start <= code <= end:
                    japanese_count += 1
                    break

        if chinese_count > 0 and japanese_count == 0:
            return "Chinese_ABS"
        elif japanese_count < chinese_count:
            return "Chinese"
        else:
            return "Japanese"

    @staticmethod
    def fix_ai_generated_text(text: str) -> str:
        """规范化带有情绪标签的文本，修正不符合格式的部分"""
        # 首先使用原函数的正则表达式分割文本
        emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)

        if not emotion_segments:
            return text  # 如果没有找到任何情绪标签，返回原文本

        normalized_parts = []

        for full_tag, emotion_tag, following_text in emotion_segments:
            following_text = following_text.replace('(', '（').replace(')', '）')

            # 提取日语部分和动作文本（与原函数一致）
            japanese_match = re.search(r'<(.*?)>', following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            motion_match = re.search(r'（(.*?)）', following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

            # 如果日语文本存在，移除其中的动作标注
            if japanese_text:
                japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

            # 检查是否需要交换日语和中文文本
            if japanese_text and cleaned_text:
                try:
                    lang_jp = Function.detect_language(japanese_text)
                    lang_clean = Function.detect_language(cleaned_text)

                    if (lang_jp in ['Chinese', 'Chinese_ABS'] and
                            lang_clean in ['Japanese', 'Chinese'] and
                            lang_clean != 'Chinese_ABS'):
                        # 交换位置
                        cleaned_text, japanese_text = japanese_text, cleaned_text
                except Exception as e:
                    print(f"语言检测错误: {e}")

            # 重建规范化后的文本部分
            normalized_part = full_tag
            if cleaned_text:
                normalized_part += cleaned_text
            if japanese_text:
                normalized_part += f"<{japanese_text}>"
            if motion_text:
                normalized_part += f"（{motion_text}）"

            # 检查是否有有效内容（至少要有情绪标签和中文部分）
            if cleaned_text or (japanese_text and not cleaned_text):
                normalized_parts.append(normalized_part)

        # 重新组合规范化后的文本
        return "".join(normalized_parts)

    @staticmethod
    def parse_enhanced_txt(file_path):
        """
        解析settings.txt，包含里面的全部信息并且附带文件路径。

        Args:
            file_path (str): settings.txt的文件路径。

        Returns:
            settings :(dict) 返回角色的所有信息。
        """
        settings = {}
        with open(file_path, 'r', encoding='utf-8') as file:
            content = file.read()

        single_line_pattern = re.compile(r'^(\w+)\s*=(.*?)\s*$', re.MULTILINE)
        multi_line_pattern = re.compile(r'^(\w+)\s*=\s*"""(.*?)"""\s*$', re.MULTILINE | re.DOTALL)
        dict_pattern = re.compile(r'^(\w+)\s*=\s*({.*?})\s*$', re.MULTILINE | re.DOTALL)

        # 处理多行字符串
        for match in multi_line_pattern.finditer(content):
            key = match.group(1)
            value = match.group(2).strip()
            settings[key] = value

        # 处理字典类型
        for match in dict_pattern.finditer(content):
            key = match.group(1)
            value = match.group(2).strip()
            if key not in settings:  # 避免被多行字符串覆盖
                try:
                    # 使用eval将字符串转换为字典
                    settings[key] = eval(value)
                except:
                    # 如果解析失败，保留原始字符串
                    settings[key] = value

        # 处理单行值
        for match in single_line_pattern.finditer(content):
            key = match.group(1)
            if key not in settings:
                value = match.group(2).strip()
                if (value.startswith('"') and value.endswith('"')) or \
                        (value.startswith("'") and value.endswith("'")):
                    value = value[1:-1]
                if value == '' and key in Function.HIDE_NONE_FIELDS:
                    value = None
                settings[key] = value

        dir_path = os.path.dirname(file_path)
        settings['resource_path'] = dir_path

        return settings

    @staticmethod
    def parse_chat_log(content: str) -> tuple[datetime | None, List[Dict[str, str]] | None]:
        """
        解析聊天内容字符串，将其转换为JSON所需的聊天记录列表，并提取对话日期。

        Args:
            content (str): 包含聊天记录的字符串内容。

        Returns:
            tuple: (datetime_object, list_of_chat_dicts)
                如果解析失败，则返回 (None, None)。
        """
        chat_records = []
        dialog_datetime = None

        try:
            lines = content.split('\n')

            # 1. 解析对话日期
            first_line = lines[0]
            if first_line.startswith("对话日期:"):
                datetime_str = first_line.replace("对话日期:", "").strip()
                try:
                    dialog_datetime = datetime.strptime(datetime_str, "%Y-%m-%d %H:%M:%S")
                except ValueError:
                    print("错误: 聊天记录中的时间格式错误")
                    return None, None
            else:
                print("错误: 聊天记录格式不正确，未找到 '对话日期:'")
                return None, None

            # 2. 解析聊天内容
            current_speaker = None
            current_content_parts = []

            # 从第二行开始处理对话内容
            for line in lines[1:]:
                # 处理系统设定
                if line.startswith("设定:"):
                    # 如果之前有其他内容，先保存
                    if current_speaker and current_content_parts:
                        chat_records.append({
                            "role": current_speaker,
                            "content": "\n".join(current_content_parts)
                        })
                        current_content_parts = []

                    # 开始收集系统设定内容
                    current_content_parts = [line.replace("设定:", "").strip()]
                    current_speaker = "system"

                elif line.startswith("用户:"):
                    # 如果之前有内容，先保存
                    if current_speaker and current_content_parts:
                        chat_records.append({
                            "role": current_speaker,
                            "content": "\n".join(current_content_parts)
                        })
                        current_content_parts = []

                    # 开始新的用户输入
                    current_content_parts = [line.replace("用户:", "").strip()]
                    current_speaker = "user"

                elif line.startswith("钦灵:"):
                    # 如果之前有内容，先保存
                    if current_speaker and current_content_parts:
                        chat_records.append({
                            "role": current_speaker,
                            "content": "\n".join(current_content_parts)
                        })
                        current_content_parts = []

                    # 开始新的AI回复
                    current_content_parts = [line.replace("钦灵:", "").strip()]
                    current_speaker = "assistant"

                else:
                    # 如果当前行不是新的对话开始，则作为当前说话者的内容继续
                    if current_speaker:
                        current_content_parts.append(line)

            # 处理循环结束后可能剩余的内容
            if current_speaker and current_content_parts:
                chat_records.append({
                    "role": current_speaker,
                    "content": "\n".join(current_content_parts)
                })

            if not chat_records:
                print("警告: 未能解析出任何聊天内容")
                return dialog_datetime, []

            return dialog_datetime, chat_records

        except Exception as e:
            print(f"解析聊天记录时发生错误: {str(e)}")
            return None, None

    @staticmethod
    def extract_archive(archive_path: Path, extract_to: Path):
        """
        解压压缩文件到指定目录，支持7z和zip格式

        :param archive_path: 压缩文件路径(7z或zip)
        :param extract_to: 解压目标目录
        :raises ValueError: 当文件格式不支持时
        """
        # 避免logger那边的环境变量没拿到，延迟一下import
        from ling_chat.core.logger import logger

        logger.info(f"正在解压 {archive_path} 到 {extract_to}...")

        try:
            # 确保目标目录存在
            extract_to.mkdir(parents=True, exist_ok=True)

            # 根据后缀选择解压方式
            suffix = archive_path.suffix.lower()

            if suffix == '.7z':
                with py7zr.SevenZipFile(archive_path, mode='r') as z:
                    z.extractall(path=extract_to)
                logger.info(f"成功解压 {archive_path} 到 {extract_to}")
            elif suffix == '.zip':
                with zipfile.ZipFile(archive_path, 'r') as z:
                    z.extractall(path=extract_to)
                logger.info(f"成功解压 {archive_path} 到 {extract_to}")
            else:
                logger.warning(f"不支持的压缩格式: {suffix}. 仅支持 .7z 和 .zip")

        except Exception as e:
            logger.warning(f"解压失败: {e}")


    @staticmethod
    def find_next_time(schedule_times: list[str]) -> str:
        """计算到下一个提醒时间的秒数"""
        now = datetime.now()
        current_time = now.time()
        current_time_str = current_time.strftime("%H:%M")

        # 将时间字符串转换为时间对象
        process_schedule_times = [datetime.strptime(time_str, "%H:%M").time() for time_str in schedule_times]

        # 找到下一个提醒时间
        next_time = None
        for time_obj in sorted(process_schedule_times):
            if time_obj > current_time:
                next_time = time_obj
                break

        # 如果没有找到今天的时间，就用明天第一个时间
        if next_time is None and schedule_times:
            next_time = schedule_times[0]
            # 计算到明天这个时间的秒数
            tomorrow = now + timedelta(days=1)
            next_datetime = datetime.combine(tomorrow.date(), next_time)
        else:
            next_datetime = datetime.combine(now.date(), next_time)

        ans = next_datetime.strftime("%H:%M")
        return ans


    @staticmethod
    def calculate_time_to_next_reminder(schedule_times: list[str]) -> float:
        schedule_times.sort()

        """计算到下一个提醒时间的秒数"""
        now = datetime.now()
        current_time = now.time()
        current_time_str = current_time.strftime("%H:%M")

        next_time = None
        try:
            # 将时间字符串转换为时间对象
            process_schedule_times = [datetime.strptime(time_str, "%H:%M").time() for time_str in schedule_times]
            # 找到下一个提醒时间
            for time_obj in sorted(process_schedule_times):
                if time_obj > current_time:
                    next_time = time_obj
                    break
        except Exception as e:
            print(e)

        # 如果没有找到今天的时间，就用明天第一个时间
        if next_time is None and schedule_times:
            next_time = schedule_times[0]
            # 计算到明天这个时间的秒数
            tomorrow = now + timedelta(days=1)
            next_datetime = datetime.combine(tomorrow.date(), next_time)
        else:
            next_datetime = datetime.combine(now.date(), next_time)

        time_difference = next_datetime - now
        return max(0, time_difference.total_seconds())

    @staticmethod
    def format_seconds(seconds: float) -> str:
        """将秒数格式化为易读的时间字符串"""
        hours = int(seconds // 3600)
        minutes = int((seconds % 3600) // 60)
        seconds = int(seconds % 60)

        if hours > 0:
            return f"{hours}小时{minutes}分{seconds}秒"
        elif minutes > 0:
            return f"{minutes}分{seconds}秒"
        else:
            return f"{seconds}秒"

    # 方法1：直接读取 YAML 文件
    @staticmethod
    def read_yaml_file(file_path):
        try:
            with open(file_path, 'r', encoding='utf-8') as file:
                data = yaml.safe_load(file)
            return data
        except Exception as e:
            print(f"读取文件时发生错误: {e}")
            return None


    # 方法2：如果你已经有 YAML 字符串内容
    @staticmethod
    def read_yaml_string(yaml_content):
        data = yaml.safe_load(yaml_content)
        return data

    def clean_temp_files(self):
        """
        清理所有临时文件
        """
        temp_dir = Path(os.environ.get("TEMP_VOICE_DIR", temp_path / "data/voice"))
        self.format = os.environ.get("VOICE_FORMAT", "wav")

        for file in temp_dir.glob(f"*.{self.format}"):
            try:
                file.unlink()
            except Exception as e:
                logger.warning(f"清理临时文件失败 {file.name}: {str(e)}")
