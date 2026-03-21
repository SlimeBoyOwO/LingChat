from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.script_engine.events.events_handler_loader import (
    EventHandlerLoader,
)
from ling_chat.core.logger import logger
from ling_chat.core.ai_service.script_engine.utils.expression import evaluate


class EventsHandler:
    """事件处理器，负责按顺序执行章节内的事件，并处理条件跳转与章节结束结果。"""

    def __init__(
        self, config: AIServiceConfig, event_list: list[dict], game_status: GameStatus
    ):
        """
        初始化事件处理器。
        :param config: AI服务配置
        :param event_list: 事件配置列表
        :param game_status: 游戏状态对象
        """
        self.progress = 0                # 当前处理到的事件索引
        self.config = config
        self.game_status = game_status
        self.event_list: list[dict] = event_list
        self.current_event: dict = {}
        self._chapter_result = None      # 存储章节结束事件返回的结果（下一章名称）

    def is_finished(self) -> bool:
        """
        判断是否所有事件已处理完毕，或者已获得章节结果。
        若已获得章节结果，则立即结束章节（即使还有未处理的事件）。
        """
        if self._chapter_result is not None:
            return True
        return self.progress >= len(self.event_list)

    def get_chapter_result(self) -> str:
        """获取章节处理结果（下一章节名），若无结果则返回 "end"。"""
        return self._chapter_result if self._chapter_result is not None else "end"

    async def process_next_event(self):
        """处理下一个事件，若获得章节结果则保存。"""
        if self.is_finished():
            return

        self.current_event = self.event_list[self.progress]
        self.progress += 1

        result = await self.process_event(self.current_event)

        if result is not None:
            self._chapter_result = result

    async def process_event(self, event: dict):
        """
        处理单个事件，可能返回章节结束结果。
        :param event: 事件配置字典
        :return: 若事件为 chapter_end 则返回其下一章名称，否则返回 None
        """
        event_type = event.get("type", "unknown")
        logger.info(f"处理事件 {self.progress}/{len(self.event_list)}: {event_type}")

        # 处理 condition 属性：若不满足条件则跳过该事件
        condition = event.get("condition")
        if condition:
            # 获取剧本变量字典（只使用剧本变量，不包含全局变量）
            vars_dict = (
                self.game_status.script_status.vars.copy()
                if self.game_status.script_status
                else {}
            )
            if not evaluate(condition, vars_dict):
                logger.info(f"条件 '{condition}' 不满足，跳过事件 {event_type}")
                return None

        try:
            handler_class = EventHandlerLoader.get_handler_for_event(event)

            if handler_class is not None:
                handler = handler_class(self.config, event, self.game_status)
                result = await handler.execute()

                # 章节结束事件需要返回结果供外层使用
                if event_type == "chapter_end":
                    return result
                return None
            else:
                logger.error(f"找不到对应{event_type}的事件处理器，跳过当前事件")
                return None

        except Exception as e:
            logger.error(f"处理事件时出错: {event} - {e}")
            import traceback

            traceback.print_exc()
            return None