import asyncio
import time
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple
import os

from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import TermColors, logger
from ling_chat.game_database.managers.memory_manager import MemoryManager


def _safe_read_int(key: str, default: int) -> int:
    try:
        return int(os.environ.get(key, default))
    except Exception:
        return default


@dataclass
class PersistentMemoryMeta:
    """
    使用全局 line_list 的索引作为“已归档指针”，避免依赖 DB line_id（运行时可能为 None）。
    """
    last_processed_global_idx: int = 0
    updated_at: str = ""


class PersistentMemorySystem:
    """
    面向 0.4.0 新架构的“永久记忆（MemoryBank）+ 自动压缩”实现：
    - key = (save_id, role_id)，保证多人物独立
    - 当累计新台词达到阈值（默认 50）时，触发后台 LLM 总结并写回 DB MemoryBank
    - 对 LLM 上下文：在角色 system prompt 之后插入 memory_bank 文本，并裁剪历史台词窗口
    """

    def __init__(self, save_id: int, role_id: int):
        self.save_id = save_id
        self.role_id = role_id

        # 多少条“全局新增台词”触发一次总结
        self.update_interval = _safe_read_int("MEMORY_UPDATE_INTERVAL", 50)
        # 总结后保留多少条“全局台词”作为上下文重叠窗口
        self.recent_window = _safe_read_int("MEMORY_RECENT_WINDOW", 15)

        self.is_updating = False
        self.meta = PersistentMemoryMeta()

        self.memory_data: Dict[str, str] = {
            "short_term": "暂无近期对话摘要。",
            "long_term": "暂无长期关键经历。",
            "user_info": "暂无用户特征记录。",
            "promises": "暂无未完成的约定。",
        }

        self._memory_row_id: Optional[int] = None
        self._llm = LLMManager()
        self._init_prompts()
        self._load_from_db()

    def _init_prompts(self) -> None:
        base_role = (
            "你是一个专业的【记忆档案管理员】。你的任务是基于【旧的记忆档案】和【新增的对话日志】，"
            "生成一份更新后的、逻辑连贯的记忆文本。\n"
            "通用规则：\n"
            "1. 视角：必须严格使用【第三人称】（例如：'用户提到...'，'AI感到...'）。\n"
            "2. 时态：使用陈述语气，客观记录事实。\n"
            "3. 输出：直接输出更新后的内容本身，不要包含任何解释。\n"
            "4. 逻辑：如果没有新信息需要更新，请原样保留【旧的记忆档案】的内容。\n"
        )

        self.section_prompts: Dict[str, str] = {
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
            ),
        }

    def _load_from_db(self) -> None:
        try:
            memory = MemoryManager.get_latest_memory(save_id=self.save_id, role_id=self.role_id)
            if not memory:
                return
            self._memory_row_id = memory.id
            info = memory.info or {}
            data = info.get("data") or {}
            meta = info.get("meta") or {}

            # 容错：不同版本字段名
            self.memory_data.update({k: str(v) for k, v in data.items() if k in self.memory_data})
            self.meta.last_processed_global_idx = int(meta.get("last_processed_global_idx", 0) or 0)
            self.meta.updated_at = str(meta.get("updated_at", "") or "")
        except Exception as e:
            logger.error(f"PersistentMemorySystem: 加载 MemoryBank 失败: {e}", exc_info=True)

    def _persist_to_db(self) -> None:
        info = {
            "schema_version": 1,
            "meta": {
                "last_processed_global_idx": self.meta.last_processed_global_idx,
                "updated_at": self.meta.updated_at,
            },
            "data": dict(self.memory_data),
        }
        try:
            self._memory_row_id = MemoryManager.upsert_memory(
                save_id=self.save_id,
                role_id=self.role_id,
                info=info,
                memory_id=self._memory_row_id,
            ).id
        except Exception as e:
            logger.error(f"PersistentMemorySystem: 保存 MemoryBank 失败: {e}", exc_info=True)

    def get_memory_prompt(self) -> str:
        return (
            "\n====== 核心记忆库 (Memory Bank) ======\n"
            f"【用户信息】：{self.memory_data.get('user_info', '')}\n"
            f"【重要约定】：{self.memory_data.get('promises', '')}\n"
            f"【长期经历】：{self.memory_data.get('long_term', '')}\n"
            f"【近期回顾】：{self.memory_data.get('short_term', '')}\n"
            "====================================\n"
        )

    def get_slice_start_index(self) -> int:
        """
        返回给 GameRoleManager 用于裁剪 line_list 的起点。
        注意：这里用的是全局 line_list 索引窗口（而不是“可见台词数”窗口），简化实现并保持稳定。
        """
        return max(0, self.meta.last_processed_global_idx - self.recent_window)

    def check_and_trigger_auto_update(self, all_lines: List[Any]) -> None:
        if self.is_updating:
            return

        current_total = len(all_lines)
        # 取出这段区间的可见台词文本（只总结“该角色说过/听到过”的内容）
        new_lines = all_lines[self.meta.last_processed_global_idx:current_total]
        chat_text, visible_count = self._build_chat_text_and_count(new_lines)
        target_idx = current_total

        # 以“该角色可见台词数”作为触发条件（多人物同剧本时确保每个角色独立计数）
        if visible_count < self.update_interval:
            return

        if not chat_text.strip():
            # 这段区间对该角色完全不可见，直接移动指针，避免无限触发
            self.meta.last_processed_global_idx = target_idx
            self.meta.updated_at = time.strftime("%Y-%m-%d %H:%M:%S")
            self._persist_to_db()
            return

        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            # 没有 event loop（例如同步调用），跳过异步压缩
            logger.debug("PersistentMemorySystem: 当前无事件循环，跳过自动压缩触发。")
            return

        logger.info_color(
            f"MemoryBank: role_id={self.role_id} 累积未归档可见台词 {visible_count} 条 (阈值 {self.update_interval})，触发自动压缩...",
            TermColors.YELLOW,
        )
        self.is_updating = True
        asyncio.create_task(self._run_update_pipeline(loop, chat_text, target_idx))

    def _build_chat_text_and_count(self, lines: List[Any]) -> Tuple[str, int]:
        """
        将 GameLine 列表转换为“User/AI: ...”日志。
        规则：
        - 忽略 system 台词（system 主要用于 persona/prompt，不参与摘要）
        - 对该角色来说：自己说的 => AI；别人说的（且可见）=> User
        - 只纳入“可见台词”：sender == role_id 或 role_id in perceived_role_ids
        """
        chunks: List[str] = []
        visible_count = 0
        for line in lines:
            try:
                if getattr(line, "attribute", None) == "system":
                    continue

                sender_role_id = getattr(line, "sender_role_id", None)
                perceived_ids = getattr(line, "perceived_role_ids", []) or []

                is_visible = (sender_role_id == self.role_id) or (self.role_id in perceived_ids)
                if not is_visible:
                    continue

                content = (getattr(line, "content", "") or "").strip()
                if not content:
                    continue

                role = "AI" if sender_role_id == self.role_id else "User"
                chunks.append(f"{role}: {content}")
                visible_count += 1
            except Exception:
                continue

        return ("\n".join(chunks) + ("\n" if chunks else "")), visible_count

    async def _run_update_pipeline(self, loop: asyncio.AbstractEventLoop, chat_text: str, new_total_idx: int) -> None:
        try:
            logger.info(
                f"MemoryBank: 开始处理记忆压缩 role_id={self.role_id} (范围: {self.meta.last_processed_global_idx} -> {new_total_idx})..."
            )
            start_time = time.time()

            async def update_section(section_key: str) -> Tuple[str, str]:
                old_content = self.memory_data.get(section_key, "")
                prompt_req = self.section_prompts[section_key]

                full_prompt = (
                    f"{prompt_req}\n\n"
                    f"【旧内容】：\n{old_content}\n\n"
                    f"【新增对话】：\n{chat_text}\n\n"
                    "【新内容】(直接输出结果，不要废话)："
                )

                messages = [{"role": "user", "content": full_prompt}]
                response = await loop.run_in_executor(None, self._llm.process_message, messages)
                cleaned = (response or "").strip()
                return section_key, (cleaned if cleaned else old_content)

            tasks = [
                update_section("short_term"),
                update_section("long_term"),
                update_section("user_info"),
                update_section("promises"),
            ]

            results = await asyncio.gather(*tasks)
            for key, new_val in results:
                self.memory_data[key] = new_val

            self.meta.last_processed_global_idx = new_total_idx
            self.meta.updated_at = time.strftime("%Y-%m-%d %H:%M:%S")
            self._persist_to_db()

            logger.info_color(
                f"MemoryBank: role_id={self.role_id} 记忆库更新完成! 指针已移动至 {self.meta.last_processed_global_idx}，耗时 {time.time() - start_time:.2f}s",
                TermColors.GREEN,
            )
        except Exception as e:
            logger.error(f"MemoryBank 更新流水线严重错误: {e}", exc_info=True)
        finally:
            self.is_updating = False

