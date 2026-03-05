from datetime import datetime, date
import random
from typing import Optional
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.proactive_system.type import PerceptionResult
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
        # 注意：这里需要持久化记录今天是否提醒过，这里简化处理，假设 memory 会记录
        if self.settings.importantDays:
            last_talk_date = self.game_status.last_dialog_time.strftime("%m-%d") if self.game_status.last_dialog_time else ""
            if last_talk_date != today_str:
                for day in self.settings.importantDays:
                    if day.date.endswith(today_str): # 简单匹配月日
                        return f"{{今天是特殊的一天：{day.title}，{day.desc}。可以和{self.game_status.player.user_name}聊聊哦}}"

        # 2. 随机模式选择
        # 权重: Todo(30%), Topic(30%), Screen(20%), Script(0% - Not Implemented)
        mode = random.choices(["TODO", "TOPIC", "SCREEN"], weights=[10, 60, 30])[0]

        if mode == "TODO":
            prompt = self._get_todo_prompt()
            if prompt: return prompt
            # 如果没有 Todo，降级到 TOPIC
        
        if mode == "SCREEN":
            # 调用辅助LLM总结屏幕内容，或者直接把OCR扔给AI
            analyze_prompt = "你是一个图像信息转述者，你将需要把你看到的画面描述给另一个AI让他理解用户的图片内容。用户开放了那个AI的自主窥屏功能，请获取桌面画面中的重点内容，用200字描述主体部分即可。"
            analyze_info = await self.desktop_analyzer.analyze_desktop(analyze_prompt)
            ai_name = self.game_status.current_character.display_name if self.game_status.current_character else "你"
            sys_desktop_part = f"{{ {ai_name} 偷看了一眼 {self.game_status.player.user_name} 的电脑桌面信息: {analyze_info} }}"
            return sys_desktop_part

        # Default: TOPIC
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