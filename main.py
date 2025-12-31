import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# 加载环境变量
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.utils.load_env import load_env

if os.path.exists(".env"):
    load_env()
else:
    try:
        load_env(".env.example")
        load_env(user_data_path / ".env")  # 加载用户数据目录下的环境变量
    except Exception as e:
        print(f"\033[91m[警告]\033[0m：加载环境变量失败，将使用默认。\n 错误为: {e}")

if __name__ == "__main__":
    from ling_chat import main
    main.main()