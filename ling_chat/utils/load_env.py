import os
import shutil
from pathlib import Path
from typing import Dict


def load_env(env_path: Path|str = ".env") -> Dict[str, str]:
    """
    读取 .env 文件中的所有环境变量并导出到当前环境中
    
    Args:
        env_path (str): .env 文件路径，默认为 ".env"
    
    Returns:
        Dict[str, str]: 解析出的环境变量字典
    """
    env_vars: Dict[str, str] = {}
    try:
        if (not os.path.exists(env_path)):
            example_path = ".env.example"
            print(f"\033[91m[WARN]\033[0m找不到环境变量文件: {env_path}")
            shutil.copy2(example_path, env_path)
            print(f"\033[92m[INFO]\033[0m从 {example_path} 复制文件到 {env_path}")
        if  os.path.exists(env_path):
            print(f"\033[92m[INFO]\033[0m正在加载环境变量文件: {env_path}")

        with open(env_path, 'r', encoding='utf-8') as file:
            lines = file.readlines()

        # 用于跟踪多行值解析的状态
        in_multiline = False
        multiline_key: str = ""
        multiline_value = ""

        i = 0
        while i < len(lines):
            line = lines[i].strip()

            # 处理多行值状态
            if in_multiline:
                # 检查是否是多行结束标记
                if '"""' in line:
                    # 结束多行解析
                    end_pos = line.find('"""')
                    multiline_value += line[:end_pos]

                    # 设置环境变量
                    multiline_value = multiline_value.strip()
                    os.environ[multiline_key] = multiline_value
                    env_vars[multiline_key] = multiline_value

                    # 重置多行状态
                    in_multiline = False
                    multiline_key = ""
                    multiline_value = ""
                else:
                    # 继续收集多行值
                    multiline_value += lines[i]  # 保持原始换行符
                i += 1
                continue

            # 跳过空行和注释行
            if not line or line.startswith('#'):
                i += 1
                continue

            # 跳过标记行（如 BEGIN/END）
            if 'BEGIN' in line or 'END' in line:
                if line.startswith('#'):
                    i += 1
                    continue

            # 分割键值对
            if '=' in line:
                key, value = line.split('=', 1)
                key = key.strip()
                value = value.strip()

                # 检查是否是多行值开始
                if value.startswith('"""'):
                    # 检查是否在同一行结束
                    start_pos = 3  # 跳过开始的 """
                    if '"""' in value[start_pos:]:
                        # 同行结束
                        end_pos = value.find('"""', start_pos)
                        multiline_value = value[start_pos:end_pos]
                        os.environ[key] = multiline_value
                        env_vars[key] = multiline_value
                    else:
                        # 多行开始
                        in_multiline = True
                        multiline_key = key
                        multiline_value = value[start_pos:] + ("\n" if not value[start_pos:].endswith("\n") else "")
                else:
                    # 单行值处理
                    # 去除值两端的引号
                    if value.startswith('"') and value.endswith('"'):
                        value = value[1:-1]
                    elif value.startswith("'") and value.endswith("'"):
                        value = value[1:-1]

                    # 去除值后面可能存在的注释
                    if '#' in value:
                        value = value.split('#', 1)[0].rstrip()

                    # 再次去除可能存在的引号（处理注释后的情况）
                    value = value.strip()
                    if value.startswith('"') and value.endswith('"'):
                        value = value[1:-1]
                    elif value.startswith("'") and value.endswith("'"):
                        value = value[1:-1]

                    # 设置环境变量
                    os.environ[key] = value
                    env_vars[key] = value

            i += 1

        return env_vars
    except Exception as e:
        print(f"[ERROR]{env_path}文件加载失败: {e}")
        return env_vars
