# file: config.py

import os
from pathlib import Path

# --- 核心路径计算 (高鲁棒性) ---
# 1. 获取此配置文件所在的目录，这是最可靠的锚点。
# SCRIPT_DIR 会是 .../desktop_pet
try:
    # __file__ refers to config.py, so .parent gives the desktop_pet directory.
    SCRIPT_DIR = Path(__file__).resolve().parent
except NameError:
    # Fallback for environments where __file__ is not defined.
    print("警告: 无法通过 __file__ 确定脚本路径，将使用当前工作目录。这可能导致路径错误。")
    SCRIPT_DIR = Path.cwd()

# 2. 从脚本目录推导项目中的其他关键目录。
BACKEND_DIR = SCRIPT_DIR.parent
PROJECT_ROOT = BACKEND_DIR.parent
ENV_PATH = PROJECT_ROOT / ".env"

# --- 配置字典 ---
CONFIG = {
    # --- 聊天API 配置 (将从 .env 加载) ---
    "CHAT_API_KEY": "sk-114514",
    "CHAT_BASE_URL": "https://api.deepseek.com",
    "CHAT_MODEL": "deepseek-chat",

    # --- 视觉API 配置 (将从 .env 加载) ---
    "VD_API_KEY": "sk-114514",
    "VD_BASE_URL": "https://api.siliconflow.cn/v1",
    "VD_MODEL": "Pro/Qwen/Qwen2.5-VL-7B-Instruct",

    # --- 角色与提示词 (将从 .env 加载) ---
    "SYSTEM_PROMPT": "你是一个名为'灵灵'的AI助手，请用可爱、简洁、口语化的方式回答问题。你的回复必须遵循格式：【情绪】中文回复<日文翻译>（动作描述）。一段回复中可以包含多个这样的格式。",
    "SCREENSHOT_PROMPT": "你是一个图像描述专家。请用简洁的语言客观地描述这张图片的核心内容。你的描述将作为输入，由另一个AI来回答。请不要进行任何评价或联想，只描述你看到了什么。",

    # --- 文件路径配置 (基于上面计算出的路径) ---
    "CHARACTER_NAME": "诺一钦灵/avatar",
    "CHARACTER_IMAGE_PATH": PROJECT_ROOT / "game_data" / "characters",
    "EMOTION_MODEL_PATH": BACKEND_DIR / "emotion_model_18emo",
    "LOG_DIRECTORY": SCRIPT_DIR / "logs",
    "SCREENSHOT_DIRECTORY": SCRIPT_DIR / "screenshots",

    # --- 默认状态 (确保有对应的 .png 图片) ---
    "DEFAULT_EMOTION": "正常",
    "THINKING_EMOTION": "思考",
    "ERROR_EMOTION": "困惑",
}
