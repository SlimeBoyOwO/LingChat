import asyncio
from typing import List, Optional

from ling_chat.core.service_manager import service_manager
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.game_database.models import LineAttribute

class ChapterEndEvent(BaseEvent):
    """处理对话事件"""
    async def _execute(self):
        end_type = self.event_data.get('end_type', 'linear')
        next_chapter = self.event_data.get('next_chapter', 'end')

        if end_type == 'linear':
            return self._handle_linear()
        elif end_type == 'branching':
            result = await self._handle_branching()
            return result
        elif end_type == 'ai_judged':
            result = await self._handle_ai_judged()
            return result
        else:
            logger.error(f"未知的 end_type: {end_type}，当作 linear 处理")
            return next_chapter

    def _handle_linear(self) -> str:
        return self.event_data.get('next_chapter', 'end')

    async def _handle_branching(self) -> str:
        options = self.event_data.get('options', [])
        if not options:
            logger.warning("branching 类型没有 options，返回 end")
            return 'end'

        vars_dict = self.script_status.vars
        default_next = None

        for opt in options:
            if opt.get('default'):
                default_next = opt.get('next', 'end')
                continue

            condition = opt.get('condition')
            if condition:
                if ScriptFunction.evaluate(condition, vars_dict):
                    logger.info(f"条件 '{condition}' 满足，选择分支: {opt.get('name', opt.get('next'))}")
                    return opt.get('next', 'end')

        if default_next is not None:
            logger.info(f"没有条件满足，使用默认分支: {default_next}")
            return default_next
        else:
            logger.warning("branching 没有满足的条件，也没有默认选项，返回 end")
            return 'end'

    async def _handle_ai_judged(self) -> str:
        prompt = self.event_data.get('prompt', '')
        options = self.event_data.get('options', [])
        if not options:
            logger.warning("ai_judged 类型没有 options，返回 end")
            return 'end'

        recent_dialogue = self._get_recent_dialogue(limit=10)
        ai_input = self._build_ai_prompt(prompt, recent_dialogue, options)

        try:
            ai_response = await self._call_llm(ai_input)
            logger.info(f"AI 返回: {ai_response}")

            matched_next = ScriptFunction.match_ai_response_options(ai_response, options)
            if matched_next:
                return matched_next

            default_opt = next((opt for opt in options if opt.get('default')), None)
            if default_opt:
                logger.info(f"AI 结果 '{ai_response}' 未匹配，使用默认选项")
                return default_opt.get('next', 'end')

            logger.warning(f"AI 结果 '{ai_response}' 未匹配任何选项，且无默认，返回 end")
            return 'end'

        except Exception as e:
            logger.error(f"AI 判断失败: {e}")
            default_opt = next((opt for opt in options if opt.get('default')), None)
            return default_opt.get('next', 'end') if default_opt else 'end'

    def _get_recent_dialogue(self, limit: int = 10) -> List[str]:
        lines = []
        if self.game_status.line_list:
            for line in reversed(self.game_status.line_list[-limit:]):
                if line.attribute in (LineAttribute.USER, LineAttribute.ASSISTANT):
                    speaker = line.display_name or ("玩家" if line.attribute == LineAttribute.USER else "角色")
                    lines.append(f"{speaker}: {line.content}")
        return list(reversed(lines))

    def _build_ai_prompt(self, prompt: str, dialogue: List[str], options: List[dict]) -> str:
        option_names = [opt['name'] for opt in options if 'name' in opt]
        options_text = "、".join(option_names)

        ai_prompt = f"{prompt}\n\n"
        if dialogue:
            ai_prompt += "最近的对话：\n" + "\n".join(dialogue) + "\n\n"
        ai_prompt += f"请根据以上信息，从以下选项中选择一个最合适的：{options_text}\n"
        ai_prompt += "请只输出选项名称，不要输出其他内容。"
        return ai_prompt

    async def _call_llm(self, prompt: str) -> str:
        ai_service = service_manager.get_ai_service()
        try:
            loop = asyncio.get_running_loop()
            messages = [{"role": "user", "content": prompt}]
            response = await loop.run_in_executor(None, ai_service.llm_model.process_message, messages)
            return response.strip()
        except Exception as e:
            logger.error(f"LLM 调用失败: {e}")
            return ""

        return next_chapter
    

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'chapter_end'