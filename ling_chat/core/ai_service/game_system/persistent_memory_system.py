import asyncio
import os
import time
from typing import Dict, List, Tuple

from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import TermColors, logger
from ling_chat.core.ai_service.type import GameRole
from ling_chat.game_database.models import GameLine
from ling_chat.core.ai_service.game_system.memory_builder import MemoryBuilder


def _safe_read_int(key: str, default: int) -> int:
    try:
        return int(os.environ.get(key, default))
    except Exception:
        return default


class PersistentMemorySystem:
    """
    面向 0.4.0 新架构的“永久记忆（MemoryBank）+ 自动压缩”实现（运行时缓存版）：
    - **不在此处做任何 DB 读写**：仅更新 `GameRole.memory_bank`（内存缓存）
    - 当累计“该角色可见台词”达到阈值（默认 50）时，触发后台 LLM 总结并更新 memory_bank
    - 对 LLM 上下文：通过 `get_slice_start_index()` 控制裁剪窗口，避免上下文无限膨胀
    """

    def __init__(self, role: GameRole):
        if role.role_id is None:
            raise ValueError("PersistentMemorySystem 需要 role.role_id 不为空")
        self.role = role
        self.role_id = role.role_id

        # 多少条“新增可见台词”触发一次总结
        self.update_interval = _safe_read_int("MEMORY_UPDATE_INTERVAL", 250)
        # 总结后保留多少条“全局台词”作为上下文重叠窗口（默认 15，避免过大上下文）
        self.recent_window = _safe_read_int("MEMORY_RECENT_WINDOW", 15)

        self.is_updating = False
        self._llm = LLMManager()
        self._init_prompts()

    @staticmethod
    def is_enabled() -> bool:
        """
        永久记忆开关：只有用户显式开启时才启用。
        通过 /api/settings/config 修改 .env 后会同步到 os.environ（运行时热更新）。
        """
        return os.environ.get("USE_PERSISTENT_MEMORY", "False").lower() == "true"

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

    def get_slice_start_index(self) -> int:
        """
        返回给 GameRoleManager 用于裁剪 line_list 的起点。
        注意：使用全局 line_list 索引窗口，简单稳定（memory_builder 会自行过滤不可见台词）。
        """
        meta = self.role.memory_bank.meta
        return max(0, meta.last_processed_global_idx - self.recent_window)

    def get_system_memory_text(self) -> str:
        """
        建议合并进“唯一的 system prompt”中的长期记忆部分（避免新增 system 消息）。
        这里不包含 short_term，short_term 更适合插到 user 上下文里。
        """
        mb = self.role.memory_bank
        return (
            "\n\n====== 记忆库 (Memory Bank) ======\n"
            f"【用户信息】：{mb.data.user_info}\n"
            f"【重要约定】：{mb.data.promises}\n"
            f"【长期经历】：{mb.data.long_term}\n"
            "=================================\n"
        )

    def get_short_term_user_text(self) -> str:
        """
        建议作为 user 消息内容的前缀插入（或合并进第一条 user 消息），用于短期承接。
        """
        short_term = (self.role.memory_bank.data.short_term or "").strip()
        if not short_term:
            return ""
        return f"【近期回顾】{short_term}\n\n"

    def check_and_trigger_auto_update(self, all_lines: List[GameLine]) -> None:
        if not self.is_enabled():
            return
        if self.is_updating:
            return

        current_total = len(all_lines)
        meta = self.role.memory_bank.meta

        # 指针异常时回滚
        if meta.last_processed_global_idx < 0 or meta.last_processed_global_idx > current_total:
            meta.last_processed_global_idx = 0

        # 取出这段区间的可见台词文本（只总结“该角色说过/听到过”的内容）
        new_lines = all_lines[meta.last_processed_global_idx:current_total]
        chat_text, visible_count = self._build_chat_text_and_count(new_lines)
        target_idx = current_total

        # 以“该角色可见台词数”作为触发条件（多人物同剧本时确保每个角色独立计数）
        if visible_count < self.update_interval:
            return

        if not chat_text.strip():
            # 这段区间对该角色完全不可见，直接移动指针，避免无限触发
            meta.last_processed_global_idx = target_idx
            meta.updated_at = time.strftime("%Y-%m-%d %H:%M:%S")
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

    def _build_chat_text_and_count(self, lines: List[GameLine]) -> Tuple[str, int]:
        """
        复用MemoryBuilder来构建对话摘要输入，避免角色混乱与信息遗漏。
        - visible_count：只统计“该角色可见”的非 system 台词数量，用于触发阈值判断
        - chat_text：用 MemoryBuilder.build(lines) 的结果生成单次 prompt 文本，保留 display_name/情绪/动作/TTS 等信息
        """
        def _attr_value(line: GameLine) -> str:
            a = line.attribute
            try:
                return a.value
            except Exception:
                return str(a)

        # 统计可见台词数
        visible_count = 0
        for line in lines:
            if _attr_value(line) == "system":
                continue
            if line.sender_role_id == self.role_id or (self.role_id in (line.perceived_role_ids or [])):
                if (line.content or "").strip():
                    visible_count += 1

        if visible_count == 0:
            return "", 0

        # 用 MemoryBuilder 构建该角色的“可见上下文”并转为摘要输入文本
        builder = MemoryBuilder(target_role_id=self.role_id)
        built = builder.build(lines)

        ai_name = self.role.display_name or f"AI({self.role_id})"
        chunks: List[str] = []
        for msg in built:
            r = msg.get("role")
            c = (msg.get("content") or "").strip()
            if not c:
                continue
            if r == "system":
                continue
            if r == "assistant":
                chunks.append(f"{ai_name}: {c}")
            elif r == "user":
                chunks.append(f"User: {c}")
            else:
                chunks.append(f"{r}: {c}")

        return ("\n".join(chunks) + ("\n" if chunks else "")), visible_count

    async def _run_update_pipeline(self, loop: asyncio.AbstractEventLoop, chat_text: str, new_total_idx: int) -> None:
        try:
            logger.info(
                f"MemoryBank: 开始处理记忆压缩 role_id={self.role_id} (范围: {self.role.memory_bank.meta.last_processed_global_idx} -> {new_total_idx})..."
            )
            start_time = time.time()

            async def update_section(section_key: str) -> Tuple[str, str]:
                # 从 dataclass 取旧内容
                data = self.role.memory_bank.data
                old_content = {
                    "short_term": data.short_term,
                    "long_term": data.long_term,
                    "user_info": data.user_info,
                    "promises": data.promises,
                }.get(section_key, "")
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
                if hasattr(self.role.memory_bank.data, key):
                    setattr(self.role.memory_bank.data, key, new_val)

            self.role.memory_bank.meta.last_processed_global_idx = new_total_idx
            self.role.memory_bank.meta.updated_at = time.strftime("%Y-%m-%d %H:%M:%S")

            logger.info_color(
                f"MemoryBank: role_id={self.role_id} 记忆库更新完成! 指针已移动至 {self.role.memory_bank.meta.last_processed_global_idx}，耗时 {time.time() - start_time:.2f}s",
                TermColors.GREEN,
            )
        except Exception as e:
            logger.error(f"MemoryBank 更新流水线严重错误: {e}", exc_info=True)
        finally:
            self.is_updating = False

