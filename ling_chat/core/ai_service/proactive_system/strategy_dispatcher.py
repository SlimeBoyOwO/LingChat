from datetime import datetime, date
import os
import random
from typing import Optional
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.proactive_system.type import PerceptionResult, UserState
from ling_chat.core.pic_analyzer import DesktopAnalyzer
from ling_chat.schemas.schedule_settings import TodoItem, UserScheduleSettings
from ling_chat.core.llm_providers.manager import LLMManager

class StrategyDispatcher:
    def __init__(self, game_status: GameStatus, settings: UserScheduleSettings):
        self.game_status = game_status
        self.settings = settings
        self.pending_topics = []
        self.llm_helper = LLMManager() # 辅助LLM
        self.desktop_analyzer = DesktopAnalyzer() # 桌面分析器

    async def get_proactive_prompt(self, perception: PerceptionResult) -> Optional[str]:
        """
        生成主动对话的 Prompt。
        优先顺序: ImportantDay (Only once) > Todo > Scene/Topic > Screen Observation
        """

        today_str = datetime.now().strftime("%m-%d")

        # 1. 检查 ImportantDay (如果是今天且未触发过)
        enable_important_day = os.getenv("ENABLE_IMPORTANT_DAY_REMINDER", "false").lower() == "true"
        if enable_important_day and self.settings.importantDays:
            last_talk_date = self.game_status.last_dialog_time.strftime("%m-%d") if self.game_status.last_dialog_time else ""
            if last_talk_date != today_str:
                for day in self.settings.importantDays:
                    if day.date.endswith(today_str):
                        return f"{{今天是特殊的一天：{day.title}，{day.desc}。可以和{self.game_status.player.user_name}聊聊哦}}"

        # 2. 随机模式选择，根据启用状态动态构建候选列表
        # 基础权重: TODO(10), TOPIC(60), SCREEN(30)
        # 禁用某个模式后，random.choices 会自动对剩余权重做归一化
        modes = []
        weights = []

        if os.getenv("ENABLE_TODO_PRECEPTION", "false").lower() == "true":
            modes.append("TODO")
            if perception.state == UserState.WORK:  # 工作状态时，TODO 的权重更高
                weights.append(60)
            else:
                weights.append(10)

        if os.getenv("ENABLE_TOPIC_CREATER", "false").lower() == "true":
            modes.append("TOPIC")
            if perception.state == UserState.IDLE:  # 挂机状态时，单纯 主动对话 的权重更高
                weights.append(80)
            else:
                weights.append(60)

        if os.getenv("ENABLE_VISUAL_PRECEPTION", "true").lower() == "true":
            modes.append("SCREEN")
            if perception.state == UserState.GAME:  # 游戏状态时，视奸的权重更高
                weights.append(60)
            else:
                weights.append(30)

        if not modes:
            return None

        mode = random.choices(modes, weights=weights)[0]

        if mode == "TODO":
            prompt = self._get_todo_prompt()
            if prompt:
                return prompt
            # 没有 Todo 时降级到 TOPIC（如果启用）
            if os.getenv("ENABLE_TOPIC_CREATER", "false").lower() == "true":
                return await self._get_topic_prompt()
            return None

        if mode == "SCREEN":
            analyze_prompt = "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。用户开放了那个AI的自主窥屏功能，请获取桌面画面中的重点内容，用200字描述主体部分即可。如果你看到一个聊天窗口，有角色的立绘和对话框，不要描述这部分，只描述桌面上的其他内容。因为那部分是玩家与AI的聊天窗口。"
            analyze_info = await self.desktop_analyzer.analyze_desktop(analyze_prompt)
            ai_name = self.game_status.current_character.display_name if self.game_status.current_character else "你"
            return f"{{ {ai_name} 偷看了一眼 {self.game_status.player.user_name} 的电脑桌面信息: {analyze_info} }}"

        # TOPIC
        return await self._get_topic_prompt()

    def _get_todo_prompt(self) -> Optional[str]:
        """随机抽取未完成且重要的 Todo"""
        if not self.settings.todoGroups: return None
        
        candidates:list[TodoItem] = []
        for _, group in self.settings.todoGroups.items():
            for todo in group.todos:
                if not todo.completed and todo.priority >= 1:
                    candidates.append(todo)
        
        if not candidates: return None
        
        selected = random.choice(candidates)
        return f"{{你想起来{self.game_status.player.user_name}有一个未完成的任务：'{selected.text} '。提醒一下吧？}}"

    async def _get_topic_prompt(self) -> str:
        # TODO： 这里应该发送上下文给一个专门创造话题的 LLM
        """获取聊天话题"""
        if not self.pending_topics:
            # 话题枯竭，使用 LLM 生成补充
            # 这里是伪代码，实际需要调用 LLM 接口
            # new_topics = await self.llm_helper.generate_topics(self.game_status.memory)
            new_topics = ["天气", "最近的游戏", "休息一下"] 
            self.pending_topics.extend(new_topics)
            
        topic = self.pending_topics.pop(0) if self.pending_topics else "发呆"
        ai_name = self.game_status.current_character.display_name if self.game_status.current_character else "你"
        return f"{{ {ai_name} 想继续说话了}}"