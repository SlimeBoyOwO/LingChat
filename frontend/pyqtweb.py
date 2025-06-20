import sys
from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QToolBar, QLineEdit, QPushButton,
    QVBoxLayout, QWidget, QProgressBar, QSizePolicy # QSizePolicy 已经导入，但使用方式需修正
)
from PyQt6.QtCore import QUrl, Qt
from PyQt6.QtGui import QIcon
from PyQt6.QtWebEngineWidgets import QWebEngineView

class Browser(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("简易浏览器 - PyQt6")
        self.setGeometry(100, 100, 1024, 768)

        # 设置窗口图标
        # 假设你的Python脚本在 'frontend/' 目录下
        # 图标路径相对于脚本的执行位置
        self.setWindowIcon(QIcon("../game_data/resources/lingchat.ico")) 

        # 1. 创建 QWebEngineView 实例
        self.browser = QWebEngineView()
        # 设置初始加载的URL为本地的8765端口
        self.browser.setUrl(QUrl("http://localhost:8765")) 

        # 2. 创建一个中央小部件，并设置布局
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        main_layout = QVBoxLayout(central_widget)

        # 3. 创建工具栏
        self.toolbar = QToolBar("导航")
        self.addToolBar(self.toolbar)

        # --- 按钮顺序调整开始 ---

        # "聊天" 按钮 (原 "主页")
        chat_btn = QPushButton("聊天")
        chat_btn.clicked.connect(self.navigate_to_chat)
        self.toolbar.addWidget(chat_btn)
        
        # "设置" 按钮
        settings_btn = QPushButton("设置")
        settings_btn.clicked.connect(self.navigate_to_settings)
        self.toolbar.addWidget(settings_btn)

        # 地址栏
        self.url_bar = QLineEdit()
        self.url_bar.returnPressed.connect(self.navigate_to_url)
        # 修正：使用 QSizePolicy.Policy.Expanding 和 QSizePolicy.Policy.Preferred
        self.url_bar.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)
        self.toolbar.addWidget(self.url_bar)

        # 刷新按钮 (移动到最右边，所以放在最后添加)
        reload_btn = QPushButton("刷新")
        reload_btn.clicked.connect(self.browser.reload)
        self.toolbar.addWidget(reload_btn)
        
        # --- 按钮顺序调整结束 ---
        
        # 4. 将浏览器视图添加到主布局中
        main_layout.addWidget(self.browser)

        # 5. 连接信号和槽：
        self.browser.urlChanged.connect(self.update_url_bar)
        self.browser.titleChanged.connect(self.setWindowTitle)
        self.browser.loadProgress.connect(self.update_progress)

        # 6. 添加一个进度条 (可选)
        self.progress_bar = QProgressBar()
        self.progress_bar.setMaximumHeight(8)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.hide()
        self.toolbar.addWidget(self.progress_bar)


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
        if progress < 100 and not self.progress_bar.isVisible():
            self.progress_bar.show()
        elif progress == 100 and self.progress_bar.isVisible():
            self.progress_bar.hide()
        self.progress_bar.setValue(progress)


if __name__ == '__main__':
    app = QApplication(sys.argv)
    window = Browser()
    window.show()
    sys.exit(app.exec())