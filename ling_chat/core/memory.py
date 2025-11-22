import os
import json
import asyncio
import time
from pathlib import Path
from typing import List, Dict, Any, Optional

from ling_chat.core.logger import logger, TermColors
from ling_chat.utils.runtime_path import user_data_path
from ling_chat.core.llm_providers.manager import LLMManager

class MemorySystem:
    '''
    多维结构化记忆库系统 (Structured Memory Bank) - 修正版
    
    逻辑核心：
    1. 维护 last_processed_idx 指针，指向已经成功"总结"进记忆库的消息索引。
    2. 只有当后台任务成功更新记忆后，指针才会移动。
    3. 提供给 RAGManager 准确的切片位置，保证未总结的消息永远保留在 Context 中。
    '''

    def __init__(self, config, character_id: int):
        self.character_id = character_id if character_id is not None else 0
        self.config = config
        
        # 多少条新消息触发一次总结 (例如 50)
        self.update_interval = self._safe_read_int("MEMORY_UPDATE_INTERVAL", 50)
        # 总结后保留多少条作为上下文重叠 (例如 15)
        self.recent_window = self._safe_read_int("MEMORY_RECENT_WINDOW", 15)
        
        self.is_updating = False
        self.last_processed_idx = 0
        
        # 记忆数据结构，可以改
        self.memory_data = {
            "short_term": "暂无近期对话摘要。",
            "long_term": "暂无长期关键经历。",
            "user_info": "暂无用户特征记录。",
            "promises": "暂无未完成的约定。"
        }
        
        self.memory_dir = user_data_path / "game_data" / "memory"
        self.memory_file = self.memory_dir / f"char_{self.character_id}_structured.json"
        
        self.llm = LLMManager()
        
        self._init_prompts()
        
        self._load_memory()

    def initialize(self) -> bool:
        if not self.memory_dir.exists():
            self.memory_dir.mkdir(parents=True, exist_ok=True)
        logger.info(f"MemorySystem (Structured) 就绪 | ID: {self.character_id} | 阈值: {self.update_interval}")
        return True

    def _safe_read_int(self, key, default):
        try:
            return int(os.environ.get(key, default))
        except:
            return default

    def _init_prompts(self):
        """初始化记忆库更新的系统提示词"""
        base_role = (
            "你是一个专业的【记忆档案管理员】。你的任务是基于【旧的记忆档案】和【新增的对话日志】，"
            "生成一份更新后的、逻辑连贯的记忆文本。\n"
            "通用规则：\n"
            "1. 视角：必须严格使用【第三人称】（例如：'用户提到...'，'AI感到...'）。\n"
            "2. 时态：使用陈述语气，客观记录事实。\n"
            "3. 输出：直接输出更新后的内容本身，不要包含任何解释。\n"
            "4. 逻辑：如果没有新信息需要更新，请原样保留【旧的记忆档案】的内容。\n"
        )

        self.section_prompts = {
            "short_term": (
                f"{base_role}\n"
                "【任务目标】：生成一份【短期上下文摘要】，用于在下一次对话中承接话题。\n"
                "【处理逻辑】：\n"
                "1. 概括话题：他们刚才在聊什么？话题是否已经结束？\n"
                "2. 捕捉氛围：当前的对话气氛如何？\n"
                "3. 遗忘机制：删除旧记忆中已经过时、结束或不再相关的琐碎细节。\n"
                "4. 篇幅控制：保持在 100-200 字以内。\n"
            ),
            "long_term": (
                f"{base_role}\n"
                "【任务目标】：编撰一份【角色经历编年史】，记录具有长期价值的核心事件。\n"
                "【处理逻辑】：\n"
                "1. 过滤噪音：忽略日常问候和闲聊。\n"
                "2. 提取事件：只记录具有里程碑意义的事件。\n"
                "3. 累积更新：将新发生的关键事件追加到旧档案中。\n"
            ),
            "user_info": (
                f"{base_role}\n"
                "【任务目标】：更新【用户画像】，确保 AI 了解屏幕对面的人。\n"
                "【处理逻辑】：\n"
                "1. 事实提取：提取用户的姓名、年龄、职业、喜好、雷点等。\n"
                "2. 冲突修正：如果信息冲突（如换了工作），以【新增对话】为准。\n"
            ),
            "promises": (
                f"{base_role}\n"
                "【任务目标】：维护一份【待办与契约清单】。\n"
                "【处理逻辑】：\n"
                "1. 新增约定：提取对话中明确达成的承诺。\n"
                "2. 状态核销：如果能够在【新增对话】中找到已完成的证据，从清单中【删除】该条目。\n"
            )
        }

    def _load_memory(self):
        """加载 JSON 记忆文件"""
        if self.memory_file.exists():
            try:
                with open(self.memory_file, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    self.memory_data.update(data.get("data", {}))
                    self.last_processed_idx = data.get("meta", {}).get("last_processed_idx", 0)
                    logger.info(f"记忆库已加载，历史归档指针位置: {self.last_processed_idx}")
            except Exception as e:
                logger.error(f"记忆库加载失败: {e}")

    def save_memory(self):
        """持久化保存"""
        try:
            save_content = {
                "meta": {
                    "last_processed_idx": self.last_processed_idx,
                    "updated_at": time.strftime("%Y-%m-%d %H:%M:%S")
                },
                "data": self.memory_data
            }
            with open(self.memory_file, "w", encoding="utf-8") as f:
                json.dump(save_content, f, ensure_ascii=False, indent=2)
            logger.info("记忆库已保存到磁盘。")
        except Exception as e:
            logger.error(f"记忆库保存失败: {e}")

    def get_memory_prompt(self) -> str:
        """构造注入 System Prompt 的文本"""
        return (
            f"\n====== 核心记忆库 (Memory Bank) ======\n"
            f"【用户信息】：{self.memory_data['user_info']}\n"
            f"【重要约定】：{self.memory_data['promises']}\n"
            f"【长期经历】：{self.memory_data['long_term']}\n"
            f"【近期回顾】：{self.memory_data['short_term']}\n"
            f"====================================\n"
        )

    def check_and_trigger_auto_update(self, history_messages: List[Dict]):
        """
        自动步跳检测
        """
        if self.is_updating:
            return

        current_count = len(history_messages)
        delta = current_count - self.last_processed_idx

        if delta >= self.update_interval:
            logger.info_color(f"Memory: 累积未归档消息 {delta} 条 (阈值 {self.update_interval})，触发自动更新...", TermColors.YELLOW)
            self.trigger_update(history_messages)

    def trigger_update(self, history_messages: List[Dict]):
        """启动后台更新任务"""
        self.is_updating = True
        
        new_msgs = history_messages[self.last_processed_idx:]
        
        target_idx = len(history_messages)
        
        chat_text = ""
        for msg in new_msgs:
            role = "User" if msg['role'] == 'user' else "AI"
            content = msg.get('content', '')
            if not content.startswith("{系统"):
                chat_text += f"{role}: {content}\n"

        if not chat_text.strip():
            self.is_updating = False
            return

        asyncio.create_task(self._run_update_pipeline(chat_text, target_idx))

    async def _run_update_pipeline(self, chat_text: str, new_total_idx: int):
        """
        执行具体的更新流水线。
        注意：new_total_idx 是这一批消息处理完后，last_processed_idx 应该变成的值
        """
        try:
            logger.info(f"Memory: 开始处理记忆压缩 (范围: {self.last_processed_idx} -> {new_total_idx})...")
            start_time = time.time()
            loop = asyncio.get_running_loop()

            async def update_section(section_key: str):
                old_content = self.memory_data.get(section_key, "")
                prompt_req = self.section_prompts[section_key]
                
                full_prompt = (
                    f"{prompt_req}\n\n"
                    f"【旧内容】：\n{old_content}\n\n"
                    f"【新增对话】：\n{chat_text}\n\n"
                    f"【新内容】(直接输出结果，不要废话)："
                )
                
                messages = [{"role": "user", "content": full_prompt}]
                response = await loop.run_in_executor(None, self.llm.process_message, messages)
                
                cleaned = response.strip()
                if not cleaned: 
                    return section_key, old_content 
                return section_key, cleaned

            tasks = [
                update_section("short_term"),
                update_section("long_term"),
                update_section("user_info"),
                update_section("promises")
            ]
            
            results = await asyncio.gather(*tasks)
            
            for key, new_val in results:
                self.memory_data[key] = new_val

            self.last_processed_idx = new_total_idx
            self.save_memory()
            
            logger.info_color(f"Memory: 记忆库更新完成! 指针已移动至 {self.last_processed_idx}，耗时 {time.time() - start_time:.2f}s", TermColors.GREEN)

        except Exception as e:
            logger.error(f"Memory 更新流水线严重错误: {e}", exc_info=True)
        finally:
            self.is_updating = False