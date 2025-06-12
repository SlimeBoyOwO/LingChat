import sys
import os
import json
import logging
import re
from datetime import datetime
from pathlib import Path
from functools import partial
import base64
from io import BytesIO

# --- GUI ---
from PyQt6.QtWidgets import QApplication, QWidget, QLabel, QVBoxLayout, QLineEdit, QPushButton, QHBoxLayout, QMenu, QMessageBox
from PyQt6.QtGui import QPixmap, QMouseEvent, QFont, QIcon, QAction, QPainter, QPen, QCursor, QColor
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QPoint, QRect, QBuffer, QTimer

# --- AI & 模型 ---
from openai import OpenAI
from dotenv import load_dotenv
import torch
from transformers import BertTokenizer, BertForSequenceClassification

# ==============================================================================
# ---                           配置区域                           ---
# ==============================================================================

# --- 核心路径计算 (高鲁棒性) ---
# 1. 获取脚本文件所在的目录，这是最可靠的锚点。
# 示例: 如果脚本是 D:\Auto\Github\LingChat\develop\backend\desktop_pet\desktop_pet.py
# SCRIPT_DIR 会是 D:\Auto\Github\LingChat\develop\backend\desktop_pet
try:
    SCRIPT_DIR = Path(__file__).resolve().parent
except NameError:
    # 如果在某些特殊环境（如某些交互式shell）__file__可能不存在。
    # 此时使用当前工作目录作为后备，但这不够可靠，并打印警告。
    print("警告: 无法通过 __file__ 确定脚本路径，将使用当前工作目录。这可能导致路径错误。")
    SCRIPT_DIR = Path.cwd()

# 2. 从脚本目录推导项目中的其他关键目录。
# BACKEND_DIR 会是 D:\Auto\Github\LingChat\develop\backend
BACKEND_DIR = SCRIPT_DIR.parent 

# PROJECT_ROOT 会是 D:\Auto\Github\LingChat\develop
PROJECT_ROOT = BACKEND_DIR.parent

# .env 文件路径 (通常在项目根目录)
ENV_PATH = PROJECT_ROOT / ".env"

