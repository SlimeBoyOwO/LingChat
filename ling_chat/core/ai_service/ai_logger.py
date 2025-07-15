import os
from datetime import datetime

class AILogger:
    def __init__(self):
        self.log_dir = os.path.join("data", "logs")
        os.makedirs(self.log_dir, exist_ok=True)
        self.log_file = self.setup_logging()
    
    def setup_logging(self):
        """确定本次对话使用的日志文件名（基于现有文件数量），创建该文件，并在文件开头写入会话开始的日期和时间"""
        existing_logs = [f for f in os.listdir(self.log_dir) if f.endswith('.log')]
        next_num = len(existing_logs)
        
        log_file = os.path.join(self.log_dir, f"{next_num}.log")
        
        with open(log_file, 'w', encoding='utf-8') as f:
            f.write(f"对话日期: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        
        return log_file

    def log_conversation(self, role, content):
        """记录对话内容到日志文件"""
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(f"{role}: {content}\n\n")