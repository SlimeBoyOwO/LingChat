import os
import sys
import json
from datetime import datetime
from enum import Enum
from typing import Dict, Optional
from dotenv import load_dotenv

load_dotenv()

# 日志级别
class LogLevel(Enum):
    DEBUG = 0
    INFO = 1
    WARNING = 2
    ERROR = 3

class Color:
    BLACK = "\033[30m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"
    BRIGHT_BLACK = "\033[90m"
    BRIGHT_RED = "\033[91m"
    BRIGHT_GREEN = "\033[92m"
    BRIGHT_YELLOW = "\033[93m"
    BRIGHT_BLUE = "\033[94m"
    BRIGHT_MAGENTA = "\033[95m"
    BRIGHT_CYAN = "\033[96m"
    BRIGHT_WHITE = "\033[97m"
    BOLD = "\033[1m"
    RESET = "\033[0m"

class Logger:
    def __init__(self):
        # 应用日志配置
        self.app_log_dir = os.environ.get("APP_LOG_DIR", "log")
        os.makedirs(self.app_log_dir, exist_ok=True)
        self.app_log_file = self._setup_app_logging()

        # 对话日志配置 (保持原有逻辑和环境变量 BACKEND_LOG_DIR)
        self.conversation_log_dir = os.environ.get("BACKEND_LOG_DIR", "logs")
        os.makedirs(self.conversation_log_dir, exist_ok=True)
        self.conversation_log_file = self._setup_conversation_logging()
        
        log_level_str = os.environ.get("LOG_LEVEL", "INFO").upper()
        try:
            self.log_level = LogLevel[log_level_str]
        except KeyError:
            self.log_level = LogLevel.INFO
            print(f"{Color.YELLOW}无效的日志级别: {log_level_str}，使用默认级别 INFO{Color.RESET}")
        
        self.level_config = {
            LogLevel.DEBUG: {"color": Color.BRIGHT_BLACK, "prefix": "DEBUG"},
            LogLevel.INFO: {"color": Color.BRIGHT_GREEN, "prefix": "INFO"},
            LogLevel.WARNING: {"color": Color.BRIGHT_YELLOW, "prefix": "WARN"},
            LogLevel.ERROR: {"color": Color.BRIGHT_RED, "prefix": "ERROR"}
        }
        
        self.status_colors = {
            "success": Color.GREEN,
            "error": Color.RED,
            "warning": Color.YELLOW
        }
    
    def _setup_app_logging(self):
        """配置应用程序日志文件路径 (例如: log/0.log)"""
        try:
            existing_logs = [f for f in os.listdir(self.app_log_dir) if f.endswith('.log') and f[:-4].isdigit()]
            next_num = 0
            if existing_logs:
                next_num = max(int(f[:-4]) for f in existing_logs) + 1
        except FileNotFoundError:
            # 如果目录刚创建，listdir可能会在此刻失败（尽管 makedirs exist_ok=True）
            # 或其他权限问题，安全起见，从0开始
            existing_logs = []
            next_num = 0
            
        app_log_file_path = os.path.join(self.app_log_dir, f"{next_num}.log")
        with open(app_log_file_path, 'a', encoding='utf-8') as f:
            f.write(f"--- Application Log Started: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')} ---\n")
        return app_log_file_path

    def _setup_conversation_logging(self):
        """配置对话日志文件路径 (例如: logs/0.log)，保持原有格式"""
        try:
            existing_logs = [f for f in os.listdir(self.conversation_log_dir) if f.endswith('.log') and f[:-4].isdigit()]
            next_num = 0
            if existing_logs:
                next_num = max(int(f[:-4]) for f in existing_logs) + 1
        except FileNotFoundError:
            existing_logs = []
            next_num = 0

        conv_log_file_path = os.path.join(self.conversation_log_dir, f"{next_num}.log")
        # 对话日志通常每个会话（或每次启动）是一个新文件，或追加到特定文件，这里保持了每次启动新文件并写日期的逻辑
        with open(conv_log_file_path, 'w', encoding='utf-8') as f: 
            f.write(f"对话日期: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        return conv_log_file_path

    def log_conversation(self, role, content):
        """记录对话内容到专门的对话日志文件"""
        with open(self.conversation_log_file, 'a', encoding='utf-8') as f:
            f.write(f"{role}: {content}\n\n")

    def _log(self, level: LogLevel, message: str, file_output: bool = True, force_message_color: Optional[str] = None):
        """基础日志输出函数"""
        if level.value < self.log_level.value:
            return
        
        config = self.level_config[level]
        now = datetime.now().strftime('%H:%M:%S')
        
        colored_log_prefix = f"{config['color']}[{config['prefix']}]{Color.RESET}"
        
        message_body_color = force_message_color if force_message_color else config['color']
        if force_message_color == Color.WHITE: 
            message_body_color = Color.WHITE
        
        console_message = f"{colored_log_prefix} {message_body_color}{message}{Color.RESET}"
        print(console_message)
        
        if file_output:
            plain_log_prefix = f"[{config['prefix']}]" 
            with open(self.app_log_file, 'a', encoding='utf-8') as f:
                f.write(f"{now} {plain_log_prefix} {message}\n")

    def log_text(self, message: str):
        """输出默认颜色（白色）的文本，不添加前缀"""
        print(message)

    def debug(self, message: str):
        """调试级别日志"""
        self._log(LogLevel.DEBUG, message)

    def info(self, message: str):
        """信息级别日志"""
        self._log(LogLevel.INFO, message)

    def info_white_text(self, message: str):
        """INFO级别日志，但消息文本为白色"""
        self._log(LogLevel.INFO, message, force_message_color=Color.WHITE)

    def warning(self, message: str):
        """警告级别日志"""
        self._log(LogLevel.WARNING, message)

    def error(self, message: str):
        """错误级别日志"""
        self._log(LogLevel.ERROR, message)

    def service_status(self, service_name: str, is_running: bool, details: Optional[str] = None, status_type: str = None):
        """输出服务状态信息（成功/失败）"""
        status_type = status_type or ("success" if is_running else "error")
        status_text = f"已{'运行' if is_running else '停止'}"
        color = self.status_colors.get(status_type, Color.RESET)
        
        status_part = f"{color}[{service_name} {status_text}]{Color.RESET}" 
        
        cleaned_details = details
        if details and details.strip() == "()":
            cleaned_details = None 
        elif details:
            cleaned_details = details.strip()

        if cleaned_details:
            message_to_log = f"{status_part} {Color.WHITE}{cleaned_details}{Color.RESET}"
        else:
            message_to_log = status_part
        

        level = LogLevel.INFO if is_running else LogLevel.WARNING
        self._log(level, message_to_log, file_output=False, force_message_color=Color.WHITE if cleaned_details else None)
        
    def emotion_model_status(self, is_success: bool, details: Optional[str] = None):
        """情绪模型加载状态"""
        status = "情绪分类模型加载正常" if is_success else "情绪分类模型加载异常"
        self.service_status(status, is_success, details, "success" if is_success else "error")
    
    def tts_status(self, is_running: bool, details: Optional[str] = None):
        """语音服务状态"""
        status = "语音服务已运行" if is_running else "语音服务未运行"
        cleaned_details = details
        if details:
            details_str = str(details).strip()
            if details_str == "()" or details_str == "('()',)":
                cleaned_details = None
            elif "vits-simple-api未运行" in details_str and "语音功能将被禁用" in details_str:
                import re
                match = re.search(r'\((.*?)\)', details_str)
                if match and not match.group(1).strip(): 
                    cleaned_details = details_str.split('(')[0].strip()
                else:
                    cleaned_details = details_str 

        self.service_status(status, is_running, cleaned_details, "success" if is_running else "error")
    
    def backend_status(self, is_running: bool, details: Optional[str] = None):
        """后端服务状态"""
        status = "后端服务已启动" if is_running else "后端服务未启动"
        self.service_status(status, is_running, details, "success" if is_running else "error")
    
    def client_message(self, message: Dict):
        """记录客户端消息"""
        try:
            message_str = json.dumps(message, ensure_ascii=False)
            self.debug(f"收到原始客户端消息: {message_str}")
        except TypeError:
            self.debug(f"收到原始客户端消息 (无法JSON序列化): {message}")
    
    def client_connect(self):
        """记录客户端连接"""
        self.info(f"新的客户端连接建立")
    
    def client_disconnect(self):
        """记录客户端断开连接"""
        self.info(f"客户端断开连接")
    
    def response_info(self, count: int):
        """记录响应信息"""
        self.info(f"准备发送 {count} 条回复")