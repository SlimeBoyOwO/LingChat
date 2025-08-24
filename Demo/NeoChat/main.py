# main.py
import os
import sys

# 将项目根目录添加到 sys.path，以便能正确找到 neochat 包
sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

# 导入必要的模块
from neochat.platform.configuration import config
from neochat.platform.logging import initialize_logger, log_error
from neochat.presentation.cli.animation import run_boot_animation
from neochat.presentation.cli.menu import MainMenu

def setup_environment():
    """确保所有必需的目录都存在。"""
    try:
        os.makedirs(config.paths.saves, exist_ok=True)
        os.makedirs(config.paths.story_packs, exist_ok=True)
        os.makedirs(config.paths.characters, exist_ok=True)
        os.makedirs(config.paths.player_characters, exist_ok=True)
        os.makedirs(config.paths.logs, exist_ok=True)
        # 确保 data 目录和其 __init__.py 存在
        os.makedirs('data', exist_ok=True)
        if not os.path.exists('data/__init__.py'):
            with open('data/__init__.py', 'w') as f:
                pass
    except OSError as e:
        log_error(f"创建目录结构时出错: {e}")
        sys.exit(1)

def main():
    """程序主入口点。"""
    # 1. 运行启动动画
    run_boot_animation()

    # 2. 初始化日志系统
    initialize_logger(app_name="NeoChat", config_debug_mode=config.debug.mode)

    # 3. 检查API Key配置
    if not config.llm.api_key or "YOUR" in str(config.llm.api_key):
        log_error("错误: 请在项目根目录的 '.env' 文件中设置你的 API_KEY。")
        log_error("例如: API_KEY=\"sk-xxxxxxxxxxxxxxxxxxxxxxxx\"")
        input("按回车键退出...")
        return

    # 4. 创建所需目录
    setup_environment()

    # 5. 启动主菜单
    menu = MainMenu()
    menu.run()

if __name__ == "__main__":
    main()