CONFIG = {
    # --- 聊天API 配置 (将从 .env 加载) ---
    "CHAT_API_KEY": "sk-111",
    "CHAT_BASE_URL": "https://api.deepseek.com",
    "CHAT_MODEL": "deepseek-chat",

    # --- 视觉API 配置 (将从 .env 加载) ---
    "VD_API_KEY": "sk-111",
    "VD_BASE_URL": "https://api.gpt.ge/v1",
    "VD_MODEL": "gpt-4o",

    # --- 角色与提示词 (将从 .env 加载) ---
    "SYSTEM_PROMPT": "你是一个名为'灵灵'的AI助手，请用可爱、简洁、口语化的方式回答问题。你的回复必须遵循格式：【情绪】中文回复<日文翻译>（动作描述）。一段回复中可以包含多个这样的格式。",
    "SCREENSHOT_PROMPT": "你是一个图像描述专家。请用简洁的语言客观地描述这张图片的核心内容。你的描述将作为输入，由另一个AI来回答。请不要进行任何评价或联想，只描述你看到了什么。",

    # --- 文件路径配置 (基于上面计算出的路径) ---
    "CHARACTER_NAME": "qinling",
    "CHARACTER_IMAGE_PATH": PROJECT_ROOT / "frontend" / "public" / "pictures",
    
    # FIX: 情绪模型路径修正。它位于 backend 目录下，与 desktop_pet 目录同级。
    "EMOTION_MODEL_PATH": BACKEND_DIR / "emotion_model_18emo",
    
    "LOG_DIRECTORY": SCRIPT_DIR / "logs",
    "SCREENSHOT_DIRECTORY": SCRIPT_DIR / "screenshots",

    # --- 默认状态 (确保有对应的 .png 图片) ---
    "DEFAULT_EMOTION": "正常",
    "THINKING_EMOTION": "思考", # 确保 '思考.png' 存在
    "ERROR_EMOTION": "困惑",    # 确保 '困惑.png' 存在
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
        
        if self.logger.hasHandlers():
            self.logger.handlers.clear()

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
            # 检查模型路径是否存在
            if not model_path.exists():
                raise FileNotFoundError(f"情绪模型路径不存在: {model_path}")
            
            # 检查模型路径是否是一个目录，并且包含必要的文件
            # transformers.from_pretrained 期望一个目录
            if not model_path.is_dir():
                raise ValueError(f"情绪模型路径 '{model_path}' 不是一个目录。")

            self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
            self.logger.info(f"情绪分类模型将使用设备: {self.device}")

            # 尝试从本地文件加载模型和分词器
            self.tokenizer = BertTokenizer.from_pretrained(str(model_path), local_files_only=True)
            self.model = BertForSequenceClassification.from_pretrained(str(model_path), local_files_only=True)
            self.model.to(self.device)
            self.model.eval()

            # 加载标签映射文件
            mapping_path = model_path / "label_mapping.json"
            if not mapping_path.exists():
                 raise FileNotFoundError(f"找不到标签映射文件: {mapping_path}")
            
            with open(mapping_path, 'r', encoding='utf-8') as f:
                label_config = json.load(f)
            
            # 兼容 id2label 和 label2id 两种格式
            if "id2label" in label_config:
                self.id2label = label_config["id2label"]
            elif "label2id" in label_config:
                self.id2label = {str(v): k for k, v in label_config["label2id"].items()}

            if not self.id2label:
                raise ValueError("label_mapping.json 格式不正确或为空")
            
            # 确保字典的键是字符串，以匹配后面 `str(pred_id)`
            self.id2label = {str(k): v for k, v in self.id2label.items()}
            
            self.logger.info(f"成功加载情绪分类模型: {model_path.name}")
        except Exception as e:
            self.logger.error(f"加载情绪分类模型失败: {e}", exc_info=True)
            self.model = None # 确保模型加载失败时，self.model为None

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
            
            # 使用 str(pred_id) 作为键来查找，因为JSON加载的键是字符串
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
            api_key=CONFIG["CHAT_API_KEY"],
            base_url=CONFIG["CHAT_BASE_URL"],
        )

    def run(self):
        try:
            response = self.client.chat.completions.create(
                model=CONFIG["CHAT_MODEL"],
                messages=self.messages,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.response_ready.emit(ai_response)
        except Exception as e:
            logger.error(f"聊天API请求出错: {e}", exc_info=True)
            self.error_occurred.emit(f"呜...网络出错了，请检查API Key和网络连接。")

class VisionWorker(QThread):
    description_ready = pyqtSignal(str)
    error_occurred = pyqtSignal(str)

    def __init__(self, base64_image):
        super().__init__()
        self.base64_image = base64_image
        self.vision_client = OpenAI(
            api_key=CONFIG["VD_API_KEY"],
            base_url=CONFIG["VD_BASE_URL"],
        )

        logger.info(f"XXX视觉API是 {self.vision_client.api_key}")
        logger.info(f"XXX视觉URL是 {self.vision_client.base_url}")
        logger.info(f"XXX视觉模型 是 {CONFIG['VD_MODEL']}")

    def run(self):
        try:
            logger.info("向视觉模型发送请求以获取描述...")
            vision_response = self.vision_client.chat.completions.create(
                model=CONFIG["VD_MODEL"],
                messages=[
                    {
                        "role": "user",
                        "content": [
                            {"type": "text", "text": CONFIG["SCREENSHOT_PROMPT"]},
                            {
                                "type": "image_url",
                                "image_url": {
                                    "url": f"data:image/png;base64,{self.base64_image}"
                                },
                            },
                        ],
                    }
                ],
                max_tokens=300,
            )
            image_description = vision_response.choices[0].message.content
            logger.info(f"视觉模型返回的描述: {image_description}")
            self.description_ready.emit(image_description)

        except Exception as e:
            # 日志中的 openai.AuthenticationError 在这里被捕获
            logger.error(f"视觉模型请求出错: {e}", exc_info=True)
            self.error_occurred.emit(f"呜...分析图片时出错了，请检查VD_API_KEY和网络。")


# ==============================================================================
# ---           【已合并】截图覆盖层 (Multi-Monitor & DPI Aware)        ---
# ==============================================================================
class ScreenshotOverlay(QWidget):
    screenshot_taken = pyqtSignal(QPixmap)
    finished = pyqtSignal()

    def __init__(self, screen, full_desktop_pixmap):
        super().__init__()
        self.screen = screen
        self.full_desktop_pixmap = full_desktop_pixmap
        
        self.setGeometry(self.screen.geometry())
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint | Qt.WindowType.WindowStaysOnTopHint)
        self.setCursor(Qt.CursorShape.CrossCursor)
        self.setMouseTracking(True)
        
        self.begin = QPoint()
        self.end = QPoint()

    def paintEvent(self, event):
        painter = QPainter(self)
        painter.setRenderHint(QPainter.RenderHint.Antialiasing)

        monitor_geo = self.screen.geometry()
        ratio = self.screen.devicePixelRatio()
        
        virtual_desktop_geo = self.screen.virtualGeometry()
        
        offset_from_virtual_origin = monitor_geo.topLeft() - virtual_desktop_geo.topLeft()

        source_rect = QRect(
            int(offset_from_virtual_origin.x() * ratio),
            int(offset_from_virtual_origin.y() * ratio),
            int(monitor_geo.width() * ratio),
            int(monitor_geo.height() * ratio)
        )
        
        painter.drawPixmap(self.rect(), self.full_desktop_pixmap, source_rect)

        overlay_color = QColor(0, 0, 0, 120)
        painter.fillRect(self.rect(), overlay_color)
        
        if not self.begin.isNull() and not self.end.isNull():
            selection_rect_local = QRect(self.begin, self.end).normalized()
            
            selection_offset_from_monitor_origin_logical = selection_rect_local.topLeft()
            total_offset_logical = offset_from_virtual_origin + selection_offset_from_monitor_origin_logical

            selection_source_rect = QRect(
                int(total_offset_logical.x() * ratio),
                int(total_offset_logical.y() * ratio),
                int(selection_rect_local.width() * ratio),
                int(selection_rect_local.height() * ratio)
            )

            painter.drawPixmap(selection_rect_local, self.full_desktop_pixmap, selection_source_rect)
            painter.setPen(QPen(Qt.GlobalColor.white, 1, Qt.PenStyle.DashLine))
            painter.drawRect(selection_rect_local)

    def mousePressEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            self.begin = event.pos()
            self.end = self.begin
            self.update()

    def mouseMoveEvent(self, event):
        self.end = event.pos()
        self.update()

    def mouseReleaseEvent(self, event):
        if event.button() == Qt.MouseButton.LeftButton:
            selection_rect_local = QRect(self.begin, self.end).normalized()
            
            if selection_rect_local.width() > 5 and selection_rect_local.height() > 5:
                monitor_geo = self.screen.geometry()
                ratio = self.screen.devicePixelRatio()
                virtual_desktop_geo = self.screen.virtualGeometry()
                offset_from_virtual_origin = monitor_geo.topLeft() - virtual_desktop_geo.topLeft()
                
                selection_offset_from_monitor_origin_logical = selection_rect_local.topLeft()
                total_offset_logical = offset_from_virtual_origin + selection_offset_from_monitor_origin_logical

                final_crop_rect_physical = QRect(
                    int(total_offset_logical.x() * ratio),
                    int(total_offset_logical.y() * ratio),
                    int(selection_rect_local.width() * ratio),
                    int(selection_rect_local.height() * ratio)
                )

                final_pixmap = self.full_desktop_pixmap.copy(final_crop_rect_physical)
                final_pixmap.setDevicePixelRatio(ratio)
                self.screenshot_taken.emit(final_pixmap)
            
            self.finished.emit()

    def keyPressEvent(self, event):
        if event.key() == Qt.Key.Key_Escape:
            self.finished.emit()

