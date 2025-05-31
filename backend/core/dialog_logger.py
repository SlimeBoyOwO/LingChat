import os
from datetime import datetime

class DialogLogger:

    def __init__(self):
        self.log_dir = os.path.join("data", "logs")
        os.makedirs(self.log_dir, exist_ok=True)
        self.log_file = self.setup_logging()
    
    def setup_logging(self):
        # 获取现有日志文件数量
        existing_logs = [f for f in os.listdir(self.log_dir) if f.endswith('.log')]
        next_num = len(existing_logs)
        
        # 创建日志文件路径
        log_file = os.path.join(self.log_dir, f"{next_num}.log")
        
        # 写入第一行（日期时间）
        with open(log_file, 'w', encoding='utf-8') as f:
            f.write(f"对话日期: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        
        return log_file

    def log_conversation(self, role, content):
        """记录对话内容到日志文件"""
        with open(self.log_file, 'a', encoding='utf-8') as f:
            f.write(f"{role}: {content}\n\n")