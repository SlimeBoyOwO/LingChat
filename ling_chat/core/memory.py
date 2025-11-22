import os
import json
from typing import List, Dict, Optional
from ling_chat.core.logger import logger, TermColors
from ling_chat.utils.runtime_path import user_data_path

class MemorySystem:
    '''
    Memory Bank 系统。
    不使用向量检索，而是通过定期压缩历史对话来维护长期记忆。
    机制：
    1. 维护一个 "Memory Bank" (总结后的记忆文本)。
    2. 每次对话时，将 Memory Bank 插入到 System Prompt 之后。
    3. 当对话轮数达到阈值时，触发后台压缩更新，更新后截断上下文。
    '''

    def __init__(self, config, character_id: int):
        if character_id is None:
            raise ValueError("MemorySystem必须使用一个有效的character_id进行初始化。")

        self.config = config
        self.character_id = character_id
        
        # === 安全读取配置 (步跳机制参数) ===
        self.update_interval = self._safe_read_int("MEMORY_UPDATE_INTERVAL", 50)
        self.recent_window = self._safe_read_int("MEMORY_RECENT_WINDOW", 15)
        
        # === 状态管理 ===
        # 存储当前的记忆库文本
        self.memory_bank_text = "" 
        # 标记是否已经拥有有效的总结记忆（用于决定是否截断上下文）
        self.has_valid_memory = False
        # 标记是否正在后台更新（防止重复触发）
        self.is_updating = False
        
        # 记忆文件路径
        self.memory_file_path = user_data_path / "game_data" / "memory" / f"char_{character_id}_bank.txt"
        self._load_memory_bank()

    def _safe_read_int(self, env_key: str, default_val: int) -> int:
        """安全读取环境变量为整数"""
        try:
            val = os.environ.get(env_key)
            if val is None:
                return default_val
            return int(val)
        except (ValueError, TypeError):
            logger.warning(f"环境变量 {env_key} 格式错误，使用默认值: {default_val}")
            return default_val

    def initialize(self) -> bool:
        """初始化 (接口兼容性保留)"""
        logger.info(f"Memory Bank 系统已就绪 (角色ID: {self.character_id})")
        logger.info(f"配置: 每 {self.update_interval} 轮触发更新，保留最近 {self.recent_window} 轮上下文")
        return True

    def _load_memory_bank(self):
        """加载持久化的记忆库"""
        if self.memory_file_path.exists():
            try:
                with open(self.memory_file_path, "r", encoding="utf-8") as f:
                    self.memory_bank_text = f.read()
                    if self.memory_bank_text.strip():
                        self.has_valid_memory = True
            except Exception as e:
                logger.error(f"加载记忆库失败: {e}")
        else:
            # 初始占位符
            self.memory_bank_text = "【记忆库：暂无长期记忆，正在积累对话...】"
            self.has_valid_memory = False

    def save_memory_bank(self, text: str):
        """保存记忆库 (供后台任务调用)"""
        try:
            self.memory_file_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.memory_file_path, "w", encoding="utf-8") as f:
                f.write(text)
            self.memory_bank_text = text
            self.has_valid_memory = True
            logger.info("记忆库已保存更新")
        except Exception as e:
            logger.error(f"保存记忆库失败: {e}")

    def get_memory_prompt(self) -> str:
        """获取注入 Prompt 的固定字段"""
        prefix = "以下是你的核心记忆库(Memory Bank)，包含了过去的重要信息，请基于此保持人设连贯性：\n"
        return f"{prefix}======\n{self.memory_bank_text}\n======\n"

    def check_and_trigger_update(self, history_count: int):
        """
        步跳机制检测：检查是否满足更新条件
        注意：这里仅做触发检测，具体的后台任务逻辑需在下一步接入 LLM 时实现
        """
        if self.is_updating:
            return

        # 如果历史记录超过设定阈值，触发更新
        # 逻辑：如果是第一次达到50条，或者距离上次处理又过了50条
        # 这里暂时简化逻辑：只要当前上下文长度超过阈值 + 缓冲窗口，且没有正在更新，就应当触发
        if history_count >= self.update_interval:
            logger.info_color(f"MemorySystem: 对话轮数 ({history_count}) 达到阈值 {self.update_interval}，触发记忆压缩...", TermColors.YELLOW)
            self.trigger_background_update()

    def trigger_background_update(self):
        """
        触发后台更新任务 (占位符)
        TODO: 这里需要接入 LLM 进行总结，是异步操作
        """
        self.is_updating = True
        # 模拟：实际开发中这里会启动一个 asyncio task 调用 LLM 总结 history
        logger.info("后台记忆更新任务已启动 (当前为占位符)...")
        
        # 假设更新完成（在实际代码中这应该在回调里）
        # self.is_updating = False 
        # self.has_valid_memory = True