# ==============================================================================
# ---                           主窗口 (桌宠)                         ---
# ==============================================================================
class DesktopPet(QWidget):
    def __init__(self, logger_instance, classifier_instance):
        super().__init__()
        self.logger = logger_instance
        self.classifier = classifier_instance
        self.screenshot_overlays = []
        self.screenshot_context = None
        self.conversation_history = [{"role": "system", "content": CONFIG["SYSTEM_PROMPT"]}]
        self.current_response_segments = []
        self.current_segment_index = 0
        self.drag_position = QPoint()
        self.original_pixmap = None 
        self.current_scale = 0.33
        self.SIZE_OPTIONS = {"小 (x0.25)": 0.25, "中 (x0.33)": 0.33, "大 (x0.50)": 0.50, "超大 (x0.75)": 0.75}

        # 检查API Keys
        if not CONFIG["CHAT_API_KEY"] or CONFIG["CHAT_API_KEY"] == "sk-111":
            self.logger.error("错误：CHAT_API_KEY未在 .env 文件中设置！")
            self.initial_message = "聊天API Key未设置，我无法思考哦！"
        else:
            self.initial_message = "你好呀！有什么可以帮你的吗？"
        if not CONFIG["VD_API_KEY"] or CONFIG["VD_API_KEY"] == "sk-111":
             self.logger.warning("警告：VD_API_KEY未在 .env 文件中设置，截图功能将不可用。")

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

    def load_character_images(self):
        character_path = CONFIG["CHARACTER_IMAGE_PATH"] / CONFIG["CHARACTER_NAME"]
        pixmaps = {}
        if not character_path.is_dir():
            self.logger.error(f"错误：角色图片文件夹 '{character_path}' 不存在！")
            return None
        for file in character_path.glob("*.png"):
            pixmaps[file.stem] = QPixmap(str(file))
        if not pixmaps or CONFIG["DEFAULT_EMOTION"] not in pixmaps:
            self.logger.error(f"角色图片资源不完整或默认情绪图片 '{CONFIG['DEFAULT_EMOTION']}.png' 缺失。")
            return None
        self.logger.info(f"成功为角色'{CONFIG['CHARACTER_NAME']}'加载 {len(pixmaps)} 张情绪图片。")
        return pixmaps

    def setup_ui(self, error_mode=False):
        self.layout = QVBoxLayout(self)
        self.layout.setContentsMargins(15, 15, 15, 15)
        self.chat_bubble = QLabel(self.initial_message, self)
        self.chat_bubble.setFont(QFont("Microsoft YaHei", 10))
        self.chat_bubble.setStyleSheet("background-color: rgba(52, 152, 219, 0.85); color: white; border: 1px solid rgba(255, 255, 255, 0.5); border-radius: 15px; padding: 10px;")
        self.chat_bubble.setWordWrap(True)
        self.chat_bubble.hide()
        input_layout = QHBoxLayout()
        self.input_box = QLineEdit(self)
        self.input_box.setPlaceholderText("在这里输入后按Enter...")
        self.input_box.setFont(QFont("Microsoft YaHei", 9))
        self.input_box.setStyleSheet("QLineEdit { background-color: rgba(41, 128, 185, 0.8); color: white; border: 1px solid rgba(255, 255, 255, 0.4); border-radius: 10px; padding: 5px 8px; } QLineEdit:focus { border: 1px solid rgba(255, 255, 255, 0.9); background-color: rgba(52, 152, 219, 0.9); }")
        button_style = "QPushButton { background-color: rgba(52, 152, 219, 0.85); color: white; border: 1px solid rgba(255, 255, 255, 0.5); border-radius: 10px; padding: 5px 10px; font-family: 'Microsoft YaHei'; font-size: 9pt; } QPushButton:hover { background-color: rgba(62, 172, 239, 0.9); } QPushButton:pressed { background-color: rgba(41, 128, 185, 0.95); }"
        self.screenshot_button = QPushButton("截图", self)
        self.screenshot_button.setStyleSheet(button_style)
        self.screenshot_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.screenshot_button.setToolTip("截图并提问")
        self.screenshot_button.clicked.connect(self.initiate_screenshot)
        self.send_button = QPushButton("发送", self)
        self.send_button.setStyleSheet(button_style)
        self.send_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.send_button.clicked.connect(self.start_new_chat)
        self.next_button = QPushButton("下一句", self)
        self.next_button.setStyleSheet(button_style)
        self.next_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.next_button.hide()
        self.next_button.clicked.connect(self.display_next_segment)
        self.input_box.returnPressed.connect(self.send_button.click)
        input_layout.addWidget(self.screenshot_button)
        input_layout.addWidget(self.input_box)
        input_layout.addWidget(self.send_button)
        input_layout.addWidget(self.next_button)
        self.pet_label = QLabel(self)
        self.pet_label.setScaledContents(True)
        if not error_mode:
            default_pixmap = self.emotion_pixmaps.get(CONFIG["DEFAULT_EMOTION"])
            if default_pixmap:
                self.pet_label.setFixedSize(int(default_pixmap.width() * self.current_scale), int(default_pixmap.height() * self.current_scale))
        else:
             self.pet_label.setFixedSize(128, 128)
        self.layout.addWidget(self.chat_bubble)
        self.layout.addWidget(self.pet_label, 0, Qt.AlignmentFlag.AlignCenter)
        self.layout.addLayout(input_layout)
        self.adjustSize()

    def initiate_screenshot(self):
        if not CONFIG["VD_API_KEY"] or CONFIG["VD_API_KEY"] == "sk-111":
            self.show_bubble("视觉API Key未设置，无法使用此功能哦！")
            self.update_pet_emotion(CONFIG["ERROR_EMOTION"])
            return
        
        self.hide()
        QTimer.singleShot(200, self._create_screenshot_overlays)

    def _create_screenshot_overlays(self):
        primary_screen = QApplication.primaryScreen()
        if not primary_screen:
            self.logger.error("无法获取主屏幕信息！截图功能失败。")
            self.cleanup_overlays()
            self.handle_error("糟糕，我好像没法看到你的屏幕...")
            return

        virtual_geo = primary_screen.virtualGeometry()
        full_desktop_pixmap = primary_screen.grabWindow(0, *virtual_geo.getRect())

        if full_desktop_pixmap.isNull():
            self.logger.error("无法截取桌面！截图功能失败。")
            self.cleanup_overlays()
            self.handle_error("糟糕，我好像没法看到你的屏幕...")
            return

        for screen in QApplication.screens():
            overlay = ScreenshotOverlay(screen, full_desktop_pixmap)
            overlay.screenshot_taken.connect(self.handle_screenshot_finished)
            overlay.finished.connect(self.cleanup_overlays)
            overlay.show()
            self.screenshot_overlays.append(overlay)
            
    def cleanup_overlays(self):
        for overlay in self.screenshot_overlays:
            overlay.close()
            overlay.deleteLater()
        self.screenshot_overlays.clear()
        self.show()
        self.activateWindow()

    def handle_screenshot_finished(self, pixmap):
        save_dir = CONFIG["SCREENSHOT_DIRECTORY"]
        save_path = save_dir / f"screenshot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
        pixmap.save(str(save_path), "PNG")
        self.logger.info(f"截图已保存至: {save_path}")
        
        buffer = QBuffer()
        buffer.open(QBuffer.OpenModeFlag.ReadWrite)
        pixmap.save(buffer, "PNG")
        base64_image = base64.b64encode(buffer.data()).decode("utf-8")
        buffer.close()
        
        self.show_bubble("正在识别图片内容...")
        self.update_pet_emotion(CONFIG["THINKING_EMOTION"])
        self._update_ui_mode('THINKING')
        
        self.vision_worker = VisionWorker(base64_image)
        self.vision_worker.description_ready.connect(self.handle_description_ready)
        self.vision_worker.error_occurred.connect(self.handle_error)
        self.vision_worker.start()

    def _update_ui_mode(self, mode: str):
        is_idle = (mode == 'IDLE')
        self.screenshot_button.setEnabled(is_idle)
        self.input_box.setEnabled(is_idle)
        if is_idle:
            self.send_button.show()
            self.next_button.hide()
            if not self.screenshot_context:
                self.input_box.setPlaceholderText("在这里输入后按Enter...")
            self.input_box.setFocus()
        elif mode == 'THINKING':
            self.send_button.hide()
            self.next_button.hide()
            self.input_box.setPlaceholderText("AI正在思考中...")
            self.input_box.setEnabled(False)
        elif mode == 'RESPONDING_MULTI':
            self.send_button.hide()
            self.next_button.show()
            self.input_box.setPlaceholderText("请按“下一句”继续...")
            self.input_box.setEnabled(False)
            self.next_button.setFocus()

    def _parse_ai_response(self, text):
        segments = []
        # 检查是否包含预期的格式标记
        if '【' not in text or '】' not in text:
            # 如果没有格式，尝试清理并作为单个默认情绪段落
            cleaned_text = re.sub(r'<.*?>|（.*?）', '', text).strip()
            if cleaned_text:
                segments.append({"original_tag": CONFIG["DEFAULT_EMOTION"], "chinese_text": cleaned_text})
            return segments
        
        # 按格式标记分割
        parts = text.split('【')
        for part in parts:
            if '】' not in part:
                continue
            try:
                emotion_tag, content = part.split('】', 1)
            except ValueError:
                # 如果分割失败，跳过此部分
                continue
            
            # 提取中文文本，去除日文翻译和动作描述
            chinese_text = re.sub(r'<[^>]*>|（[^）]*）', '', content).strip()
            if chinese_text:
                segments.append({"original_tag": emotion_tag.strip(), "chinese_text": chinese_text})
        
        # 如果解析后没有得到任何有效片段，则将整个回复作为默认情绪处理
        if not segments:
            self.logger.warning(f"无法从回复中解析出格式化片段，将显示完整回复: {text}")
            cleaned_text = re.sub(r'<.*?>|（.*?）|【.*?】', '', text).strip()
            if cleaned_text:
                segments.append({"original_tag": CONFIG["DEFAULT_EMOTION"], "chinese_text": cleaned_text})
        return segments

    def display_next_segment(self):
        if self.current_segment_index < len(self.current_response_segments):
            segment = self.current_response_segments[self.current_segment_index]
            # 使用情绪分类模型预测情绪
            predicted_emotion = self.classifier.predict(segment.get('original_tag', CONFIG['DEFAULT_EMOTION']))
            self.update_pet_emotion(predicted_emotion)
            self.show_bubble(segment['chinese_text'])
            self._update_ui_mode('RESPONDING_MULTI')
            self.current_segment_index += 1
        else:
            # 所有片段显示完毕
            self.chat_bubble.hide()
            self.current_response_segments = []
            self.current_segment_index = 0
            self.update_pet_emotion(CONFIG["DEFAULT_EMOTION"])
            self._update_ui_mode('IDLE')
    
    def start_new_chat(self):
        user_input = self.input_box.text().strip()
        if not user_input or not CONFIG["CHAT_API_KEY"] or CONFIG["CHAT_API_KEY"] == "sk-111": return

        final_prompt = user_input
        if self.screenshot_context:
            final_prompt = f"我刚才截了一张图，内容是：“{self.screenshot_context}”。现在，关于这张图，我的问题是：“{user_input}”"
            self.logger.info(f"结合截图上下文生成新提问: {final_prompt}")
            self.screenshot_context = None # 清除截图上下文，避免下次聊天继续使用

        self.logger.info(f"用户输入 (原始): {user_input}")
        self.conversation_history.append({"role": "user", "content": final_prompt})
        self.input_box.clear()
        
        self.show_bubble("思考中...")
        self.update_pet_emotion(CONFIG["THINKING_EMOTION"])
        self._update_ui_mode('THINKING')
        
        self.worker = ChatWorker(self.conversation_history)
        self.worker.response_ready.connect(self.handle_response)
        self.worker.error_occurred.connect(self.handle_error)
        self.worker.start()

    def handle_description_ready(self, description):
        self.logger.info("图片描述接收成功。")
        self.screenshot_context = description
        self.update_pet_emotion(CONFIG["DEFAULT_EMOTION"])
        self.show_bubble("好啦，图片我看过啦，你想问什么呀？")
        self._update_ui_mode('IDLE')
        self.input_box.setPlaceholderText("请针对图片内容提问...")
        self.input_box.setFocus()

    def update_pet_emotion(self, emotion_name):
        pixmap = self.emotion_pixmaps.get(emotion_name)
        if not pixmap:
            self.logger.warning(f"找不到情绪 '{emotion_name}' 的图片，使用默认图片。")
            pixmap = self.emotion_pixmaps.get(CONFIG["DEFAULT_EMOTION"])
        
        if pixmap:
            self.original_pixmap = pixmap
            # 应用缩放并更新显示
            scaled_pixmap = self.original_pixmap.scaled(
                int(self.original_pixmap.width() * self.current_scale),
                int(self.original_pixmap.height() * self.current_scale),
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation
            )
            self.pet_label.setPixmap(scaled_pixmap)
            self.pet_label.setFixedSize(scaled_pixmap.size())
            self.adjustSize()

    def handle_response(self, response_text):
        self.logger.info(f"AI原始回复: {response_text}")
        self.conversation_history.append({"role": "assistant", "content": response_text})
        segments = self._parse_ai_response(response_text)
        if segments:
            self.current_response_segments = segments
            self.current_segment_index = 0
            self.display_next_segment() # 开始显示第一个片段
        else:
            # 如果解析失败，直接显示原始回复并处理为错误状态
            self.logger.warning("解析后的片段为空，将显示原始回复并进入错误状态。")
            self.handle_error(f"AI的回复格式好像有点问题，我没看懂... (原始回复: {response_text[:50]}...)")

    def handle_error(self, error_message):
        self.logger.error(f"处理错误: {error_message}")
        self.screenshot_context = None # 清除任何截图上下文
        self.update_pet_emotion(CONFIG["ERROR_EMOTION"])
        self.show_bubble(error_message)
        self.current_response_segments = [] # 清除待显示片段
        self.current_segment_index = 0
        self._update_ui_mode('IDLE') # 恢复到空闲模式

    def show_bubble(self, text):
        self.chat_bubble.setText(text)
        self.chat_bubble.show()
        self.chat_bubble.adjustSize()
        self.adjustSize()

    def contextMenuEvent(self, event):
        # 只有在点击宠物本身或背景时才显示右键菜单
        if self.childAt(event.pos()) in [None, self.pet_label]:
            menu = QMenu(self)
            menu.setStyleSheet("QMenu { background-color: rgba(41, 128, 185, 0.9); color: white; border: 1px solid rgba(255, 255, 255, 0.5); } QMenu::item:selected { background-color: rgba(52, 152, 219, 1.0); }")
            
            size_menu = menu.addMenu("调整大小")
            for text, scale in self.SIZE_OPTIONS.items():
                action = QAction(text, self, checkable=True)
                action.setChecked(scale == self.current_scale)
                action.triggered.connect(partial(self.resize_pet, scale))
                size_menu.addAction(action)
            
            menu.addSeparator()
            menu.addAction("退出").triggered.connect(self.close)
            menu.exec(event.globalPos())

    def resize_pet(self, scale):
        if self.current_scale == scale: return # 如果大小没变，则不操作
        self.current_scale = scale
        # 重新应用当前情绪和大小
        # 找到当前显示的原始情绪名称，然后更新
        current_emotion_name = CONFIG["DEFAULT_EMOTION"]
        for emo_name, pixmap in self.emotion_pixmaps.items():
            if pixmap == self.original_pixmap:
                current_emotion_name = emo_name
                break
        self.update_pet_emotion(current_emotion_name)
        self.adjustSize() # 调整窗口大小以适应新的宠物图片大小

    def mousePressEvent(self, event: QMouseEvent):
        # 只有在点击宠物本身或背景时才允许拖动
        if event.button() == Qt.MouseButton.LeftButton and self.childAt(event.pos()) in [None, self.pet_label]:
            self.drag_position = event.globalPosition().toPoint() - self.frameGeometry().topLeft()
            event.accept()

    def mouseMoveEvent(self, event: QMouseEvent):
        if event.buttons() == Qt.MouseButton.LeftButton and not self.drag_position.isNull():
            self.move(event.globalPosition().toPoint() - self.drag_position)
            event.accept()
    
    def mouseReleaseEvent(self, event: QMouseEvent):
        self.drag_position = QPoint()
        event.accept()

    def mouseDoubleClickEvent(self, event: QMouseEvent):
        # 只有在双击宠物本身或背景时才退出
        if self.childAt(event.pos()) in [None, self.pet_label]:
            self.close()


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

    logger.info(f"视觉API是 {CONFIG['VD_API_KEY']}")
    logger.info(f"视觉URL是 {CONFIG['VD_BASE_URL']}")
    logger.info(f"视觉模型 是 {CONFIG['VD_MODEL']}")

    # 启动Qt应用
    app = QApplication(sys.argv)
    
    try:
        classifier = EmotionClassifier(CONFIG["EMOTION_MODEL_PATH"], logger)
        pet = DesktopPet(logger, classifier)
        pet.show()
        sys.exit(app.exec())
    except Exception as e:
        logger.error(f"程序启动时发生致命错误: {e}", exc_info=True)
        # 提供GUI错误提示
        msg_box = QMessageBox()
        msg_box.setIcon(QMessageBox.Icon.Critical)
        msg_box.setText("程序启动失败！")
        msg_box.setInformativeText(f"发生了一个严重错误，请检查日志文件获取详细信息。\n\n错误: {e}")
        msg_box.setWindowTitle("致命错误")
        msg_box.exec()
        sys.exit(1)