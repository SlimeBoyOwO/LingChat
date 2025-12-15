import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# 加载环境变量
env_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".env")
try:
    from dotenv import load_dotenv
    load_dotenv(env_path)
except Exception:
    try:
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" in line:
                    k, v = line.split("=", 1)
                    v = v.strip().strip('"').strip("'")
                    os.environ.setdefault(k.strip(), v)
    except FileNotFoundError:
        pass

def run_application():
    """运行主应用程序"""
    print("主应用开始运行 http://localhost:8765")
    from ling_chat import main
    main.main()

if __name__ == "__main__":
    run_application()