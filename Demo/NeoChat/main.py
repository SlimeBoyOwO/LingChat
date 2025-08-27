# main.py
import os
import sys

# 将项目根目录添加到 sys.path，以便能正确找到 neochat 包
sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

# 导入必要的模块
from neochat.platform.configuration import config
from neochat.platform.logging import initialize_logger, log_error, log_info_color, TermColors
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
        # 确保 user_data 目录存在，因为 user_config.json 会放在那里
        os.makedirs(os.path.dirname(config.paths.user_config), exist_ok=True)
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
    # config.debug.mode 已经从 config.yaml 加载，确保其正确
    initialize_logger(app_name="NeoChat", config_debug_mode=config.debug.mode)
    log_info_color("NeoChat 启动成功！", TermColors.GREEN)

    # 3. 检查API Key配置 (修复了AttributeError)
    try:
        # 获取当前激活的LLM提供商的名称
        active_provider_name = config.llm.active_provider
        # 获取该提供商的配置对象
        active_provider_config = getattr(config.llm.providers, active_provider_name)
        # 从该提供商的配置中获取api_key
        api_key_value = active_provider_config.api_key

        if not api_key_value or "YOUR" in str(api_key_value):
            log_error("错误: 请在项目根目录的 '.env' 文件中设置你的 API_KEY。")
            log_error("例如: API_KEY=\"sk-xxxxxxxxxxxxxxxxxxxxxxxx\"")
            input("按回车键退出...")
            return
    except AttributeError as e:
        log_error(f"配置错误: 无法获取 LLM API 密钥。请检查 config.yaml 中 'llm.active_provider' 和对应提供商的配置是否正确。错误详情: {e}")
        log_error("例如，确保 config.yaml 中有类似以下结构：")
        log_error("llm:")
        log_error("  active_provider: deepseek")
        log_error("  providers:")
        log_error("    deepseek:")
        log_error("      api_key: ${API_KEY}")
        input("按回车键退出...")
        return
    except Exception as e:
        log_error(f"初始化LLM配置时发生未知错误: {e}")
        input("按回车键退出...")
        return

    # 4. 创建所需目录
    setup_environment()

    # 5. 启动主菜单
    menu = MainMenu()
    menu.run()

if __name__ == "__main__":
    main()