import os
import json
from datetime import datetime
import glob

def parse_log_to_json(log_filepath):
    """
    解析单个.log文件，将其内容转换为JSON所需的聊天记录列表，并提取对话日期。

    Args:
        log_filepath (str): .log文件的路径。

    Returns:
        tuple: (datetime_object, list_of_chat_dicts)
               如果解析失败，则返回 (None, None)。
    """
    chat_records = []
    dialog_datetime = None

    try:
        with open(log_filepath, 'r', encoding='utf-8') as f:
            lines = [line.strip() for line in f.readlines() if line.strip()] # 读取并去除空行

        if not lines:
            print(f"警告: 文件 {log_filepath} 为空，跳过。")
            return None, None

        # 1. 解析对话日期
        first_line = lines[0]
        if first_line.startswith("对话日期:"):
            datetime_str = first_line.replace("对话日期:", "").strip()
            try:
                dialog_datetime = datetime.strptime(datetime_str, "%Y-%m-%d %H:%M:%S")
            except ValueError:
                print(f"错误: 文件 {log_filepath} 中的日期时间格式 '{datetime_str}' 不正确，应为 'YYYY-MM-DD HH:MM:SS'。跳过。")
                return None, None
        else:
            print(f"错误: 文件 {log_filepath} 第一行格式不正确，未找到 '对话日期:'。跳过。")
            return None, None

        # 2. 解析聊天内容
        # .log 文件格式特殊：AI的回复可能分布在多行，直到下一个 "用户:" 或文件末尾
        current_speaker = None
        current_content_parts = []

        # 从第二行开始处理对话内容
        for line in lines[1:]:
            if line.startswith("用户:"):
                # 如果之前有AI的内容，先保存
                if current_speaker == "assistant" and current_content_parts:
                    chat_records.append({
                        "role": "assistant",
                        "content": "\n".join(current_content_parts)
                    })
                    current_content_parts = []
                
                content = line.replace("用户:", "").strip()
                chat_records.append({"role": "user", "content": content})
                current_speaker = "user"

            # 假设AI的名字是 "钦灵"，可以根据实际情况修改或扩展
            elif line.startswith("钦灵:"):
                # 如果之前有AI的内容（理论上不应该，除非格式错误），也保存一下
                if current_speaker == "assistant" and current_content_parts:
                     chat_records.append({
                        "role": "assistant",
                        "content": "\n".join(current_content_parts)
                    })
                
                # 开始新的AI回复
                current_content_parts = [line.replace("钦灵:", "").strip()]
                current_speaker = "assistant"
            
            elif current_speaker == "assistant": # 如果当前是AI在说话，且行不以"用户:"或"钦灵:"开头，则认为是AI回复的延续
                current_content_parts.append(line)
            
            # else: # 忽略无法识别的行或将其视为错误
            #     print(f"警告: 在 {log_filepath} 中发现无法识别的行: '{line}'")


        # 处理循环结束后可能剩余的AI内容
        if current_speaker == "assistant" and current_content_parts:
            chat_records.append({
                "role": "assistant",
                "content": "\n".join(current_content_parts)
            })
        
        if not chat_records:
            print(f"警告: 文件 {log_filepath} 未能解析出任何聊天内容。")
            return dialog_datetime, [] # 返回日期和空列表，以便创建空json

        return dialog_datetime, chat_records

    except Exception as e:
        print(f"处理文件 {log_filepath} 时发生严重错误: {e}")
        return None, None

def main():
    # 获取当前脚本所在的目录作为根目录
    # 如果您想指定其他根目录，请修改下面这行
    root_directory = os.getcwd() 
    log_files_pattern = os.path.join(root_directory, "*.log")
    
    log_files = glob.glob(log_files_pattern)

    if not log_files:
        print(f"在目录 '{root_directory}' 下没有找到 .log 文件。")
        return

    print(f"找到以下 .log 文件: {log_files}")

    for log_filepath in log_files:
        print(f"\n正在处理文件: {log_filepath}")
        
        dialog_datetime, chat_data = parse_log_to_json(log_filepath)

        if dialog_datetime is None or chat_data is None:
            # parse_log_to_json内部已打印错误信息
            continue
        
        # 构建输出目录和文件名
        # YYYY年MM月\DD日\session_YYYYMMDD_HHMMSS.json
        year_str = str(dialog_datetime.year)
        month_str = dialog_datetime.strftime("%m") # 01, 02, ..., 12
        day_str = dialog_datetime.strftime("%d")   # 01, 02, ..., 31

        # 创建目录路径： YYYY年MM月
        month_folder_name = f"{year_str}年{month_str}月"
        # 创建完整的日文件夹路径： YYYY年MM月\DD日
        day_folder_name = f"{day_str}日"
        
        # 最终输出目录
        output_directory = os.path.join(root_directory, month_folder_name, day_folder_name)
        
        try:
            os.makedirs(output_directory, exist_ok=True) # exist_ok=True 避免目录已存在时报错
        except OSError as e:
            print(f"创建目录 {output_directory} 失败: {e}")
            continue

        # 构建文件名: session_YYYYMMDD_HHMMSS.json
        timestamp_filename_part = dialog_datetime.strftime("%Y%m%d_%H%M%S")
        output_filename = f"session_{timestamp_filename_part}.json"
        output_filepath = os.path.join(output_directory, output_filename)

        # 写入JSON文件
        try:
            with open(output_filepath, 'w', encoding='utf-8') as json_file:
                json.dump(chat_data, json_file, ensure_ascii=False, indent=4)
            print(f"成功转换并保存到: {output_filepath}")
        except IOError as e:
            print(f"写入JSON文件 {output_filepath} 失败: {e}")
        except Exception as e:
            print(f"在写入 {output_filepath} 时发生未知错误: {e}")


if __name__ == "__main__":
    main()