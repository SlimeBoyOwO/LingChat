from datetime import datetime
from typing import List, Dict

class Function:
    @staticmethod
    def parse_chat_log(content: str) -> List[Dict[str, str]]:
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