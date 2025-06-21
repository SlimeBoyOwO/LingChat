import sys
import os

from PyQt6.QtWidgets import (
    QApplication, QMainWindow, QPushButton, QVBoxLayout, QWidget, 
    QProgressBar, QStackedWidget, QLabel, QHBoxLayout, QFrame
)
from PyQt6.QtCore import QUrl, Qt, QPoint
from PyQt6.QtGui import QIcon, QPixmap, QMouseEvent
from PyQt6.QtWebEngineWidgets import QWebEngineView
from PyQt6.QtWebEngineCore import QWebEngineProfile

class GameWindow(QMainWindow):
    """
    一个拥有Galgame风格界面的浏览器应用。
    - 主菜单界面 (开始, 设置, 退出)
    - 游戏/聊天界面 (内嵌网页)
    - 标准窗口边框
    """
    def __init__(self):
        super().__init__()
        
        self.setWindowTitle("LingChat")
        self.setFixedSize(1024, 768)
        
        icon_path = os.path.join("develop", "game_data", "resources", "lingchat.ico")
        self.setWindowIcon(QIcon(icon_path))
        
        self.stacked_widget = QStackedWidget()
        self.setCentralWidget(self.stacked_widget)

        self._create_web_engine_profile()
        self.main_menu_page = self._create_main_menu_page()
        self.browser_page = self._create_browser_page()

        self.stacked_widget.addWidget(self.main_menu_page)
        self.stacked_widget.addWidget(self.browser_page)

        self._apply_styles()
        
        self.stacked_widget.setCurrentWidget(self.main_menu_page)

    def _create_web_engine_profile(self):
        """创建并配置WebEngine，使其数据持久化"""
        current_script_dir = os.path.dirname(os.path.abspath(__file__))
        self.custom_data_dir = os.path.join(current_script_dir, "browser_data")
        os.makedirs(self.custom_data_dir, exist_ok=True)
        print(f"WebEngine持久化存储路径已设置为: {self.custom_data_dir}")

        self.profile = QWebEngineProfile("storage_profile", self)
        self.profile.setPersistentStoragePath(self.custom_data_dir)

    def _create_main_menu_page(self) -> QWidget:
        """创建游戏主菜单界面"""
        page = QWidget()
        
        layout = QVBoxLayout(page)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(15)
        layout.addStretch(3)

        start_game_btn = QPushButton("开始游戏")
        start_game_btn.setObjectName("MenuButton")
        start_game_btn.clicked.connect(self._navigate_to_chat)

        settings_btn = QPushButton("游戏设置")
        settings_btn.setObjectName("MenuButton")
        settings_btn.clicked.connect(self._navigate_to_settings)

        exit_btn = QPushButton("退出游戏")
        exit_btn.setObjectName("MenuButton")
        exit_btn.clicked.connect(self.close)

        button_layout = QVBoxLayout()
        button_layout.setSpacing(20)
        button_layout.addWidget(start_game_btn)
        button_layout.addWidget(settings_btn)
        button_layout.addWidget(exit_btn)

        h_layout = QHBoxLayout()
        h_layout.addStretch()
        h_layout.addLayout(button_layout)
        h_layout.addStretch()

        layout.addLayout(h_layout)
        layout.addStretch(1)

        page.setObjectName("MainMenu")
        return page

    def _create_browser_page(self) -> QWidget:
        """创建游戏内嵌浏览器界面"""
        page = QWidget()
        layout = QVBoxLayout(page)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        top_bar = QFrame()
        top_bar.setObjectName("TopBar")
        top_bar.setFixedHeight(40)
        top_bar_layout = QHBoxLayout(top_bar)
        
        back_button = QPushButton("返回主菜单")
        back_button.setObjectName("BackButton")
        back_button.clicked.connect(self._show_main_menu)
        
        self.progress_bar = QProgressBar()
        self.progress_bar.setMaximumHeight(8)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.hide()

        top_bar_layout.addWidget(back_button)
        top_bar_layout.addStretch()
        top_bar_layout.addWidget(self.progress_bar, 1)
        top_bar_layout.setContentsMargins(10, 5, 10, 5)

        layout.addWidget(top_bar)
        
        self.browser = QWebEngineView(self.profile, self)
        layout.addWidget(self.browser)

        self.browser.titleChanged.connect(self.setWindowTitle)
        self.browser.loadProgress.connect(self._update_progress)
        
        return page

    def _apply_styles(self):
        """应用CSS样式表，实现Galgame风格"""
        background_image_path_raw = os.path.join(
            "frontend", "public", "pictures", "backgrounds", "login", "X0_6rTZl.png"
        )
        background_image_path = background_image_path_raw.replace("\\", "/")
        
        self.setStyleSheet(f"""
            /* 主菜单背景 */
            #MainMenu {{
                background-image: url({background_image_path});
                background-repeat: no-repeat;
                background-position: center;
                /* 改动 2: 移除了 border-radius，因为窗口不再是无边框圆角 */
            }}

            /* 菜单按钮样式 */
            #MenuButton {{
                background-color: rgba(0, 0, 0, 0.6); /* 半透明黑色背景 */
                color: white;
                font-size: 24px;
                font-family: 'Microsoft YaHei', 'Segoe UI', sans-serif;
                font-weight: bold;
                padding: 15px 60px;
                border: 2px solid rgba(255, 255, 255, 0.7);
                border-radius: 25px; /* 圆角按钮 */
                min-width: 200px; /* 最小宽度 */
            }}
            #MenuButton:hover {{
                background-color: rgba(255, 105, 180, 0.7); /* 悬停时变为半透明粉色 */
                border: 2px solid white;
            }}
            #MenuButton:pressed {{
                background-color: rgba(255, 20, 147, 0.8); /* 按下时颜色更深 */
            }}

            /* 浏览器界面顶部栏 */
            #TopBar {{
                background-color: #2c3e50;
            }}
            
            /* 返回按钮 */
            #BackButton {{
                background-color: #3498db;
                color: white;
                font-size: 14px;
                border: none;
                padding: 5px 15px;
                border-radius: 5px;
            }}
            #BackButton:hover {{
                background-color: #2980b9;
            }}

            /* 进度条样式 */
            QProgressBar {{
                border: 1px solid #2c3e50;
                background-color: #555;
                height: 6px;
                text-align: center;
                border-radius: 3px;
            }}
            QProgressBar::chunk {{
                background-color: #3498db; /* 进度条颜色 */
                border-radius: 3px;
            }}
        """)

    # --- 导航和界面切换逻辑 ---
    def _navigate_to_chat(self):
        """加载聊天URL并切换到浏览器界面"""
        self.browser.setUrl(QUrl("http://localhost:8765"))
        self.stacked_widget.setCurrentWidget(self.browser_page)

    def _navigate_to_settings(self):
        """加载设置URL并切换到浏览器界面"""
        self.browser.setUrl(QUrl("http://localhost:8765/settings"))
        self.stacked_widget.setCurrentWidget(self.browser_page)

    def _show_main_menu(self):
        """切换回主菜单界面"""
        self.stacked_widget.setCurrentWidget(self.main_menu_page)
        self.setWindowTitle("LingChat")

    def _update_progress(self, progress):
        """更新加载进度条的显示"""
        if 0 < progress < 100:
            self.progress_bar.setValue(progress)
            if not self.progress_bar.isVisible():
                self.progress_bar.show()
        else:
            self.progress_bar.hide()
            self.progress_bar.setValue(0)

if __name__ == '__main__':
    app = QApplication(sys.argv)
    app.setApplicationName("GalgameBrowser")
    
    window = GameWindow()
    window.show()
    sys.exit(app.exec())