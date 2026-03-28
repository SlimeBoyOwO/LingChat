import os
import re
from typing import Dict, List, Tuple


def parse_env_file(file_path: str) -> Dict[str, Tuple[str, List[str]]]:
    """
    解析 .env 文件，返回一个字典，包含键值对以及每个键的注释和所在分区
    """
    env_vars = {}
    current_section_comments = []
    
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        
        # 如果是注释行或空行，保存它
        if line.startswith('#') or not line:
            current_section_comments.append(line)
        else:
            # 查找键值对
            match = re.match(r'^([A-Z_][A-Z0-9_]*)\s*=.*$', line)
            if match:
                key = match.group(1)
                
                # 找到键后，收集该键的注释（包括之前的注释和本行的注释）
                key_comments = []
                j = len(current_section_comments) - 1
                while j >= 0:
                    comment = current_section_comments[j]
                    if comment.startswith('#'):
                        key_comments.insert(0, comment)
                        j -= 1
                    else:
                        break
                
                env_vars[key] = (line, key_comments)
                current_section_comments = []  # 清空分区注释
            else:
                # 如果不是键值对也不是注释，可能是其他内容，加入分区注释
                current_section_comments.append(line)
        
        i += 1
    
    return env_vars


def find_section_boundaries(example_lines: List[str]) -> Dict[str, Tuple[int, int]]:
    """
    找到每个分区的边界（开始和结束行号）
    """
    sections = {}
    current_section = None
    section_start = 0
    
    for idx, line in enumerate(example_lines):
        if 'BEGIN' in line and (line.startswith('# ') or line.startswith('## ')):
            # 找到新分区开始，保存上一个分区
            if current_section:
                sections[current_section] = (section_start, idx - 1)
            
            # 提取分区名称
            section_match = re.search(r'##?\s*(.*?)\s+BEGIN', line)
            if section_match:
                current_section = section_match.group(1)
                section_start = idx
        
        elif 'END' in line and '##' in line and current_section:
            # 找到分区结束
            sections[current_section] = (section_start, idx)
            current_section = None
    
    # 处理最后一个分区
    if current_section:
        sections[current_section] = (section_start, len(example_lines) - 1)
    
    return sections


def extract_keys_from_section(lines: List[str], start: int, end: int) -> List[str]:
    """
    从指定分区提取所有键
    """
    keys = []
    for i in range(start, end + 1):
        line = lines[i].strip()
        if not line or line.startswith('#'):
            continue
            
        match = re.match(r'^([A-Z_][A-Z0-9_]*)\s*=.*$', line)
        if match:
            keys.append(match.group(1))
    
    return keys


def copy_missing_keys(env_path: str, example_path: str):
    """
    比较 env 和 example 文件，将缺失的键从 example 复制到 env
    """
    # 解析两个文件
    env_vars = parse_env_file(env_path)
    example_vars = parse_env_file(example_path)
    
    # 读取 example 文件的所有行，用于查找分区
    with open(example_path, 'r', encoding='utf-8') as f:
        example_lines = f.readlines()
    
    # 找到所有分区的边界
    section_boundaries = find_section_boundaries(example_lines)
    
    # 找出缺失的键
    missing_keys = {key: value for key, value in example_vars.items() if key not in env_vars}
    
    if not missing_keys:
        print("没有发现缺失的键")
        return
    
    # 按分区组织缺失的键
    sections_with_missing_keys = {}
    for key, (line, comments) in missing_keys.items():
        # 找到这个键在 example 文件中属于哪个分区
        for section_name, (start, end) in section_boundaries.items():
            section_keys = extract_keys_from_section(example_lines, start, end)
            if key in section_keys:
                if section_name not in sections_with_missing_keys:
                    sections_with_missing_keys[section_name] = []
                sections_with_missing_keys[section_name].append((key, line, comments))
                break
    
    # 读取当前 env 文件内容
    with open(env_path, 'r', encoding='utf-8') as f:
        env_lines = f.readlines()
    
    # 为每个分区添加缺失的键
    for section_name, keys_list in sections_with_missing_keys.items():
        if section_name in section_boundaries:
            start, end = section_boundaries[section_name]
            
            # 获取该分区在 env 文件中的范围
            env_section_start = -1
            env_section_end = -1
            
            # 寻找对应分区在 env 文件中的位置
            for i, line in enumerate(env_lines):
                if section_name in line and 'BEGIN' in line and (line.startswith('# ') or line.startswith('## ')):
                    env_section_start = i
                elif section_name in line and 'END' in line and '##' in line and env_section_start != -1:
                    env_section_end = i
                    break
            
            if env_section_start != -1 and env_section_end != -1:
                # 在分区末尾（但在 END 注释之前）插入缺失的键
                insert_position = env_section_end
                
                # 添加缺失的键及其注释
                for key, line, comments in keys_list:
                    print(f"正在添加缺失的键: {key}")
                    
                    # 添加注释
                    for comment in comments:
                        env_lines.insert(insert_position, comment + '\n')
                        insert_position += 1
                    
                    # 添加键值对
                    env_lines.insert(insert_position, line + '\n')
                    insert_position += 1
                
                # 添加一个空行分隔
                if len(keys_list) > 0:
                    env_lines.insert(insert_position, '\n')
    
    # 写回文件
    with open(env_path, 'w', encoding='utf-8') as f:
        f.writelines(env_lines)
    
    print(f"完成！已将 {len(missing_keys)} 个缺失的键添加到 {env_path}")


def main():
    """
    主函数，比较当前目录下的 .env 和 .env.example 文件
    """
    project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    env_path = os.path.join(project_root, '.env')
    example_path = os.path.join(project_root, '.env.example')
    
    if not os.path.exists(env_path):
        print(f"错误: 无法找到 {env_path}")
        return
    
    if not os.path.exists(example_path):
        print(f"错误: 无法找到 {example_path}")
        return
    
    print(f"比较 {env_path} 和 {example_path}")
    copy_missing_keys(env_path, example_path)


if __name__ == "__main__":
    main()