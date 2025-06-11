"""
LingChat - 自包含桌面宠物模块 (项目集成版)

功能:
- 一个独立的、可拖动的桌面宠物窗口。
- 与 DeepSeek 等兼容 OpenAI 格式的 LLM 进行对话。
- 从项目根目录的 .env 文件加载 CHAT_API_KEY 和 SYSTEM_PROMPT。
- 调用项目内的本地情绪分类模型(`backend/emotion_model_18emo`)来分析 LLM 的回复。
- 根据分析出的情绪，切换项目内 `frontend/public/pictures/qinling/` 目录下的角色立绘。
- 所有日志记录在 `backend/desktop_pet/logs` 文件夹中。
- 自动计算并使用项目内的相对路径，无需手动配置。

运行方式:
1. 确保已安装所有依赖: pip install PyQt6 openai python-dotenv torch transformers
2. 确保项目根目录 .env 文件已配置 CHAT_API_KEY 和 SYSTEM_PROMPT。
3. 从项目根目录(develop/)运行此脚本: python backend/desktop_pet/desktop_pet.py
"""
import sys
import os
import json
import logging
import re  # <--- MODIFICATION START ---> (导入re模块)
from datetime import datetime
from pathlib import Path

# --- GUI ---
from PyQt6.QtWidgets import QApplication, QWidget, QLabel, QVBoxLayout, QLineEdit
from PyQt6.QtGui import QPixmap, QMouseEvent, QFont, QIcon
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QPoint

# --- AI & 模型 ---
from openai import OpenAI
from dotenv import load_dotenv
import torch
from transformers import BertTokenizer, BertForSequenceClassification

# ==============================================================================
# ---                           配置区域                           ---
# ==============================================================================

# 自动计算项目路径
SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent

# 初始配置，部分值将在主程序入口处被 .env 文件覆盖
CONFIG = {
    # --- API 配置 (将从 .env 加载) ---
    "API_KEY": None,
    "BASE_URL": "https://api.deepseek.com",
    "MODEL": "deepseek-chat",

    # --- 角色与提示词 (将从 .env 加载) ---
    "SYSTEM_PROMPT": "你是一个名为'灵灵'的AI助手，请用可爱、简洁、口语化的方式回答问题。你的回复必须遵循格式：【情绪】中文回复<日文翻译>（动作描述）。一段回复中可以包含多个这样的格式。",

    # --- 文件路径配置 (自动计算) ---
    "CHARACTER_NAME": "qinling",
    "CHARACTER_IMAGE_PATH": PROJECT_ROOT / "frontend" / "public" / "pictures",
    "EMOTION_MODEL_PATH": SCRIPT_DIR.parent / "emotion_model_18emo",
    "LOG_DIRECTORY": SCRIPT_DIR / "logs",

    # --- 默认状态 ---
    "DEFAULT_EMOTION": "正常", # 对应 正常.png
    "THINKING_EMOTION": "思考",
    "ERROR_EMOTION": "困惑",
}

# ==============================================================================
# ---                        独立的日志记录模块                        ---
# ==============================================================================
class Logger:
    _instance = None
    def __new__(cls, *args, **kwargs):
        if not cls._instance:
            cls._instance = super(Logger, cls).__new__(cls)
        return cls._instance

    def __init__(self, log_dir="logs", level=logging.INFO):
        if hasattr(self, '_initialized') and self._initialized:
            return
        
        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.logger = logging.getLogger("DesktopPetLogger")
        self.logger.setLevel(level)
        
        for handler in self.logger.handlers[:]:
            self.logger.removeHandler(handler)

        formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
        
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setFormatter(formatter)
        self.logger.addHandler(console_handler)

        log_file = self.log_dir / f"pet_{datetime.now().strftime('%Y-%m-%d')}.log"
        file_handler = logging.FileHandler(log_file, encoding='utf-8')
        file_handler.setFormatter(formatter)
        self.logger.addHandler(file_handler)
        
        self._initialized = True

    def info(self, msg): self.logger.info(msg)
    def warning(self, msg): self.logger.warning(msg)
    def error(self, msg, exc_info=False): self.logger.error(msg, exc_info=exc_info)
    def debug(self, msg): self.logger.debug(msg)

