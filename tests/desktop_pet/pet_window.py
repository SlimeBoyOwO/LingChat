# file: pet_window.py

import re
import base64
from datetime import datetime
from functools import partial

# --- GUI ---
from PyQt6.QtWidgets import QWidget, QLabel, QVBoxLayout, QLineEdit, QPushButton, QHBoxLayout, QMenu, QApplication
from PyQt6.QtGui import QPixmap, QMouseEvent, QFont, QIcon, QAction
from PyQt6.QtCore import Qt, QPoint, QBuffer, QTimer

# --- Local Modules ---
from config import CONFIG
from api_workers import ChatWorker, VisionWorker
from ui_components import ScreenshotOverlay

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
        if '【' not in text or '】' not in text:
            cleaned_text = re.sub(r'<.*?>|（.*?）', '', text).strip()
            if cleaned_text:
                segments.append({"original_tag": CONFIG["DEFAULT_EMOTION"], "chinese_text": cleaned_text})
            return segments
        
        parts = text.split('【')
        for part in parts:
            if '】' not in part:
                continue
            try:
                emotion_tag, content = part.split('】', 1)
            except ValueError:
                continue
            
            chinese_text = re.sub(r'<[^>]*>|（[^）]*）', '', content).strip()
            if chinese_text:
                segments.append({"original_tag": emotion_tag.strip(), "chinese_text": chinese_text})
        
        if not segments:
            self.logger.warning(f"无法从回复中解析出格式化片段，将显示完整回复: {text}")
            cleaned_text = re.sub(r'<.*?>|（.*?）|【.*?】', '', text).strip()
            if cleaned_text:
                segments.append({"original_tag": CONFIG["DEFAULT_EMOTION"], "chinese_text": cleaned_text})
        return segments

    def display_next_segment(self):
        if self.current_segment_index < len(self.current_response_segments):
            segment = self.current_response_segments[self.current_segment_index]
            predicted_emotion = self.classifier.predict(segment.get('original_tag', CONFIG['DEFAULT_EMOTION']))
            self.update_pet_emotion(predicted_emotion)
            self.show_bubble(segment['chinese_text'])
            self._update_ui_mode('RESPONDING_MULTI')
            self.current_segment_index += 1
        else:
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
            self.screenshot_context = None

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
            self.display_next_segment()
        else:
            self.logger.warning("解析后的片段为空，将显示原始回复并进入错误状态。")
            self.handle_error(f"AI的回复格式好像有点问题，我没看懂... (原始回复: {response_text[:50]}...)")

    def handle_error(self, error_message):
        self.logger.error(f"处理错误: {error_message}")
        self.screenshot_context = None
        self.update_pet_emotion(CONFIG["ERROR_EMOTION"])
        self.show_bubble(error_message)
        self.current_response_segments = []
        self.current_segment_index = 0
        self._update_ui_mode('IDLE')

    def show_bubble(self, text):
        self.chat_bubble.setText(text)
        self.chat_bubble.show()
        self.chat_bubble.adjustSize()
        self.adjustSize()

    def contextMenuEvent(self, event):
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
        if self.current_scale == scale: return
        self.current_scale = scale
        current_emotion_name = CONFIG["DEFAULT_EMOTION"]
        for emo_name, pixmap in self.emotion_pixmaps.items():
            if pixmap == self.original_pixmap:
                current_emotion_name = emo_name
                break
        self.update_pet_emotion(current_emotion_name)
        self.adjustSize()

    def mousePressEvent(self, event: QMouseEvent):
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
        if self.childAt(event.pos()) in [None, self.pet_label]:
            self.close()