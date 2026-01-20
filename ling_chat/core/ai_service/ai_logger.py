import os
from datetime import datetime

from ling_chat.core.logger import TermColors, logger
from ling_chat.utils.runtime_path import user_data_path


class DialogLogger:
    def __init__(self):
        self.log_dir = str(user_data_path / "logs")
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

class AILogger:
    def __init__(self):
        self.dialog_logger = DialogLogger()

    def log_conversation(self, speaker: str, message: str):
        """记录对话日志"""
        log_message = f"{speaker}: {message}"
        logger.info_color(log_message, TermColors.WHITE)
        self.dialog_logger.log_conversation(speaker, message)

    def log_analysis_result(self, segments):
        """记录分析结果"""
        for segment in segments:
            logger.debug(f"\n分析结果 (片段 {segment['index']}):")
            logger.debug(f"  原始标记: 【{segment['original_tag']}】")
            logger.debug(f"  中文文本: {segment['following_text']}")
            if segment['motion_text']:
                logger.debug(f"  动作文本: （{segment['motion_text']}）")
            if segment['japanese_text']:
                logger.debug(f"  日文文本: <{segment['japanese_text']}>")
            logger.debug(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            if os.path.exists(segment['voice_file']):
                logger.debug(f"  对应语音: {os.path.basename(segment['voice_file'])}")
            else:
                if segment['japanese_text']:
                    logger.debug("  对应语音: (未生成或生成失败)")

    def print_debug_message(self, current_context, rag_messages, messages):
        # if logger.should_print_context():
        logger.debug("\n------ 开发者模式：以下信息被发送给了llm ------")
        for message in current_context:
            logger.debug(f"Role: {message['role']}\nContent: {message['content']}\n")

        # 增加更详细的RAG信息日志
        if rag_messages:
            logger.debug("\n------ RAG增强信息详情 ------")
            logger.debug(f"原始消息数: {len(messages)}，RAG增强后消息数: {len(current_context)}")
            logger.debug(f"RAG增强消息数量: {len(rag_messages)}，位置: 系统提示后、用户消息前")

            # 计算并输出RAG消息的总长度（字符数）
            total_rag_chars = sum(len(msg.get('content', '')) for msg in rag_messages)
            logger.debug(f"RAG增强内容总长度: {total_rag_chars} 字符")

            # 分析RAG消息类型统计
            role_counts = {}
            for msg in rag_messages:
                role = msg.get('role', 'unknown')
                role_counts[role] = role_counts.get(role, 0) + 1

            role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
            logger.debug(f"RAG消息角色分布: {role_stats}")

        logger.debug("------ 结束 ------")