# ==============================================================================
# ---                    独立的情绪分类模型模块                    ---
# ==============================================================================
class EmotionClassifier:
    def __init__(self, model_path, logger_instance):
        self.logger = logger_instance
        self.model = None
        self.tokenizer = None
        self.id2label = {}

        try:
            if not model_path.exists():
                raise FileNotFoundError(f"情绪模型路径不存在: {model_path}")

            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
            self.logger.info(f"情绪分类模型将使用设备: {self.device}")

            self.tokenizer = BertTokenizer.from_pretrained(model_path, local_files_only=True)
            self.model = BertForSequenceClassification.from_pretrained(model_path, local_files_only=True)
            self.model.to(self.device)
            self.model.eval()

            mapping_path = model_path / "label_mapping.json"
            if not mapping_path.exists():
                 raise FileNotFoundError(f"找不到标签映射文件: {mapping_path}")
            
            with open(mapping_path, 'r', encoding='utf-8') as f:
                label_config = json.load(f)
            self.id2label = label_config.get("id2label", {})
            if not self.id2label:
                raise ValueError("label_mapping.json 格式不正确或为空")
            
            self.logger.info(f"成功加载情绪分类模型: {model_path.name}")
        except Exception as e:
            self.logger.error(f"加载情绪分类模型失败: {e}")
            self.model = None

    def predict(self, text):
        if not self.model:
            self.logger.warning("情绪模型未加载，无法进行预测。返回默认情绪。")
            return CONFIG["DEFAULT_EMOTION"]

        try:
            inputs = self.tokenizer(
                text,
                truncation=True,
                max_length=128,
                return_tensors="pt"
            ).to(self.device)
            
            with torch.no_grad():
                outputs = self.model(**inputs)
                logits = outputs.logits
                pred_id = torch.argmax(logits, dim=1).item()
            
            predicted_label = self.id2label.get(str(pred_id), CONFIG["DEFAULT_EMOTION"])
            self.logger.info(f"情绪预测: '{text[:30]}...' -> '{predicted_label}'")
            return predicted_label

        except Exception as e:
            self.logger.error(f"情绪预测时发生错误: {e}")
            return CONFIG["ERROR_EMOTION"]

