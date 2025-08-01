# file: desktop_pet.py

import sys
import os
from dotenv import load_dotenv

# --- GUI ---
from PyQt6.QtWidgets import QApplication, QMessageBox

# --- Local Modules ---
from config import CONFIG, ENV_PATH, SCRIPT_DIR, BACKEND_DIR, PROJECT_ROOT
from logger import Logger
from emotion_classifier import EmotionClassifier
from pet_window import DesktopPet # Import the main window from its new file

# ==============================================================================
# ---                           程序主入口                          ---
# ==============================================================================
if __name__ == '__main__':
    # 确保System Prompt包含格式要求
    if "【情绪】" not in CONFIG["SYSTEM_PROMPT"]:
         CONFIG["SYSTEM_PROMPT"] += " 你的回复必须遵循格式：【情绪】中文回复<日文翻译>（动作描述）。一段回复中可以包含多个这样的格式。"

    # 加载环境变量
    load_dotenv(dotenv_path=ENV_PATH)
    
    # 从环境变量覆盖默认配置
    CONFIG["CHAT_API_KEY"] = os.environ.get("CHAT_API_KEY", CONFIG["CHAT_API_KEY"])
    CONFIG["CHAT_BASE_URL"] = os.environ.get("CHAT_BASE_URL", CONFIG["CHAT_BASE_URL"])
    CONFIG["VD_API_KEY"] = os.environ.get("VD_API_KEY", CONFIG["VD_API_KEY"])
    CONFIG["VD_BASE_URL"] = os.environ.get("VD_BASE_URL", CONFIG["VD_BASE_URL"])
    CONFIG["VD_MODEL"] = os.environ.get("VD_MODEL", CONFIG["VD_MODEL"])
    CONFIG["SYSTEM_PROMPT"] = os.environ.get("SYSTEM_PROMPT", CONFIG["SYSTEM_PROMPT"])
    
    # 创建必要的目录
    CONFIG["LOG_DIRECTORY"].mkdir(parents=True, exist_ok=True)
    CONFIG["SCREENSHOT_DIRECTORY"].mkdir(parents=True, exist_ok=True)

    # 初始化日志记录器
    logger = Logger(log_dir=CONFIG["LOG_DIRECTORY"])
    logger.info("================== 桌面宠物启动 ==================")
    logger.info(f"脚本目录: {SCRIPT_DIR}")
    logger.info(f"后端目录: {BACKEND_DIR}")
    logger.info(f"项目根目录: {PROJECT_ROOT}")
    logger.info(f".env 文件路径: {ENV_PATH} (存在: {ENV_PATH.exists()})")
    logger.info(f"情绪模型路径: {CONFIG['EMOTION_MODEL_PATH']} (存在: {CONFIG['EMOTION_MODEL_PATH'].exists()})")
    logger.info(f"角色图片路径: {CONFIG['CHARACTER_IMAGE_PATH'] / CONFIG['CHARACTER_NAME']} (存在: {(CONFIG['CHARACTER_IMAGE_PATH'] / CONFIG['CHARACTER_NAME']).exists()})")
    logger.info(f"角色: {CONFIG['CHARACTER_NAME']}")
    logger.info(f"聊天API Key 已{'加载' if CONFIG['CHAT_API_KEY'] and CONFIG['CHAT_API_KEY'] != 'sk-111' else '未加载或使用默认占位符'}")
    logger.info(f"视觉API Key 已{'加载' if CONFIG['VD_API_KEY'] and CONFIG['VD_API_KEY'] != 'sk-111' else '未加载或使用默认占位符'}")

    # 启动Qt应用
    app = QApplication(sys.argv)
    
    try:
        # 1. 初始化后端组件
        classifier = EmotionClassifier(CONFIG["EMOTION_MODEL_PATH"], logger)
        
        # 2. 初始化主窗口，并传入依赖项
        pet = DesktopPet(logger_instance=logger, classifier_instance=classifier)
        
        # 3. 显示窗口并启动
        pet.show()
        sys.exit(app.exec())

    except Exception as e:
        # 捕获启动过程中的任何致命错误
        logger.error(f"程序启动时发生致命错误: {e}", exc_info=True)
        # 提供GUI错误提示
        msg_box = QMessageBox()
        msg_box.setIcon(QMessageBox.Icon.Critical)
        msg_box.setText("程序启动失败！")
        msg_box.setInformativeText(f"发生了一个严重错误，请检查日志文件获取详细信息。\n\n错误: {e}")
        msg_box.setWindowTitle("致命错误")
        msg_box.exec()
        sys.exit(1)