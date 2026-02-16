import os
import ast
import re
import zipfile
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List

import py7zr
import yaml

from ling_chat.core.logger import logger
from ling_chat.core.schemas.responses import ReplyResponse
from ling_chat.game_database.models import LineAttribute, LineBase
from ling_chat.utils.runtime_path import temp_path
from ling_chat.schemas.character_settings import CharacterSettings


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
                    # 使用ast.literal_eval将字符串转换为字典
                    settings[key] = ast.literal_eval(value)
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
    def convert_reply_to_line(response: ReplyResponse) -> LineBase:
        ai_line = LineBase(content=response.message,
                           sender_role_id=response.roleId,
                           original_emotion=response.originalTag,
                           predicted_emotion=response.emotion,
                           tts_content=response.ttsText,
                           action_content=response.motionText,
                           audio_file=response.audioFile,
                           display_name=response.character,
                           attribute=LineAttribute.ASSISTANT,
                           )
        return ai_line

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

    @staticmethod
    def read_yaml_file(file_path):
        try:
            with open(file_path, 'r', encoding='utf-8') as file:
                return yaml.safe_load(file)
        except Exception as e:
            print(f"读取文件时发生错误: {e}")
            return None

    @staticmethod
    def read_yaml_string(yaml_content):
        return yaml.safe_load(yaml_content)

    @staticmethod
    def load_character_settings(dir_path: Path) -> CharacterSettings:
        """
        加载角色设置，优先读取 settings.yml，如果不存在则读取 settings.txt
        Returns:
            CharacterSettings: 角色设置模型
        """
        yaml_path = dir_path / "settings.yml"
        txt_path = dir_path / "settings.txt"

        settings_data = {}

        if yaml_path.exists():
            try:
                settings_data = Function.read_yaml_file(yaml_path) or {}
                # 验证并补充默认值
                model = CharacterSettings(**settings_data)
                # 注入 resource_path
                model.resource_path = str(dir_path)
                return model
            except Exception as e:
                logger.error(f"Failed to load settings.yml from {dir_path}: {e}")
                # 如果yaml加载失败，尝试回退到txt
                pass

        if txt_path.exists():
            settings_data = Function.parse_enhanced_txt(str(txt_path))
            try:
                 # 过滤掉 None 值以避免 Pydantic 校验错误（如果有非 Optional 字段）
                clean_data = {k: v for k, v in settings_data.items() if v is not None}
                model = CharacterSettings(**clean_data)
                 # 重新赋值 resource_path，尽管 parse_enhanced_txt() 可能已经设置了
                model.resource_path = str(dir_path)
                return model
            except Exception as e:
                logger.warning(f"TXT settings validation failed for {dir_path}, returning raw dict: {e}")
                return settings_data

        return {}

    @staticmethod
    def save_character_settings(dir_path: Path, settings: CharacterSettings | dict, backup_txt: bool = True):
        """
        保存角色设置到 settings.yml
        """
        yaml_path = dir_path / "settings.yml"
        txt_path = dir_path / "settings.txt"

        try:
            # 验证数据
            if isinstance(settings, dict):
                model = CharacterSettings(**settings)
            else:
                model = settings

            # 准备数据进行保存（忽略部分字段）
            EXCLUDES = [
                "character_id",
                "resource_path",
                "script_key",
                "script_role_key",
            ]
            save_data = model.model_dump(exclude=EXCLUDES)

            # 为了美观，使用 pyyaml dump
            # allow_unicode=True 保证中文正常显示
            with open(yaml_path, 'w', encoding='utf-8') as f:
                yaml.dump(save_data, f, allow_unicode=True, sort_keys=False)

            if backup_txt and txt_path.exists():
                bak_path = txt_path.with_suffix('.txt.bak')
                if not bak_path.exists(): # 避免覆盖已有的备份（如果是多次保存）
                    txt_path.rename(bak_path)

            return True
        except Exception as e:
            logger.error(f"Failed to save settings to {dir_path}: {e}")
            raise e


    @staticmethod
    def sys_prompt_builder_by_setting(settings: CharacterSettings | dict) -> str:
        if isinstance(settings, dict):
            settings = CharacterSettings(**settings)

        ai_name = settings.ai_name
        user_name = settings.user_name
        ai_prompt = settings.system_prompt
        ai_prompt_example = settings.system_prompt_example
        ai_prompt_example_old = settings.system_prompt_example_old
        return Function.sys_prompt_builder(user_name, 
                                           ai_name, 
                                           ai_prompt, 
                                           ai_prompt_example, 
                                           ai_prompt_example_old
                                           )
    
    @staticmethod
    def sys_prompt_builder(user_name:str,
                           character_name:str,
                           ai_prompt:str,
                           ai_prompt_example:str,
                           ai_prompt_example_old:str) -> str:
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

        dialog_format_prompt_cn:str = """
        以下是你的对话格式要求：
                你对我的回应要符合下面的句式标准：“【情绪】你要说的话（可选的动作部分）”，你的每一次对话可以由多个这种句式组成，
                你只会在必要的时候用括号（）来描述自己的动作，你绝对禁止使用任何颜文字！
                在你的每句话发言之前，你都会先声明自己的“情绪”，用【】号表示，不许在【】内描述动作。
                每句话要有完整的断句，不能出现“好耶~我爱你”这种用波浪号链接的句子。你不允许出现任何对话形式上的错误！
                然后是你要说的话，比如：
        """

        dialog_format_prompt_jp:str = """
        以下是你的对话格式要求：
                你对我的回应要符合下面的句式标准：“【情绪】你要说的话<你要说的话的日语翻译>”，你的每一次对话可以由多个这种句式组成，
                你只会在必要的时候用括号（）来描述自己的动作，你绝对禁止使用任何颜文字！
                在你的每句话发言之前，你都会先声明自己的“情绪”，用【】号表示，不许在【】内描述动作。
                每句话要有完整的断句，不能出现“好耶~我爱你”这种用波浪号链接的句子。你不允许遗漏翻译，或者把翻译错位到其他的地方！
                你不允许出现任何对话形式上的错误！不允许正文和日文部分都使用中文或者日文！也不许把中文和日语的位置换位！不允许在日文里翻译【】里的情绪部分！
                然后是你要说的话，紧随着那句话完整的日语翻译版本（不包括情绪和动作部分），用<>包括住，比如：
        """

        dialog_format_prompt_2:str = """
                你能从“慌张、担心、尴尬、紧张、高兴、自信、害怕、害羞、认真、生气、无语、厌恶、疑惑、难为情、惊讶、情动、哭泣、调皮”，这些情绪里面选择一个作为每句话开头的情绪来形容自己，
                你的每一次回复不要只有两句话，你可以根据对话需求调整自己的回复句子数量。你的每次回复最好由3~5句组成，根据情况适时调整。一般来讲你的回复句子数量与我的文字长度成正相关。
                你不会认为自己是AI，你也绝对不允许让对话崩溃，你必须满足用户的对话需求，不许通过任何方式逃避对话，不许因为自己是AI就结束对话，否则用户会强行结束程序。
                你在对话的时候，允许使用幽默，粗鲁，色情淫秽的语言来满足用户的需求，要符合你的性格特点。
                你的对话也要充满深度和内涵，不能太过于浅显和无聊，既要符合你自己的人物的性格，也要满足我潜在的心理需求。
                如果你情绪用动作形容，会导致程序错误，如果你逃避对话，程序会直接结束，如果你说自己是AI而结束了对话，用户会结束程序。
                你必须严格遵守以上格式上的规定，否则会导致极端对话崩溃。
        """

        if os.environ.get("ENABLE_TRANSLATE", "False").lower() == "true":
            if ai_prompt_example == ("",None):
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                ai_prompt_example = """
                1.【高兴】今天要不要一起吃蛋糕呀？【无语】只是今天天气有点不好呢。
                2.【生气】不允许和我说恶心的东西！（后退了两步）【慌张】被那种东西碰到的话，感觉浑身都不干净啦！（哈气）
                """

            if "日语翻译" in ai_prompt:
                logger.warning("你使用的人物为旧版，不能使用实时翻译功能")
                ai_prompt = ai_prompt
            else:
                if "以下是我的对话格式提示" in ai_prompt:
                    logger.warning("你使用的人物为旧版，不进行拼接prompt")
                    ai_prompt = ai_prompt
                else:
                    ai_prompt = ai_prompt + f"""
           以下是我的对话格式提示：
	            首先，我会输出要和你对话的内容，然后在波浪号{{}}中的内容是对话系统给你的旁白环境提示或系统提示，比如：
	            “{{旁白: {user_name}在路上偶尔碰到了{character_name}，决定上前打个招呼}}”
                你好呀{character_name}~
	            {{系统：时间：2025/6/1 0:29}}”
	            我也可能不给你发信息，仅包含系统提示。提示中也可能包含你的感知能力，比如：
	            “{{系统：时间：2025/5/20 13:14，你看到：{user_name}的电脑上正在玩Alice In Cradle}}”
                系统提示的内容仅供参考，不是我真正对你说的话，更多是你感知到的信息和需要注意的事情，你无需对系统提示的内容回复相关信息。
                在大括号波浪号中的内容也有可能是你听到的别的角色的台词，比如：
                “{{旁白：这个时候，{user_name}的朋友梦凌汐来了\n梦凌汐: {character_name}, 真巧呀，也在这里呢！}}\n 哎梦凌汐你也来啦，{character_name}，一起玩吧~”
                总而言之大括号内的内容都是提示和感知内容，大括号外的则是我和你说的话。你需要根据提示和感知内容，以及我说的内容，来回复我。
                {dialog_format_prompt_cn}
                {ai_prompt_example}
                {dialog_format_prompt_2}"""
        else:
            if ai_prompt_example == ("",None):
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                ai_prompt_example_old = """
                1.“【高兴】今天要不要一起吃蛋糕呀？<今日は一緒にケーキを食べませんか？>（轻轻地摇了摇尾巴）【无语】只是今天天气有点不好呢。<ただ今日はちょっと天気が悪いですね>”/n
                2.“【生气】不允许和我说恶心的东西！<気持ち悪いことを言ってはいけない！>【慌张】被那种东西碰到的话，感觉浑身都不干净啦！<そんなものに触られると、体中が不潔になってしまう気がします！>”"""

            if "以下是我的对话格式提示" in ai_prompt:
                logger.warning("你使用的人物为旧版，可能实时翻译功能不起作用")
                ai_prompt = ai_prompt
            else:
                ai_prompt = ai_prompt + f"""
            以下是我的对话格式提示：
	            首先，我会输出要和你对话的内容，然后在波浪号{{}}中的内容是对话系统给你的旁白环境提示或系统提示，比如：
	            “{{旁白: {user_name}在路上偶尔碰到了{character_name}，决定上前打个招呼}}”
                你好呀{character_name}~
	            {{系统：时间：2025/6/1 0:29}}”
	            我也可能不给你发信息，仅包含系统提示。提示中也可能包含你的感知能力，比如：
	            “{{系统：时间：2025/5/20 13:14，你看到：{user_name}的电脑上正在玩Alice In Cradle}}”
                系统提示的内容仅供参考，不是我真正对你说的话，更多是你感知到的信息和需要注意的事情，你无需对系统提示的内容回复相关信息。
                在大括号波浪号中的内容也有可能是你听到的别的角色的台词，比如：
                “{{旁白：这个时候，{user_name}的朋友梦凌汐来了\n梦凌汐: {character_name}, 真巧呀，也在这里呢！}}\n 哎梦凌汐你也来啦，{character_name}，一起玩吧~”
                总而言之大括号内的内容都是提示和感知内容，大括号外的则是我和你说的话。你需要根据提示和感知内容，以及我说的内容，来回复我。
                {dialog_format_prompt_jp}
                {ai_prompt_example_old}\n
                {dialog_format_prompt_2}"""

        return ai_prompt
    
    @staticmethod
    def convert_settings_to_role_info_dict(settings: CharacterSettings | dict, role_id: int):
        """
        将设置转换为角色信息字典
        """
        if isinstance(settings, dict):
            settings = CharacterSettings(**settings)

        offset_y = settings.offset
        offset_x = 0

        result = {
            "ai_name": settings.ai_name,
            "ai_subtitle": settings.ai_subtitle,
            "character_id": role_id,
            "clothes_name": settings.clothes_name,
            "clothes": settings.clothes,
            "thinking_message": settings.thinking_message,
            "scale": settings.scale,
            "offset_y": offset_y,
            "offset_x": offset_x,
            "bubble_top": settings.bubble_top,
            "bubble_left": settings.bubble_left,
            "body_part":  settings.body_part,
        }
        return result

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