# ==============================================================================
# ---                      与API交互的后台线程                      ---
# ==============================================================================
class ChatWorker(QThread):
    response_ready = pyqtSignal(str)
    error_occurred = pyqtSignal(str)

    def __init__(self, messages):
        super().__init__()
        self.messages = messages
        self.client = OpenAI(
            api_key=CONFIG["API_KEY"],
            base_url=CONFIG["BASE_URL"],
            default_headers={"Content-Type": "application/json; charset=utf-8"}
        )

    def run(self):
        try:
            response = self.client.chat.completions.create(
                model=CONFIG["MODEL"],
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.response_ready.emit(ai_response)
        except Exception as e:
            logger.error(f"API请求出错: {e}")
            self.error_occurred.emit(f"呜...网络出错了，请检查API Key和网络连接。")

# ==============================================================================
# ---                           主窗口 (桌宠)                         ---
# ==============================================================================
class DesktopPet(QWidget):
    def __init__(self, logger_instance, classifier_instance):
        super().__init__()
        self.logger = logger_instance
        self.classifier = classifier_instance

        self.conversation_history = [{"role": "system", "content": CONFIG["SYSTEM_PROMPT"]}]
        
        # <--- MODIFICATION START --->
        # 新增状态变量，用于管理多段回复
        self.current_response_segments = []
        self.current_segment_index = 0
        # <--- MODIFICATION END --->

        if not CONFIG["API_KEY"]:
            self.logger.error("错误：API Key未在 .env 文件中设置！请检查项目根目录的 .env 文件并添加 'CHAT_API_KEY=...'。")
            self.initial_message = "API Key未设置，我无法思考哦！"
        else:
            self.initial_message = "你好呀！有什么可以帮你的吗？"

        self.emotion_pixmaps = self.load_character_images()
        if not self.emotion_pixmaps:
            self.setup_ui(error_mode=True)
            self.show_bubble("角色图片资源加载失败，请检查路径和文件。")
            return
        
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.WindowStaysOnTopHint)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setWindowTitle("LingChat Pet")
        if CONFIG["DEFAULT_EMOTION"] in self.emotion_pixmaps:
             self.setWindowIcon(QIcon(self.emotion_pixmaps[CONFIG["DEFAULT_EMOTION"]]))

        self.setup_ui()
        self.update_pet_emotion(CONFIG["DEFAULT_EMOTION"])
        self.drag_position = QPoint()

    def load_character_images(self):
        character_path = CONFIG["CHARACTER_IMAGE_PATH"] / CONFIG["CHARACTER_NAME"]
        pixmaps = {}
        if not character_path.is_dir():
            self.logger.error(f"错误：角色图片文件夹 '{character_path}' 不存在！")
            return None

        for file in character_path.glob("*.png"):
            emotion_name = file.stem
            pixmaps[emotion_name] = QPixmap(str(file))
        
        if not pixmaps:
            self.logger.error(f"错误：角色图片文件夹 '{character_path}' 为空！")
            return None
        
        if CONFIG["DEFAULT_EMOTION"] not in pixmaps:
            self.logger.error(f"错误：找不到默认情绪图片 '{CONFIG['DEFAULT_EMOTION']}.png'！")
            return None

        self.logger.info(f"成功为角色'{CONFIG['CHARACTER_NAME']}'加载 {len(pixmaps)} 张情绪图片。")
        return pixmaps

    def setup_ui(self, error_mode=False):
        self.layout = QVBoxLayout(self)
        self.layout.setContentsMargins(15, 15, 15, 15)

        self.chat_bubble = QLabel(self.initial_message, self)
        self.chat_bubble.setFont(QFont("Microsoft YaHei", 10))
        self.chat_bubble.setStyleSheet("background-color: rgba(255, 255, 255, 0.9); border-radius: 15px; padding: 10px; color: black; border: 1px solid rgba(0, 0, 0, 0.1);")
        self.chat_bubble.setWordWrap(True)
        self.chat_bubble.hide()

        self.input_box = QLineEdit(self)
        self.input_box.setPlaceholderText("在这里输入后按Enter...")
        self.input_box.setFont(QFont("Microsoft YaHei", 9))
        self.input_box.setStyleSheet("background-color: rgba(245, 245, 245, 0.95); border: 1px solid #ccc; border-radius: 10px; padding: 5px 8px;")
        self.input_box.returnPressed.connect(self.handle_input) # <--- MODIFICATION: 连接到新的处理函数

        self.pet_label = QLabel(self)
        self.pet_label.setScaledContents(True)
        if not error_mode:
            default_pixmap = self.emotion_pixmaps[CONFIG["DEFAULT_EMOTION"]]
            self.pet_label.setFixedSize(int(default_pixmap.width() / 3), int(default_pixmap.height() / 3))
        else:
             self.pet_label.setFixedSize(128, 128)

        self.layout.addWidget(self.chat_bubble)
        self.layout.addWidget(self.pet_label, 0, Qt.AlignmentFlag.AlignCenter)
        self.layout.addWidget(self.input_box)
        self.adjustSize()

    # <--- MODIFICATION START --->
    def _parse_ai_response(self, text):
        """
        解析AI的完整回复，将其拆分为多个片段。
        此逻辑移植自 core/ai_service.py
        """
        segments = []
        # 正则表达式，用于匹配 【情绪标签】中文<日文>（动作） 这样的格式
        pattern = re.findall(r'(【(.*?)】)([^【】]*)', text)
        
        for i, (full_tag, emotion_tag, following_text) in enumerate(pattern):
            # 提取中文部分（移除日文和动作）
            chinese_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()
            
            # 只有当中文部分不为空时，才认为是一个有效的片段
            if chinese_text:
                segments.append({
                    "original_tag": emotion_tag.strip(),
                    "chinese_text": chinese_text,
                })
        return segments

    def handle_input(self):
        """
        处理用户在输入框按下的回车键。
        - 如果正在显示多段回复，则显示下一段。
        - 否则，发起新的对话。
        """
        # 检查是否正在显示多段回复
        if self.current_response_segments:
            self.display_next_segment()
        else:
            self.start_new_chat()

    def display_next_segment(self):
        """显示多段回复中的下一个片段，或在结束后重置状态。"""
        self.current_segment_index += 1
        
        # 检查是否还有更多片段
        if self.current_segment_index < len(self.current_response_segments):
            segment = self.current_response_segments[self.current_segment_index]
            self.show_bubble(segment['chinese_text'])
        else:
            # 所有片段都已显示完毕，重置状态
            self.chat_bubble.hide()
            self.current_response_segments = []
            self.current_segment_index = 0
            self.update_pet_emotion(CONFIG["DEFAULT_EMOTION"])
            self.input_box.setPlaceholderText("在这里输入后按Enter...")

    def start_new_chat(self):
        """获取用户输入并启动新的聊天线程。"""
        user_input = self.input_box.text().strip()
        if not user_input or not CONFIG["API_KEY"]:
            return

        self.logger.info(f"用户输入: {user_input}")
        self.conversation_history.append({"role": "user", "content": user_input})
        self.input_box.clear()
        
        self.show_bubble("思考中...")
        self.update_pet_emotion(CONFIG["THINKING_EMOTION"])
        
        self.input_box.setPlaceholderText("AI正在思考中...")

        self.worker = ChatWorker(self.conversation_history)
        self.worker.response_ready.connect(self.handle_response)
        self.worker.error_occurred.connect(self.handle_error)
        self.worker.start()
    # <--- MODIFICATION END --->

    def update_pet_emotion(self, emotion_name):
        if emotion_name in self.emotion_pixmaps:
            self.pet_label.setPixmap(self.emotion_pixmaps[emotion_name])
        else:
            self.logger.warning(f"找不到情绪 '{emotion_name}' 的图片，使用默认图片 '{CONFIG['DEFAULT_EMOTION']}'。")
            self.pet_label.setPixmap(self.emotion_pixmaps[CONFIG["DEFAULT_EMOTION"]])

    def handle_response(self, response_text):
        self.logger.info(f"AI原始回复: {response_text}")
        self.conversation_history.append({"role": "assistant", "content": response_text})

        # <--- MODIFICATION START --->
        # 解析回复
        segments = self._parse_ai_response(response_text)

        self.input_box.setPlaceholderText("在这里输入后按Enter...")

        if segments:
            # 如果是多段式回复
            self.current_response_segments = segments
            self.current_segment_index = 0
            
            # 根据第一个片段的情绪来设置立绘
            first_emotion_tag = segments[0]['original_tag']
            predicted_emotion = self.classifier.predict(first_emotion_tag)
            self.update_pet_emotion(predicted_emotion)
            
            # 显示第一个片段的中文文本
            self.show_bubble(segments[0]['chinese_text'])
            
            # 更新输入框提示，告知用户可以按回车继续
            if len(segments) > 1:
                self.input_box.setPlaceholderText("按Enter查看下一句...")

        else:
            # 如果是普通回复（没有特定格式），则按原逻辑处理
            predicted_emotion = self.classifier.predict(response_text)
            self.update_pet_emotion(predicted_emotion)
            self.show_bubble(response_text)
        # <--- MODIFICATION END --->

    def handle_error(self, error_message):
        self.logger.error(f"处理错误: {error_message}")
        self.update_pet_emotion(CONFIG["ERROR_EMOTION"])
        self.show_bubble(error_message)
        # <--- MODIFICATION START --->
        # 发生错误时，也需要重置多段回复的状态
        self.current_response_segments = []
        self.current_segment_index = 0
        self.input_box.setPlaceholderText("在这里输入后按Enter...")
        # <--- MODIFICATION END --->

    def show_bubble(self, text):
        self.chat_bubble.setText(text)
        self.chat_bubble.show()
        self.chat_bubble.adjustSize()
        self.adjustSize()

    def mousePressEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            self.drag_position = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
            event.accept()

    def mouseMoveEvent(self, event: QMouseEvent):
        if event.buttons() == Qt.MouseButton.LeftButton:
            self.move(event.globalPosition().toPoint() - self.drag_position)
            event.accept()
    
    def mouseDoubleClickEvent(self, event: QMouseEvent):
        self.close()

