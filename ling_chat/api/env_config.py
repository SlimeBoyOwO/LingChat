import os
import re
from typing import Any, Dict

from fastapi import APIRouter, Body, HTTPException

from ling_chat.utils.runtime_config import apply_runtime_config_changes
from ling_chat.utils.runtime_path import package_root

router = APIRouter()
env_file_path = package_root.parent / '.env'
tmp_env_file_path = package_root.parent / '.env.example'

def parse_env_file():
    """
    一个基于状态机的、健壮的.env文件解析器。
    此版本从根本上解决了多行值解析的缺陷。
    """
    if not os.path.exists(env_file_path):
        return {}

    structured_config = {}
    current_category = None
    current_subcategory = None

    category_begin_re = re.compile(r'^#\s*([^#]+?)\s*BEGIN')
    subcategory_begin_re = re.compile(r'^##\s*([^#]+?)\s*BEGIN(?:\s*#\s*(.*))?$')
    category_end_re = re.compile(r'^#\s*([^#]+?)\s*END')
    subcategory_end_re = re.compile(r'^##\s*([^#]+?)\s*END')
    env_var_re = re.compile(r'^([A-Z_0-9]+)=')
    type_re = re.compile(r'\[type:\s*(\w+)\s*\]')

    # --- 核心修改：引入状态机来处理多行 ---
    in_multiline_block = False
    multiline_buffer = ""
    current_key = None

    def process_setting(key, value_block):
        # 辅助函数，用于处理一个完整的（单行或多行）键值对
        value_block = value_block.strip()

        # 从块的末尾提取注释
        comment_match = re.search(r'\s*#\s*(.*)$', value_block)
        if comment_match:
            full_description = comment_match.group(1).strip()
            value_str = value_block[:comment_match.start()].strip()
        else:
            full_description = ""
            value_str = value_block

        # 解析类型
        input_type = 'text'
        type_match = type_re.search(full_description)
        if type_match:
            input_type = type_match.group(1).lower()
            description = type_re.sub('', full_description).strip()
        else:
            description = full_description

        # 清理引号
        if value_str.startswith('"') and value_str.endswith('"'):
            value = value_str[1:-1]
        else:
            value = value_str

        if input_type == 'text' and value.lower() in ['true', 'false']:
            input_type = 'bool'

        return {
            'key': key, 'value': value, 'description': description, 'type': input_type
        }

    with open(env_file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    for line in lines:
        # --- 状态1: 如果正在读取一个多行值 ---
        if in_multiline_block:
            multiline_buffer += line
            # 如果在当前行找到了结束的双引号，说明多行块结束
            if '"' in line.split('#')[0]:
                setting = process_setting(current_key, multiline_buffer)
                structured_config[current_category]['subcategories'][current_subcategory]['settings'].append(setting)
                in_multiline_block = False
                multiline_buffer = ""
                current_key = None
            continue

        # --- 状态2: 不在多行值中，进行常规解析 ---
        line_strip = line.strip()
        if not line_strip or (line_strip.startswith('#') and not (category_begin_re.match(line_strip) or subcategory_begin_re.match(line_strip))):
            continue

        cat_match = category_begin_re.match(line_strip)
        sub_match = subcategory_begin_re.match(line_strip)
        env_match = env_var_re.match(line)

        if cat_match:
            current_category = cat_match.group(1).strip()
            if current_category not in structured_config:
                structured_config[current_category] = {'subcategories': {}}
        elif sub_match and current_category:
            current_subcategory = sub_match.group(1).strip()
            subcategory_description = sub_match.group(2).strip() if sub_match.group(2) else ""
            if current_subcategory not in structured_config[current_category]['subcategories']:
                structured_config[current_category]['subcategories'][current_subcategory] = {
                    'description': subcategory_description,
                    'settings': []
                }
        elif env_match and current_category and current_subcategory:
            key = env_match.group(1)
            value_part = line[len(key)+1:].strip()

            # 判断是单行还是多行的开始
            # 计算值部分中未被转义的引号数量
            unescaped_quotes = len(re.findall(r'(?<!\\)"', value_part.split('#')[0]))

            if value_part.startswith('"') and unescaped_quotes % 2 != 0:
                # 值的开头是引号，且引号数量为奇数，说明是多行块的开始
                in_multiline_block = True
                current_key = key
                multiline_buffer = value_part
            else:
                # 这是一个完整的单行值
                setting = process_setting(key, value_part)
                structured_config[current_category]['subcategories'][current_subcategory]['settings'].append(setting)

        elif category_end_re.match(line_strip):
            current_category = None
        elif subcategory_end_re.match(line_strip):
            current_subcategory = None

    return structured_config


def save_env_file(new_values: Dict[str, str]):
    """
    一个基于状态机的、健壮的.env文件保存函数。
    此版本根据要求，为多行字符串在引号内增加了前后的换行符。
    """

    with open(env_file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    updated_lines = []
    i = 0
    while i < len(lines):
        line = lines[i]
        match = re.match(r'^([A-Z_0-9]+)=', line)

        if not match:
            updated_lines.append(line)
            i += 1
            continue

        key = match.group(1)

        block_lines = [line]
        value_part_raw = line[len(key)+1:]
        unescaped_quotes = len(re.findall(r'(?<!\\)"', value_part_raw.split('#')[0]))

        if value_part_raw.strip().startswith('"') and unescaped_quotes % 2 != 0:
            j = i + 1
            while j < len(lines):
                block_lines.append(lines[j])
                if '"' in lines[j].split('#')[0]:
                    break
                j += 1
            i = j

        if key in new_values:
            new_val = new_values[key]

            original_comment = ""
            last_line_of_block = block_lines[-1]
            if '#' in last_line_of_block:
                comment_part = last_line_of_block.split('#', 1)[1]
                original_comment = " #" + comment_part.rstrip()

            # --- 这是本次唯一的、核心的逻辑修改 ---
            if str(new_val).lower() in ['true', 'false'] or new_val.isdigit():
                # 对于布尔值或数字，直接写入
                updated_lines.append(f"{key}={new_val}{original_comment}\n")
            else:
                # 对于字符串值，进行判断
                if '\n' in new_val:
                    # **多行字符串**: 在值的内外添加换行符
                    updated_lines.append(f'{key}="\n{new_val}\n"{original_comment}\n')
                else:
                    # **单行字符串**: 按原样标准格式写入
                    updated_lines.append(f'{key}="{new_val}"{original_comment}\n')
        else:
            updated_lines.extend(block_lines)

        i += 1

    with open(env_file_path, 'w', encoding='utf-8') as f:
        f.writelines(updated_lines)

@router.get("/api/settings/config", response_model=Dict[str, Any])
async def get_config():
    try:
        config = parse_env_file()
        return config
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"An unexpected error occurred while parsing .env file: {str(e)}")

@router.post("/api/settings/config")
async def save_config(new_values: Dict[str, str] = Body(...)):
    try:
        save_env_file(new_values)
        # 看这里！！保存文件后，进行运行时热更新
        apply_runtime_config_changes(new_values)
        return {"status": "success", "message": "配置已成功保存并已生效！"}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to save .env file: {str(e)}")
