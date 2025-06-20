import sys
import os

from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QToolBar, QLineEdit, QPushButton,
    QVBoxLayout, QWidget, QProgressBar, QSizePolicy
)
from PyQt6.QtCore import QUrl, Qt
from PyQt6.QtGui import QIcon
from PyQt6.QtWebEngineWidgets import QWebEngineView
from PyQt6.QtWebEngineCore import QWebEngineProfile

class Browser(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("简易浏览器 - PyQt6")
        self.setGeometry(100, 100, 1024, 768)

        icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "game_data", "resources", "lingchat.ico")
        self.setWindowIcon(QIcon(icon_path)) 

        # --- 核心改动开始 ---

        # 1. 定义自定义缓存目录路径 (逻辑移入类内部)
        current_script_dir = os.path.dirname(os.path.abspath(__file__))
        self.custom_data_dir = os.path.join(current_script_dir, "browser_data")
        os.makedirs(self.custom_data_dir, exist_ok=True)
        print(f"WebEngine持久化存储路径已设置为: {self.custom_data_dir}")

        # 2. 创建一个全新的、非默认的 QWebEngineProfile
        #    "storage" 是这个 profile 的名字，可以是任意字符串，方便调试
        #    self 作为父对象，确保 profile 随窗口关闭而正确销毁
        self.profile = QWebEngineProfile("storage", self)
        
        # 3. 在 profile 被使用前，设置其持久化存储路径
        self.profile.setPersistentStoragePath(self.custom_data_dir)

        # 4. 创建 QWebEngineView 实例，并将我们自定义的 profile 传递给它
        self.browser = QWebEngineView(self.profile, self) # 传入 profile
        
        # --- 核心改动结束 ---

        self.browser.setUrl(QUrl("http://localhost:8765")) 

        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        main_layout = QVBoxLayout(central_widget)

        self.toolbar = QToolBar("导航")
        self.addToolBar(self.toolbar)

        chat_btn = QPushButton("聊天")
        chat_btn.clicked.connect(self.navigate_to_chat)
        self.toolbar.addWidget(chat_btn)
        
        settings_btn = QPushButton("设置")
        settings_btn.clicked.connect(self.navigate_to_settings)
        self.toolbar.addWidget(settings_btn)

        self.url_bar = QLineEdit()
        self.url_bar.returnPressed.connect(self.navigate_to_url)
        self.url_bar.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        self.toolbar.addWidget(self.url_bar)

        reload_btn = QPushButton("刷新")
        reload_btn.clicked.connect(self.browser.reload)
        self.toolbar.addWidget(reload_btn)
        
        main_layout.addWidget(self.browser)

        self.browser.urlChanged.connect(self.update_url_bar)
        self.browser.titleChanged.connect(self.setWindowTitle)
        self.browser.loadProgress.connect(self.update_progress)

        self.progress_bar = QProgressBar()
        self.progress_bar.setMaximumHeight(8)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.hide()
        # 修正：将进度条添加到布局中，而不是工具栏，这样它会出现在工具栏下方
        main_layout.insertWidget(0, self.progress_bar)
        main_layout.setSpacing(0) # 让进度条和浏览器视图紧挨着


    def navigate_to_url(self):
        url_text = self.url_bar.text()
        if not url_text.startswith(('http://', 'https://')):
            url_text = 'http://' + url_text
        self.browser.setUrl(QUrl(url_text))

    def update_url_bar(self, qurl):
        self.url_bar.setText(qurl.toString())
        self.url_bar.setCursorPosition(0)

    def navigate_to_chat(self):
        self.browser.setUrl(QUrl("http://localhost:8765")) 

    def navigate_to_settings(self):
        self.browser.setUrl(QUrl("http://localhost:8765/settings"))

    def update_progress(self, progress):
        # 修正逻辑，确保进度条在加载开始时显示，结束时隐藏
        if 0 < progress < 100:
            self.progress_bar.setValue(progress)
            if not self.progress_bar.isVisible():
                self.progress_bar.show()
        else:
            self.progress_bar.hide()
            self.progress_bar.setValue(0)


if __name__ == '__main__':
    app = QApplication(sys.argv)
    app.setApplicationName("PyQtWebBrowser")

    # 注意：所有关于 profile 的设置都已移入 Browser 类中
    # main 函数现在只负责创建应用和窗口
    
    window = Browser()
    window.show()
    sys.exit(app.exec())