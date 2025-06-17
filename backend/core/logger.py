# logger.py
import logging
import sys
import time
import threading
from datetime import datetime
from dotenv import load_dotenv
import os
import re
from typing import Optional, Dict, List, Any, Callable


class TermColors:
    """ANSI 终端颜色代码"""
    GREY = '\033[90m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    RED = '\033[91m'
    BLUE = '\033[94m'
    RESET = '\033[0m'
    WHITE = '\033[97m'
    CYAN = '\033[96m'
    MAGENTA = '\033[95m'
    ORANGE = '\033[38;5;208m'
    BOLD = '\033[1m'


class Logger:
    """单例日志记录器，支持彩色输出和加载动画"""
    
    _instance = None
    _initialized = False
    
    # 默认配置
    DEFAULT_ANIMATION_STYLE = 'braille'
    DEFAULT_ANIMATION_COLOR = TermColors.WHITE
    DATE_FORMAT = "%Y-%m-%d-%H:%M:%S"
    
    ANIMATION_STYLES = {
        'braille': ['⢿', '⣻', '⣽', '⣾', '⣷', '⣯', '⣟', '⡿'],
        'spinner': ['-', '\\', '|', '/'],
        'dots': ['.  ', '.. ', '...', ' ..', '  .', '   '],
        'arrows': ['←', '↖', '↑', '↗', '→', '↘', '↓', '↙'],
        'moon': ['🌑', '🌒', '🌓', '🌔', '🌕', '🌖', '🌗', '🌘'],
        'clock': ['🕛', '🕐', '🕑', '🕒', '🕓', '🕔', '🕕', '🕖', '🕗', '🕘', '🕙', '🕚'],
        'directional_arrows_unicode': ['⬆️', '↗️', '➡️', '↘️', '⬇️', '↙️', '⬅️', '↖️'],
        'traffic_lights': ['🔴', '🟡', '🟢'],
        'growth_emoji': ['🌱', '🌿', '🌳'],
        'weather_icons': ['☀️', '☁️', '🌧️', '⚡️'],
        'heartbeat': ['♡', '♥'],
    }

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(
        self,
        app_name: str = "AppLogger",
        log_level: Optional[str] = None,
        show_timestamp: Optional[bool] = None,
        enable_file_logging: bool = True,
        log_file_directory: str = os.path.join("data", "run_logs"),
        log_file_level: int = logging.DEBUG
    ):
        """初始化日志记录器
        
        Args:
            app_name: 应用名称
            log_level: 日志级别 (None 时从环境变量读取)
            show_timestamp: 是否显示时间戳 (None 时从环境变量读取)
            enable_file_logging: 是否启用文件日志
            log_file_directory: 日志文件目录
            log_file_level: 文件日志级别
        """
        if self._initialized:
            return

        load_dotenv()

        # 初始化配置
        self.app_name = app_name
        self.log_level = self._get_log_level(log_level)
        self.print_context = self._get_bool_env('PRINT_CONTEXT', None)
        self.show_timestamp = self._get_bool_env('CONSOLE_SHOW_TIMESTAMP', show_timestamp)
        self.enable_file_logging = enable_file_logging
        self.log_file_directory = log_file_directory
        self.log_file_level = log_file_level
        
        # 动画相关状态
        self._animation_thread = None
        self._stop_animation_event = threading.Event()
        self._is_animating = False
        self._current_animation_line_width = 0
        self._animation_lock = threading.Lock()
        
        # 初始化日志系统
        self._initialize_logger()
        
        self._initialized = True

    def _get_log_level(self, explicit_level: Optional[str]) -> int:
        """获取日志级别配置"""
        if explicit_level is not None:
            level_str = explicit_level
        else:
            level_str = os.environ.get('LOG_LEVEL', 'INFO')
        
        # 转换字符串日志级别为logging模块的常量
        level_map = {
            'DEBUG': logging.DEBUG,
            'INFO': logging.INFO,
            'WARNING': logging.WARNING,
            'ERROR': logging.ERROR,
            'CRITICAL': logging.CRITICAL
        }
        
        return level_map.get(level_str.upper(), logging.INFO)

    def _get_bool_env(self, env_var: str, explicit_value: Optional[bool]) -> bool:
        """获取布尔型配置，优先使用显式设置，其次环境变量"""
        if explicit_value is not None:
            return explicit_value
        return os.environ.get(env_var, "false").lower() == "true"

    def _initialize_logger(self):
        """初始化日志处理器"""
        self._logger = logging.getLogger(self.app_name)
        self._logger.propagate = False
        self._logger.setLevel(self.log_level)

        # 清除现有处理器
        for handler in self._logger.handlers[:]:
            handler.close()
            self._logger.removeHandler(handler)

        # 控制台处理器
        console_handler = self._create_console_handler()
        self._logger.addHandler(console_handler)

        # 文件处理器
        # if self.enable_file_logging:
        #    file_handler = self._create_file_handler()
        #    if file_handler:
        #        self._logger.addHandler(file_handler)

    def _create_console_handler(self) -> logging.Handler:
        """创建控制台日志处理器"""
        handler = AnimationAwareStreamHandler(sys.stdout)
        handler.setFormatter(ColoredFormatter(self.show_timestamp))
        handler.setLevel(self.log_level)
        return handler

    def _create_file_handler(self) -> Optional[logging.Handler]:
        """创建文件日志处理器"""
        try:
            os.makedirs(self.log_file_directory, exist_ok=True)
            log_filename = datetime.now().strftime(f"{self.app_name}_%Y-%m-%d_%H-%M-%S.log")
            log_filepath = os.path.join(self.log_file_directory, log_filename)
            
            handler = logging.FileHandler(log_filepath, encoding='utf-8')
            handler.setFormatter(logging.Formatter(
                '%(asctime)s - %(name)s - %(levelname)s - %(message)s',
                datefmt=self.DATE_FORMAT
            ))
            handler.setLevel(self.log_file_level)
            return handler
        except Exception as e:
            sys.stderr.write(
                f"{TermColors.RED}Error: Failed to initialize file logging: {e}{TermColors.RESET}\n"
            )
            return None

    # 新增方法：检查是否可以打印上下文
    def should_print_context(self) -> bool:
        """检查是否应该打印上下文，只有在DEBUG级别且PRINT_CONTEXT为True时才打印"""
        return self.log_level <= logging.DEBUG and self.print_context

    # 以下是日志方法
    def debug(self, message: str, exc_info: bool = False):
        """记录调试级别日志"""
        self._logger.debug(message, exc_info=exc_info)

    def info(self, message: str, exc_info: bool = False):
        """记录信息级别日志"""
        self._logger.info(message, exc_info=exc_info)

    def warning(self, message: str, exc_info: bool = False):
        """记录警告级别日志"""
        self._logger.warning(message, exc_info=exc_info)

    def error(self, message: str, exc_info: bool = False):
        """记录错误级别日志"""
        self._logger.error(message, exc_info=exc_info)

    def critical(self, message: str, exc_info: bool = False):
        """记录严重错误级别日志"""
        self._logger.critical(message, exc_info=exc_info)

    def info_color(self, message: str, color: str = TermColors.GREEN, exc_info: bool = False):
        """使用自定义颜色输出信息"""
        print(f"{color}[INFO]: {message}{TermColors.RESET}")

    # 以下是动画控制方法
    def start_loading_animation(
        self,
        message: str = "Processing",
        animation_style: str = DEFAULT_ANIMATION_STYLE,
        color: str = DEFAULT_ANIMATION_COLOR
    ):
        """启动加载动画"""
        with self._animation_lock:
            if self._is_animating:
                self.debug("Animation already running, not starting another one.")
                return

            self._stop_animation_event.clear()
            animation_chars = self.ANIMATION_STYLES.get(
                animation_style, 
                self.ANIMATION_STYLES[self.DEFAULT_ANIMATION_STYLE]
            )

            # 计算初始宽度
            initial_char = animation_chars[0]
            initial_line = f"{color}{message} {initial_char}{TermColors.RESET} "
            stripped_line = self._strip_ansi_codes(initial_line)
            initial_width = self._wcswidth(stripped_line)

            # 更新状态
            self._is_animating = True
            self._current_animation_line_width = initial_width

            # 启动动画线程
            self._animation_thread = threading.Thread(
                target=self._animate,
                args=(message, animation_chars, color),
                daemon=True
            )
            self._animation_thread.start()

    def stop_loading_animation(self, success: bool = True, final_message: Optional[str] = None):
        """停止加载动画"""
        was_animating = False
        
        with self._animation_lock:
            if self._is_animating or self._animation_thread is not None:
                was_animating = True
                self._stop_animation_event.set()

        if not was_animating:
            if final_message:
                self._log_final_message(success, final_message)
            return

        # 等待动画线程结束
        if self._animation_thread and self._animation_thread.is_alive():
            self._animation_thread.join(timeout=2)

        with self._animation_lock:
            self._is_animating = False
            self._current_animation_line_width = 0
            self._animation_thread = None

        if final_message:
            self._log_final_message(success, final_message)

    def _log_final_message(self, success: bool, message: str):
        """记录最终消息"""
        if success:
            self.info(f"{TermColors.GREEN}✔{TermColors.RESET} {message}")
        else:
            self.error(f"{TermColors.RED}✖{TermColors.RESET} {message}")

    def _animate(self, message: str, animation_chars: List[str], color: str):
        """动画线程主函数"""
        idx = 0
        last_char = animation_chars[0]

        while not self._stop_animation_event.is_set():
            char = animation_chars[idx % len(animation_chars)]
            last_char = char

            # 构造动画行并计算宽度
            line = f"{color}{message} {char}{TermColors.RESET} "
            stripped_line = self._strip_ansi_codes(line)
            width = self._wcswidth(stripped_line)

            # 更新状态
            with self._animation_lock:
                self._current_animation_line_width = width

            # 输出动画
            sys.stdout.write(f"\r{line}")
            sys.stdout.flush()

            idx += 1
            time.sleep(0.12)

        # 清理动画行
        final_line = f"{color}{message} {last_char}{TermColors.RESET} "
        stripped_final = self._strip_ansi_codes(final_line)
        width = self._wcswidth(stripped_final)

        sys.stdout.write("\r" + " " * width + "\r")
        sys.stdout.flush()

    # 实用工具方法
    @staticmethod
    def _strip_ansi_codes(text: str) -> str:
        """移除ANSI转义码"""
        ansi_escape = re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')
        return ansi_escape.sub('', text)

    @staticmethod
    def _wcswidth(s: str) -> int:
        """计算字符串显示宽度，非ASCII字符计为2"""
        if not isinstance(s, str):
            return len(s) if s else 0
        return sum(2 if ord(c) > 127 else 1 for c in s)


class AnimationAwareStreamHandler(logging.StreamHandler):
    """处理动画状态的流处理器"""
    
    def emit(self, record):
        logger = Logger()
        
        if hasattr(record, 'is_animation_control') and record.is_animation_control:
            super().emit(record)
            return

        # 检查动画状态
        with logger._animation_lock:
            should_clear = logger._is_animating and logger._current_animation_line_width > 0
            width = logger._current_animation_line_width

        # 如果需要，清除动画行
        if should_clear:
            self.acquire()
            try:
                self.flush()
                self.stream.write("\r" + " " * width + "\r")
                self.stream.flush()
            finally:
                self.release()

        super().emit(record)


class ColoredFormatter(logging.Formatter):
    """带颜色和时间戳的日志格式化器"""
    
    def __init__(self, show_timestamp: bool):
        super().__init__(datefmt=Logger.DATE_FORMAT)
        self.show_timestamp = show_timestamp

    def format(self, record):
        if hasattr(record, 'is_animation_control') and record.is_animation_control:
            return record.getMessage()

        # 时间戳部分
        timestamp = f"{self.formatTime(record, Logger.DATE_FORMAT)} " if self.show_timestamp else ""
        
        # 消息内容
        message = record.getMessage()
        level = f"[{record.levelname}]: "
        
        # 根据级别设置颜色
        if record.levelno == logging.DEBUG:
            return f"{TermColors.GREY}{timestamp}{level}{message}{TermColors.RESET}"
        
        color = ""
        if record.levelno == logging.INFO:
            color = TermColors.GREEN
        elif record.levelno == logging.WARNING:
            color = TermColors.YELLOW
        elif record.levelno == logging.ERROR:
            color = TermColors.RED
            
        return f"{timestamp}{color}{level}{TermColors.RESET}{message}"


# 全局单例实例
logger = Logger()