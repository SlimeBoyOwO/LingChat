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
    """
    一个简单的基于PyQt6和QtWebEngine的浏览器。
    支持自定义缓存路径，基础导航功能，以及加载进度显示。
    """
    def __init__(self):
        super().__init__()
        self.setWindowTitle("简易浏览器 - PyQt6")
        self.setGeometry(100, 100, 1024, 768)

        icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "game_data", "resources", "lingchat.ico")
        self.setWindowIcon(QIcon(icon_path)) 

        current_script_dir = os.path.dirname(os.path.abspath(__file__))
        self.custom_data_dir = os.path.join(current_script_dir, "browser_data")
        os.makedirs(self.custom_data_dir, exist_ok=True)
        print(f"WebEngine持久化存储路径已设置为: {self.custom_data_dir}")

        self.profile = QWebEngineProfile("storage_profile", self)
        self.profile.setPersistentStoragePath(self.custom_data_dir)

        self.browser = QWebEngineView(self.profile, self)
        self.browser.setUrl(QUrl("http://localhost:8765")) 

        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        main_layout = QVBoxLayout(central_widget)

        self._create_toolbar()
        
        self.progress_bar = QProgressBar()
        self.progress_bar.setMaximumHeight(8)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.hide()
        main_layout.insertWidget(0, self.progress_bar)
        main_layout.setSpacing(0)

        main_layout.addWidget(self.browser)

        self.browser.urlChanged.connect(self._update_url_bar)
        self.browser.titleChanged.connect(self.setWindowTitle)
        self.browser.loadProgress.connect(self._update_progress)

    def _create_toolbar(self):
        """创建并配置工具栏。"""
        self.toolbar = QToolBar("导航")
        self.addToolBar(self.toolbar)

        chat_btn = QPushButton("聊天")
        chat_btn.clicked.connect(self._navigate_to_chat)
        self.toolbar.addWidget(chat_btn)
        
        settings_btn = QPushButton("设置")
        settings_btn.clicked.connect(self._navigate_to_settings)
        self.toolbar.addWidget(settings_btn)

        self.url_bar = QLineEdit()
        self.url_bar.returnPressed.connect(self._navigate_to_url)
        self.url_bar.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        self.toolbar.addWidget(self.url_bar)

        reload_btn = QPushButton("刷新")
        reload_btn.clicked.connect(self.browser.reload)
        self.toolbar.addWidget(reload_btn)

    def _navigate_to_url(self):
        """根据地址栏输入导航到指定URL。"""
        url_text = self.url_bar.text()
        if not url_text.startswith(('http://', 'https://')):
            url_text = 'http://' + url_text
        self.browser.setUrl(QUrl(url_text))

    def _update_url_bar(self, qurl):
        """当浏览器URL改变时更新地址栏。"""
        self.url_bar.setText(qurl.toString())
        self.url_bar.setCursorPosition(0)

    def _navigate_to_chat(self):
        """导航到聊天页面。"""
        self.browser.setUrl(QUrl("http://localhost:8765")) 

    def _navigate_to_settings(self):
        """导航到设置页面。"""
        self.browser.setUrl(QUrl("http://localhost:8765/settings"))

    def _update_progress(self, progress):
        """更新加载进度条的显示。"""
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
    
    window = Browser()
    window.show()
    sys.exit(app.exec())