# ==============================================================================
# ---                           程序主入口                          ---
# ==============================================================================
if __name__ == '__main__':
    # 确保 SYSTEM_PROMPT 包含格式要求
    if "【情绪】" not in CONFIG["SYSTEM_PROMPT"]:
         CONFIG["SYSTEM_PROMPT"] += " 你的回复必须遵循格式：【情绪】中文回复<日文翻译>（动作描述）。一段回复中可以包含多个这样的格式。"

    load_dotenv(PROJECT_ROOT / ".env")
    
    CONFIG["API_KEY"] = os.environ.get("CHAT_API_KEY")
    # 从.env加载的PROMPT会覆盖默认值
    CONFIG["SYSTEM_PROMPT"] = os.environ.get("SYSTEM_PROMPT", CONFIG["SYSTEM_PROMPT"])

    logger = Logger(log_dir=CONFIG["LOG_DIRECTORY"])
    logger.info("================== 桌面宠物启动 ==================")
    logger.info(f"项目根目录: {PROJECT_ROOT}")
    logger.info(f"角色: {CONFIG['CHARACTER_NAME']}")
    logger.info(f"角色图片路径: {CONFIG['CHARACTER_IMAGE_PATH'] / CONFIG['CHARACTER_NAME']}")
    logger.info(f"情绪模型路径: {CONFIG['EMOTION_MODEL_PATH']}")
    logger.info(f"API Key 已{'加载' if CONFIG['API_KEY'] else '未加载'}")


    app = QApplication(sys.argv)
    
    try:
        classifier = EmotionClassifier(CONFIG["EMOTION_MODEL_PATH"], logger)
        pet = DesktopPet(logger, classifier)
        pet.show()
        sys.exit(app.exec())
    except Exception as e:
        logger.error(f"程序启动时发生致命错误: {e}", exc_info=True)
        sys.exit(1)