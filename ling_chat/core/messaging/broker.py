import asyncio
from typing import Dict, AsyncGenerator

class MessageBroker:
    def __init__(self):
        self.queues: Dict[str, asyncio.Queue] = {}

    async def publish(self, client_id: str, message: dict):
        if client_id not in self.queues:
            self.queues[client_id] = asyncio.Queue()
        await self.queues[client_id].put(message)

    async def subscribe(self, client_id: str) -> AsyncGenerator[dict, None]:
        if client_id not in self.queues:
            self.queues[client_id] = asyncio.Queue()
        while True:
            yield await self.queues[client_id].get()
    
    async def enqueue_ai_message(self, client_id: str, message: str):
        """专门用于将消息加入到AI处理队列"""
        await self.publish(f"ai_input_{client_id}", {
            "type": "user_message", 
            "content": message
        })
    
    async def enqueue_ai_script_message(self, client_id: str, message: str):
        """专门用于将消息加入到AI剧本处理队列"""
        await self.publish(f"ai_script_input_{client_id}", {
            "type": "user_message", 
            "content": message
        })

# 单例模式
message_broker = MessageBroker()